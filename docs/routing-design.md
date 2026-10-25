# Routing-Design

## Problem

Eine Wanderung folgt Wegen, keine Luftlinie. Der Planer muss also echte
Wegdistanzen kennen, um Kilometer, Tageseinteilung und GPX-Track sinnvoll zu
berechnen.

Gleichzeitig gilt eine harte Randbedingung: **GitHub Pages hat kein Backend.**
Eine statische Seite kann keinen API-Schlüssel geheim halten. Jede Lösung, die
einen Schlüssel im JavaScript braucht, ist damit ausgeschlossen.

## Entscheidung

Der Rust-Kern kennt **kein I/O**. Er arbeitet auf einer Distanzmatrix und auf
einer Provider-Abstraktion:

```rust
pub trait RouteProvider {
    fn name(&self) -> &'static str;
    fn is_estimate(&self) -> bool;
    fn route(&self, from: LatLon, to: LatLon) -> Result<RouteLeg, RoutingError>;
}
```

Es gibt zwei Wege, echte Distanzen zu bekommen:

### A. Matrix von außen (bevorzugt)

Die Oberfläche baut die Matrix und übergibt sie an die `plan`-Operation:

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

Die Matrix ist zeilenweise, `(n+1)²` Einträge, Index 0 ist der Parkplatz. Dann
ist `estimated: false` und die Optimierung läuft auf echten Distanzen.

Kosten: `n²` Anfragen. Für 20 Stempel sind das 400. Deshalb ist dieser Weg für
kleine Auswahlen vorgesehen; die Oberfläche kann ihn mit Fortschrittsanzeige und
Abbruch anbieten.

### B. Etappen nachrüsten (Standard in 0.1)

Der Plan wird zunächst mit Luftlinie berechnet. Ist ein Dienst konfiguriert,
holt die Oberfläche anschließend die Geometrie der **berechneten** Etappen
(`2n` Anfragen, eine pro Kettenglied). Die Distanzen werden ersetzt, der Track
steht für den GPX-Export zur Verfügung. Die Reihenfolge selbst beruht weiterhin
auf der Schätzung.

Das ist bewusst so: es ist ehrlich, schnell und für eine Tagesausflugsplanung
meist ausreichend. Die Oberfläche schreibt in diesem Fall „geroutet“, sonst
„geschätzt“.

## Vertrag mit dem Dienst

Die Oberfläche spricht einen **einzigen** Endpunkt an und erwartet eine
GeoJSON-Linie:

```text
GET <routingUrlTemplate mit {lonlats} ersetzt>
    {lonlats} = "lon,lat|lon,lat"
```

Antwort, minimal:

```json
{
  "type": "LineString",
  "coordinates": [[10.5799757, 51.841649], [10.6027742, 51.8301677]],
  "properties": { "distance": 2450.0, "duration": 1800.0 }
}
```

Ebenfalls akzeptiert: `Feature` und `FeatureCollection` mit
`geometry: LineString` und `properties.summary.distance` / `.duration`. Das ist
die gemeinsame Schnittmenge von BRouter, OpenRouteService und Valhalla im
GeoJSON-Modus. Fehlt `distance`, wird die Länge aus der Geometrie gemessen;
fehlt `duration`, wird sie aus der Gehgeschwindigkeit abgeleitet.

Der Parser liegt in `web/js/routing.js` (`parseRoute`); die Rust-Seite hat mit
`routing-client/src/geojson.rs` eine zweite, getestete Implementierung für den
Matrix-Weg.

## Konfiguration

In `web/js/config.js`:

```js
routingUrlTemplate:
  "https://brouter.example.org/brouter?lonlats={lonlats}&profile=hiking-mountain&format=geojson",
```

Leer lassen = Schätzbetrieb. Es gibt keinen Standard-Dienst, weil ein fremder
Server nicht ungefragt mit dem Verkehr einer öffentlichen Anwendung belastet
werden soll.

## Warum kein Schlüssel im Client

Ein Schlüssel in einer statischen Seite ist öffentlich, unabhängig davon, wie er
verschleiert wird. Wer einen Dienst mit Schlüssel nutzen will, braucht einen
eigenen Proxy. Die Architektur ist darauf vorbereitet: derselbe Rust-Kern läuft
hinter einem Backend, weil er kein I/O kennt — nur `wasm-bindings` müsste durch
einen HTTP-Handler ersetzt werden.

## Selbst betriebene Alternativen (lizenzkonform)

| Dienst | Profile | Lizenz der Software | Bemerkung |
| --- | --- | --- | --- |
| BRouter | hiking, hiking-mountain | MIT | Java, lokal oder als Dienst; Daten aus OpenStreetMap |
| Valhalla | pedestrian | MIT/Apache-2.0 (Teile) | C++, eigene Kacheln nötig |
| OpenRouteService | foot-hiking | GPL-3.0 | als Dienst mit Schlüssel oder selbst betrieben |
| OSRM | – | BSD-2 | kein Fußprofil in der Standarddistribution |

Bei allen gilt: die **Routingdaten** stammen meist aus OpenStreetMap und
unterliegen der ODbL. Wer einen Dienst öffentlich betreibt, muss die
Attribution sichtbar führen. Details in [data-sources.md](data-sources.md).

## Offene Arbeit

1. Matrix-Aufbau im Browser mit Begrenzung und Fortschritt (Roadmap 0.2).
2. Overpass-Anbindung für Parkplätze.
3. Höhenprofil aus der Track-Geometrie.
4. Entscheidung, ob ein Referenzdienst dokumentiert beworben wird.
