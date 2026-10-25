# Datenquellen

Alle Daten, die die Anwendung verwendet, mit Herkunft, Rechtslage und
Aktualisierungsweg. Rechtlich verbindlich bleibt [ATTRIBUTION.md](../ATTRIBUTION.md).

## 1. Stempelstellen

| Feld | Wert |
| --- | --- |
| Datei | `data/official/HWN2025.gpx` |
| Herkunft | Harzer Verkehrsverband e.V., GPX-Archiv Stand 23.04.2025 |
| Umfang | 222 `<wpt>`-Elemente, keine Tracks |
| Format | GPX 1.1, erzeugt von MapSource 6.13.7 |
| Prüfsumme | `3e3667b13b8b7c69c25a95709cbc9f213d5be87f79f3fdcd921a64fade838bbc` |
| Metadaten | `data/official/PROVENANCE.json` |

Felder je Stempel: `name` (z. B. `HWN001 Eckertalsperre`), `cmt`/`desc` (Ort),
`ele` (Höhe), `lat`/`lon` (sieben Nachkommastellen).

**Aufbereitung.** `cargo xtask data` erzeugt daraus
`data/catalog/hwn-2025-stamps.json`:

1. `HWNnnn` aus dem Namen abtrennen und als `code` führen,
2. Weißraum in Namen und Ortsangaben normalisieren (die Quelle enthält in 20
   Einträgen Zeilenumbrüche),
3. Koordinaten auf ihre kürzeste exakte Darstellung bringen,
4. Prüfen: fortlaufende Nummerierung 1..222, keine Duplikate, Prüfsumme der
   Quelle stimmt.

Die Quelldatei wird **nicht** verändert. `cargo xtask data --check` schlägt fehl,
wenn Katalog und Quelle auseinanderlaufen.

**Rechte.** Siehe [ATTRIBUTION.md](../ATTRIBUTION.md), Abschnitt 1. Kurzfassung:
Die Daten stammen vom Herausgeber; eine Weiterverbreitungserlaubnis ist nicht
bekannt. Das Projekt führt sie mit, weil die Anwendung ohne sie nicht
funktioniert; wer die Anwendung weiterverbreitet, muss das selbst klären.

**Aktualisierung.** Neue Fassung herunterladen, `data/official/HWN2025.gpx`
ersetzen, Prüfsumme und Datum in `PROVENANCE.json` anpassen, `cargo xtask data`
ausführen, Tests laufen lassen. Ändern sich Stempelnummern oder Koordinaten,
ist das ein eigener Pull Request mit Hinweis im Changelog — bestehende
Nutzerauswahlen beziehen sich auf Nummern.

## 2. Kartenkacheln

| Feld | Wert |
| --- | --- |
| Anbieter | OpenStreetMap Foundation, Standard-Kachelserver |
| URL | `https://tile.openstreetmap.org/{z}/{x}/{y}.png` |
| Lizenz | ODbL 1.0, Attribution „© OpenStreetMap-Mitwirkende“ |
| Speicherung | keine; weder im Repository noch im Service-Worker-Cache |

Die URL ist in `web/js/config.js` austauschbar. Wer eine eigene Instanz
betreibt, sollte das tun: der öffentliche Server ist für mäßigen Verkehr
gedacht, nicht für eine vielbesuchte Anwendung.

Wichtig: **kein Offline-Archiv.** Der Service Worker speichert bewusst keine
Kacheln. Ein Vorrat an Kacheln wäre ein „Download in Bulk“ und damit ein
Verstoß gegen die Nutzungsrichtlinie.

## 3. Routingdaten

Der Planer liefert **keine** Routingdaten mit. Die Wegführung holt er zur
Laufzeit von **BRouter** (Profil `hiking-beta`), dessen Datengrundlage
OpenStreetMap ist.

| Feld | Wert |
| --- | --- |
| Dienst | `https://brouter.de/brouter`, Profil `hiking-beta` |
| Software | BRouter, MIT-Lizenz (Java, auch selbst betreibbar) |
| Daten | OpenStreetMap, ODbL 1.0 |
| Ablage | keine; Antworten nur im Arbeitsspeicher |
| Aufrufe | eine Anfrage pro Tag, nur beim Berechnen einer Tour |

Wer den Dienst öffentlich betreibt oder einen eigenen einsetzt, muss die
ODbL-Attribution sichtbar führen und die Share-Alike-Bedingungen beachten.
Details und Alternativen: [routing-design.md](routing-design.md).

## 4. Parkplatzdaten

Es sind **keine** Parkplatzdaten mitgeliefert; sie kommen aus OpenStreetMap über
die Overpass-API.

| Feld | Wert |
| --- | --- |
| Abfrage | `amenity=parking`, `highway=layby` im Umkreis der Auswahl |
| Dienst | `https://overpass-api.de/api/interpreter` |
| Lizenz | ODbL 1.0, Attribution „© OpenStreetMap-Mitwirkende“ |
| Aufrufe | nur auf Klick, Ergebnis zehn Minuten zwischengespeichert |
| Filter | `access=private`, `=no`, `=customers` werden verworfen |

Ein eigenes Verzeichnis wäre die Alternative, wurde aber bewusst verworfen: Es
hätte unklare Rechtslage und würde veralten.

**Was die Daten nicht sind.** „Parkplatz“ in OpenStreetMap bedeutet „jemand hat
eine Fläche eingetragen“, nicht „offizieller Wanderparkplatz der Harzer
Wandernadel“. Öffnungszeiten, Gebühren und saisonale Sperrungen werden nicht
ausgewertet. Die Rangfolge (Anzahl erreichbarer Stempel, Fußweg, Art) ist eine
Plausibilitätshilfe, keine Zusage.

Bewertet werden die Kandidaten im Rust-Kern (`parking-search`), nicht in
JavaScript — dieselbe Logik läuft damit auch in Tests.

## 5. Geodätische Grundlagen

- Koordinaten: WGS84, Dezimalgrad.
- Distanzen: Haversine-Formel mit mittlerem Erdradius 6 371 008,8 m.
- Optimierung: lokale equirectangulare Projektion (Fehler < 0,5 % über die
  Ausdehnung des Harz).
- GPX-Export: sieben Nachkommastellen (rund 1 cm), Höhen mit einer
  Nachkommastelle.

Diese Werte sind für die Wanderplanung ausreichend; es sind keine
katasteramtlichen Genauigkeiten und sie werden auch nicht behauptet.

## 6. Grenzen des Datensatzes

Der Katalog enthält, was die amtliche Datei enthält: Nummer, Name, Ort,
Koordinaten, Höhe. **Nicht** enthalten sind:

- Öffnungszeiten oder Zugänglichkeit (einzelne Stempel sind saisonal,
  bewirtet oder an Gebäude gebunden),
- Angaben zu Wegen oder Schwierigkeit,
- Parkplätze,
- Hinweise auf gesperrte Stempel.

Die Anwendung behauptet auch nicht, diese Informationen zu haben.
