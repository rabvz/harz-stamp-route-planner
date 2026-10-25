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
| Tour berechnen | fertig | Reihenfolge auf Schätzung, Tage geroutet |
| Routing entlang von Wanderwegen | fertig | BRouter `hiking-beta`, im Browser geprüft |
| Tagesgrenzen nach dem Routing anpassen | fertig | bis zu acht Durchläufe, Rest wird gemeldet |
| Gruppen bilden | fertig | |
| GPX-Export der Auswahl | fertig | im Browser geprüft |
| GPX-Export der Tour | fertig | inklusive Track aus der Weggeometrie |
| GPX-Import | fertig | Zuordnung über `HWNnnn` |
| Startpunkt per Kartenklick | fertig | |
| Parkplätze aus OpenStreetMap | fertig | Overpass, im Browser geprüft; nur auf Klick |
| Persistenz (IndexedDB) | fertig | plus JSON-Export und -Import |
| Progressive Web App | eingeschränkt | Bedienoberfläche offline, Karte und Routing nicht |
| Mobiles Layout | fertig | Breakpoint 860 px, nicht auf echten Geräten getestet |

## Werkzeuge und Qualität

| Baustein | Status | Beleg |
| --- | --- | --- |
| `cargo xtask ci` | fertig | fmt, clippy, Tests, Daten, Architektur, Lizenzen, GPX, WASM, Web |
| Architekturprüfung | fertig | `xtask/src/checks.rs` |
| Lizenzprüfung (SPDX) | fertig | dito |
| Datensatz-Driftprüfung | fertig | `cargo xtask data --check` |
| Browsertest | fertig | `scripts/smoke-test.mjs`, 20 Prüfungen (mit `EXPECT_NETWORK=1` streng) |
| CI-Workflows | fertig | `.github/workflows/` |
| Sicherheitsprüfung (`cargo audit`, `cargo deny`) | fertig | `security.yml` |
| Release-Workflow | fertig | manuell ausgelöst, `release.yml` |
| Pages-Deployment | eingeschränkt | Workflow fertig, Pages im Repository **nicht aktiviert** |

## Offene Punkte

1. **Routing nur online.** Die Wegführung kommt von einem öffentlichen BRouter.
   Ohne Netz bleibt die Luftlinien-Schätzung, klar gekennzeichnet. Ein eigener
   Dienst ist in `web/js/config.js` eintragbar; siehe
   [routing-design.md](routing-design.md).
2. **Optimierung und Routing sind getrennt.** Die Reihenfolge entsteht aus der
   Schätzung, die Tagesgrenzen werden danach an die echten Distanzen angepasst.
   Eine Matrix über echte Distanzen (echte Optimierung) ist der nächste Schritt.
3. **Parkplatzdaten sind OpenStreetMap-Daten.** Vollständigkeit, Zugang und
   Öffnungszeiten sind nicht geprüft; die Oberfläche sagt das nicht bei jedem
   Eintrag, die Dokumentation schon.
4. **Screenshots und Demo.** Der Screenshot in `docs/images/` wurde lokal
   erzeugt. Die Live-Adresse funktioniert erst nach Aktivierung von GitHub Pages.
5. **Testabdeckung der Oberfläche.** Der Smoke-Test deckt den Hauptpfad ab;
   Import, Datenexport und Fehlerfälle der Oberfläche sind nicht automatisiert.
6. **Barrierefreiheit.** Tastaturbedienung ist vorhanden, ein Test mit
   Screenreadern fehlt.

## Blockiert

| Thema | Hindernis | Nötig |
| --- | --- | --- |
| GitHub Pages aktivieren | Repository-Einstellung, nicht im Code | Berechtigung am Repository |
| Discussions aktivieren | Repository-Einstellung | Berechtigung am Repository |
| Branch-Protection / Rulesets für `main` | Repository-Einstellung | Berechtigung am Repository |
| Öffentliches Release 0.1.0 | bewusste Freigabe | Entscheidung des Maintainers |

## Nächste Schritte

Siehe [roadmap.md](roadmap.md). Kurzfristig: Overpass-Anbindung für Parkplätze,
Routing-Dienst dokumentiert anbinden, Oberflächentests erweitern.
