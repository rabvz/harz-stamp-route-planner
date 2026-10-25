# Entwicklungsstatus

Stand: 2026-10-25, Version 0.1.0

Diese Datei ist die verbindliche Statusquelle. Wenn README, Oberfläche und diese
Datei sich widersprechen, gilt diese Datei — und der Widerspruch ist ein Fehler,
der behoben werden muss.

Legende: **fertig** · **eingeschränkt** (funktioniert, mit benannter Lücke) ·
**offen** · **blockiert**

## Kern

| Baustein | Status | Beleg |
| --- | --- | --- |
| Koordinaten, Distanzen, Bounding-Box | fertig | `crates/geo-engine`, 32 Tests |
| Douglas-Peucker-Vereinfachung | fertig | `geo-engine/src/polyline.rs` |
| Lokale Projektion für die Optimierung | fertig | `geo-engine/src/plane.rs` |
| Stempelkatalog aus amtlicher GPX-Datei | fertig | 222 Einträge, `cargo xtask data --check` |
| Auswahlmodell | fertig | `stamp-core/src/selection.rs` |
| Gruppierung | fertig | `crates/stamp-clustering`, 13 Tests |
| Distanzmatrix und Reihenfolge (NN + 2-opt) | fertig | `crates/route-optimizer`, 39 Tests |
| Tageseinteilung mit Warnungen | fertig | `route-optimizer/src/split.rs` |
| GPX 1.1 lesen | fertig | `crates/gpx/src/reader.rs` |
| GPX 1.1 schreiben | fertig | `crates/gpx/src/writer.rs` |
| Parkplatz-Ranking | fertig | `crates/parking-search`, 13 Tests |
| JSON-ABI für den Browser | fertig | `crates/wasm-bindings`, 19 Tests |

## Anwendung

| Funktion | Status | Anmerkung |
| --- | --- | --- |
| Karte mit 222 Markern | fertig | Leaflet, lokal eingebunden |
| Auswahl per Marker und Liste | fertig | im Browser geprüft |
| Suche und Filter | fertig | Name, Ort, HWN-Nummer |
| Tour berechnen | fertig | mit Luftlinien-Schätzung |
| Gruppen bilden | fertig | |
| GPX-Export der Auswahl | fertig | im Browser geprüft |
| GPX-Export der Tour | eingeschränkt | Track nur mit Routing-Dienst |
| GPX-Import | fertig | Zuordnung über `HWNnnn` |
| Startpunkt per Kartenklick | fertig | |
| Parkplätze aus OpenStreetMap | offen | Operation vorhanden, Oberfläche nicht angebunden |
| Persistenz (IndexedDB) | fertig | plus JSON-Export und -Import |
| Progressive Web App | eingeschränkt | Bedienoberfläche offline, Karte nicht |
| Mobiles Layout | fertig | Breakpoint 860 px, nicht auf echten Geräten getestet |

## Werkzeuge und Qualität

| Baustein | Status | Beleg |
| --- | --- | --- |
| `cargo xtask ci` | fertig | fmt, clippy, Tests, Daten, Architektur, Lizenzen, GPX, WASM, Web |
| Architekturprüfung | fertig | `xtask/src/checks.rs` |
| Lizenzprüfung (SPDX) | fertig | dito |
| Datensatz-Driftprüfung | fertig | `cargo xtask data --check` |
| Browsertest | fertig | `scripts/smoke-test.mjs`, 15 Prüfungen |
| CI-Workflows | fertig | `.github/workflows/` |
| Sicherheitsprüfung (`cargo audit`, `cargo deny`) | fertig | `security.yml` |
| Release-Workflow | fertig | manuell ausgelöst, `release.yml` |
| Pages-Deployment | eingeschränkt | Workflow fertig, Pages im Repository **nicht aktiviert** |

## Offene Punkte

1. **Echtes Routing im Standardbetrieb.** Ohne eingetragenen Dienst rechnet der
   Planer mit Luftlinie × 1,25. Der GPX-Export enthält dann keinen Track. Siehe
   [routing-design.md](routing-design.md).
2. **Parkplatzverzeichnis.** Es ist keines enthalten. Kandidaten können über die
   `parking`-Operation eingespeist werden; die Oberfläche bindet Overpass noch
   nicht an.
3. **Screenshots und Demo.** Der Screenshot in `docs/images/` wurde lokal
   erzeugt. Die Live-Adresse funktioniert erst nach Aktivierung von GitHub Pages.
4. **Testabdeckung der Oberfläche.** Der Smoke-Test deckt den Hauptpfad ab;
   Import, Datenexport und Fehlerfälle der Oberfläche sind nicht automatisiert.
5. **Barrierefreiheit.** Tastaturbedienung ist vorhanden, ein Test mit
   Screenreadern fehlt.

## Blockiert

| Thema | Hindernis | Nötig |
| --- | --- | --- |
| GitHub Pages aktivieren | Repository-Einstellung, nicht im Code | Berechtigung am Repository |
| Discussions aktivieren | Repository-Einstellung | Berechtigung am Repository |
| Branch-Protection / Rulesets für `main` | Repository-Einstellung | Berechtigung am Repository |
| Öffentliches Release 0.1.0 | bewusste Freigabe | Entscheidung des Maintainers |
| Parkplatzdaten mit Weiterverbreitungsrecht | keine geprüfte Quelle | Datenquelle oder Entscheidung für Overpass |

## Nächste Schritte

Siehe [roadmap.md](roadmap.md). Kurzfristig: Overpass-Anbindung für Parkplätze,
Routing-Dienst dokumentiert anbinden, Oberflächentests erweitern.
