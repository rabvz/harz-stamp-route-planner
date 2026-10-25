# Routing-Design

## Problem

Eine Wanderung folgt Wegen, keine Luftlinie. Der Planer muss also echte
Wegdistanzen kennen, um Kilometer, Tagestour, Karte, Höhenprofil und GPX-Track
sinnvoll zu berechnen.

Gleichzeitig gilt eine harte Randbedingung: **GitHub Pages hat kein Backend.**
Eine statische Seite kann keinen API-Schlüssel geheim halten. Jede Lösung, die
einen Schlüssel im JavaScript braucht, ist damit ausgeschlossen.

## Entscheidung

Zwei Bausteine, bewusst getrennt:

1. **Der Rust-Kern kennt kein I/O.** Er arbeitet auf einer Distanzmatrix und
   einer Provider-Abstraktion (`routing-client`). Damit läuft derselbe Code
   nativ in Tests und im Browser.
2. **Die Wegführung holt die Oberfläche.** `web/js/routing.js` ruft einen
   öffentlichen Dienst auf, der ohne Schlüssel auskommt: **BRouter** mit dem
   Profil `hiking-beta` (Datenbasis OpenStreetMap).

### Warum BRouter

| Kriterium | BRouter |
| --- | --- |
| Schlüssel nötig | nein — entscheidend für eine statische Seite |
| Wanderprofil | ja (`hiking-beta`, `hiking-mountain`) |
| Mehrere Wegpunkte pro Anfrage | ja, als Liste in `lonlats` |
| Antwortformat | GeoJSON-Linie mit `track-length` und `total-time` |
| Eigener Betrieb | ja, Java-Anwendung, MIT-Lizenz |
| CORS | `Access-Control-Allow-Origin: *` |

Die letzte Zeile ist praktisch wichtig: ohne CORS könnte der Browser den Dienst
nicht aufrufen.

### Eine Anfrage pro Tagestour, nicht pro Etappe

BRouter akzeptiert die ganze Tour als Liste von Wegpunkten:

```text
Parkplatz → Stempel 1 → Stempel 2 → … → Parkplatz
```

Damit kostet eine Tagestour **eine** Anfrage statt einer pro Kettenglied, und die
Antwort ist eine durchgehende Linie — genau das, was Karte, Höhenprofil und GPX
brauchen. Die Grenze von 16 Wegpunkten pro Anfrage ist in `web/js/config.js`
hinterlegt; längere Touren werden in überlappende Stücke geteilt und die
Distanzen summiert.

### Reihenfolge und Wege sind getrennt

Die Optimierung (Nearest-Neighbour + 2-opt) rechnet auf einer **geschätzten**
Distanzmatrix, weil eine echte Matrix `(n+1)²` Anfragen kosten würde — bei zehn
Stempeln über hundert. Deshalb:

1. Reihenfolge aus der Schätzung (sofort sichtbar),
2. die ganze Tagestour über BRouter (echte Distanz, echte Geometrie, Gehzeit,
   Höhe),
3. Überschreitet die geroutete Tour das Limit, wird sie als zu lang gemeldet.
   Die Auswahl bleibt vollständig; eine Aufteilung in mehrere Tage gibt es nicht.

Das ist eine bewusste Entscheidung: eine echte Optimierung auf dem Wegenetz wäre
ein eigener Dienst. Die Architektur lässt sie zu (siehe unten), ohne den Kern zu
ändern.

### Track bearbeiten

Nach dem Routing kann der Track über zusätzliche Wegpunkte angepasst werden
(`web/js/app.js`, `web/js/track.js`):

- **Setzen:** Ein Klick in die Karte fügt einen Wegpunkt ein. Er wird auf die
  aktuelle Weggeometrie projiziert und an der passenden Stelle zwischen den
  Stempeln einsortiert.
- **Verschieben:** Wegpunkte sind auf der Karte ziehbar. Liegt der abgelegte
  Punkt höchstens 120 m neben der berechneten Route, rastet er auf ihr ein;
  weiter entfernt bleibt er frei, und BRouter sucht den nächsten Weg dorthin.
- **Entfernen:** über das Menü des Wegpunkts oder die Liste.
- Jede Änderung löst **eine** neue Routing-Anfrage aus.

Das Einrasten nutzt die zuletzt berechnete Route, nicht alle Wege in der Nähe.
Eine Suche über sämtliche OSM-Wege bräuchte eine Overpass-Abfrage bei jedem Zug;
die Grenze ist in `README.md` benannt.

### Höhenprofil

BRouter gibt die Höhe als dritte Koordinate jedes Punkts zurück. Daraus entstehen
Auf- und Abstieg sowie die Profilkurve (geglättet, mit 1-m-Schwelle gegen
Rauschwerte). Ohne Routing gibt es keine Höhendaten, und die Oberfläche zeigt
dann auch kein Profil.

### Rückfall ohne Netz

Ist BRouter nicht erreichbar, bleibt die Schätzung stehen (Luftlinie × 1,25),
die Oberfläche schreibt „geschätzt“ und der GPX-Export enthält keinen Track. Es
wird nie eine gerade Linie als Wanderweg ausgegeben.

## Vertrag mit dem Dienst

Aufgerufen wird:

```text
GET https://brouter.de/brouter
    ?lonlats=<lon,lat|lon,lat|...>
    &profile=hiking-beta
    &alternativeidx=0
    &format=geojson
```

Erwartet wird GeoJSON:

```json
{
  "type": "FeatureCollection",
  "features": [{
    "type": "Feature",
    "geometry": { "type": "LineString", "coordinates": [[lon, lat, ele], "..."] },
    "properties": { "track-length": "2909", "total-time": "2264" }
  }]
}
```

`track-length` ist die Distanz in Metern, `total-time` die Gehzeit in Sekunden —
**beides als Zeichenkette**. Der Parser wandelt Zahlen und Zeichenketten, weil
`Number.isFinite("2909")` falsch ist und die Distanz sonst stillschweigend aus
der Geometrie gemessen würde.

Ebenfalls akzeptiert werden `Feature` und `LineString` mit
`properties.summary.distance` / `.duration` (OpenRouteService, Valhalla). Fehlt
die Zusammenfassung, wird die Länge aus der Geometrie gemessen und die Dauer aus
4,5 km/h abgeleitet.

## Parkplätze

Die Startpunkte kommen aus OpenStreetMap über die Overpass-API
(`web/js/overpass.js`): `amenity=parking` und `highway=layby` im Umkreis der
Auswahl. Die Abfrage läuft **nur** auf ausdrücklichen Klick, wird zehn Minuten
zwischengespeichert und mit einem einzigen Wiederholungsversuch bei Zeitüberschreitung
wiederholt. Die Kandidaten werden im Rust-Kern (`parking-search`) sortiert: nach
Anzahl erreichbarer Stempel, mittlerem Fußweg und Art des Parkplatzes.

## Konfiguration

```js
// web/js/config.js
routing: {
  providerLabel: "BRouter",
  urlTemplate: "https://brouter.de/brouter?lonlats={lonlats}&profile={profile}&alternativeidx=0&format=geojson",
  profile: "hiking-beta",
  timeoutMs: 25000,
  maxWaypointsPerRequest: 16,
},
```

`{lonlats}` und `{profile}` werden ersetzt. Ein eigener BRouter:

```js
urlTemplate: "https://brouter.example.org/brouter?lonlats={lonlats}&profile={profile}&format=geojson",
```

Ein leerer `urlTemplate` schaltet das Routing ab; der Planer arbeitet dann rein
auf Schätzungen. Antworten werden im Speicher zwischengespeichert, damit
dieselbe Tagestour nicht mehrfach abgefragt wird.

## Warum kein Schlüssel im Client

Ein Schlüssel in einer statischen Seite ist öffentlich, unabhängig davon, wie er
verschleiert wird. Wer einen Dienst mit Schlüssel nutzen will, braucht einen
eigenen Proxy. Die Architektur ist darauf vorbereitet: derselbe Rust-Kern läuft
hinter einem Backend, weil er kein I/O kennt — nur `wasm-bindings` müsste durch
einen HTTP-Handler ersetzt werden.

## Nächster Ausbauschritt: Matrix von außen

Die `plan`-Operation akzeptiert bereits eine fertige Distanzmatrix:

```json
{
  "op": "plan",
  "payload": {
    "stampIds": [1, 2, 3],
    "start": { "lat": 51.841649, "lon": 10.5799757 },
    "matrix": [0, 2021, 5100, 2021, 0, 3100, 5100, 3100, 0],
    "providerName": "brouter-hiking"
  }
}
```

Mit `matrix` rechnet der Kern auf echten Distanzen, `estimated` wird `false`.
Kosten: `(n+1)²` Anfragen, deshalb für kleine Auswahlen gedacht. Der Aufbau im
Browser mit Fortschritt und Abbruch ist der nächste Schritt (siehe
[roadmap.md](roadmap.md)).

## Selbst betriebene Alternativen

| Dienst | Wanderprofil | Lizenz der Software | Bemerkung |
| --- | --- | --- | --- |
| BRouter | `hiking-beta`, `hiking-mountain` | MIT | Java, lokal oder als Dienst, Daten aus OpenStreetMap |
| Valhalla | `pedestrian` | MIT/Apache-2.0 (Teile) | C++, eigene Kacheln nötig |
| OpenRouteService | `foot-hiking` | GPL-3.0 | als Dienst mit Schlüssel oder selbst betrieben |
| OSRM | – | BSD-2 | kein Fußprofil in der Standarddistribution |

Bei allen gilt: Die **Routingdaten** stammen meist aus OpenStreetMap und
unterliegen der ODbL. Wer einen Dienst öffentlich betreibt, muss die Attribution
sichtbar führen. Details in [data-sources.md](data-sources.md).
