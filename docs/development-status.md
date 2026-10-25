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
| Distanzmatrix und Reihenfolge (NN + 2-opt) | fertig | `crates/route-optimizer`, 33 Tests |
| Tagestour mit Warnungen | fertig | `route-optimizer/src/tour.rs` |
| GPX 1.1 lesen | fertig | `crates/gpx/src/reader.rs` |
| GPX 1.1 schreiben, inklusive Höhe je Trackpunkt | fertig | `crates/gpx/src/writer.rs` |
| Parkplatz-Ranking | fertig | `crates/parking-search`, 13 Tests |
| JSON-ABI für den Browser | fertig | `crates/wasm-bindings`, 19 Tests |

## Anwendung

| Funktion | Status | Anmerkung |
| --- | --- | --- |
| Karte mit 222 Markern | fertig | Leaflet, lokal eingebunden |
| Auswahl per Marker und Liste | fertig | im Browser geprüft |
| Suche und Filter | fertig | Name, Ort, HWN-Nummer, optional nur ungestempelte |
| Stempelpass | fertig | „bereits gestempelt“ je Stempel, Fortschritt x/222, IndexedDB und JSON-Export |
| Tour berechnen | fertig | Reihenfolge auf Schätzung, genau eine Tagestour |
| Routing entlang von Wanderwegen | fertig | BRouter `hiking-beta`, im Browser geprüft |
| Track bearbeiten | fertig | Wegpunkte setzen, ziehen (Einrasten auf der Route), entfernen und neu routen |
| Höhenprofil | fertig | Auf-/Abstieg und Kurve aus den BRouter-Höhen, als SVG |
| Gruppen bilden | fertig | |
| GPX-Export der Auswahl | fertig | im Browser geprüft |
| GPX-Export der Tagestour | fertig | inklusive Track, Höhe und Weggeometrie |
| GPX-Import | fertig | Zuordnung über `HWNnnn` |
| Startpunkt per Kartenklick | fertig | |
| Parkplätze aus OpenStreetMap | fertig | Overpass, im Browser geprüft; nur auf Klick |
| Persistenz (IndexedDB) | fertig | Zustandsversion 2, Migration von Version 1; plus JSON-Export und -Import |
| Progressive Web App | eingeschränkt | Bedienoberfläche offline, Karte und Routing nicht |
| Mobiles Layout | eingeschränkt | Breakpoint 860 px, nicht auf echten Geräten getestet |

## Werkzeuge und Qualität

| Baustein | Status | Beleg |
| --- | --- | --- |
| `cargo xtask ci` | fertig | fmt, clippy, Tests, Daten, Architektur, Lizenzen, GPX, WASM, Web |
| Architekturprüfung | fertig | `xtask/src/checks.rs` |
| Lizenzprüfung (SPDX) | fertig | dito |
| Datensatz-Driftprüfung | fertig | `cargo xtask data --check` |
| Browsertest | fertig | `scripts/smoke-test.mjs`, 49 Prüfungen (mit `EXPECT_NETWORK=1` streng) |
| CI-Workflows | fertig | `.github/workflows/` |
| Sicherheitsprüfung (`cargo audit`, `cargo deny`) | fertig | `security.yml` |
| Secret Scanning, Push Protection, Dependabot | fertig | im Repository aktiviert |
| Release-Workflow | fertig | manuell ausgelöst, `release.yml` |
| Pages-Deployment | fertig | Workflow aktiv, Pages veröffentlicht unter <https://rabvz.github.io/harz-stamp-route-planner/> |
| Branch-Protection für `main` | fertig | Pull Request und sechs Pflichtprüfungen, Force-Push und Löschen gesperrt, gilt auch für Admins |
| Repository-Beschreibung, Website, Themen | fertig | im Repository hinterlegt |
| Discussions | fertig | aktiviert; Kategorien legt GitHub selbst an |
| Offene Punkte als Issues | fertig | [#2](https://github.com/rabvz/harz-stamp-route-planner/issues/2) bis [#15](https://github.com/rabvz/harz-stamp-route-planner/issues/15) |

## Offene Punkte

1. **Routing nur online.** Die Wegführung kommt von einem öffentlichen BRouter.
   Ohne Netz bleibt die Luftlinien-Schätzung, klar gekennzeichnet. Track
   bearbeiten und Höhenprofil brauchen deshalb ebenfalls eine Verbindung. Ein
   eigener Dienst ist in `web/js/config.js` eintragbar; siehe
   [routing-design.md](routing-design.md).
2. **Optimierung und Routing sind getrennt.** Die Reihenfolge entsteht aus der
   Schätzung, erst die fertige Tagestour wird geroutet. Eine Matrix über echte
   Distanzen ist der nächste Schritt ([#2](https://github.com/rabvz/harz-stamp-route-planner/issues/2)).
3. **Einrasten nur auf der berechneten Route.** Ein gezogener Wegpunkt rastet
   auf der Weggeometrie ein, nicht auf jedem nahen OSM-Weg. Dafür bräuchte es
   eine Overpass-Abfrage beim Ablegen
   ([#13](https://github.com/rabvz/harz-stamp-route-planner/issues/13)).
4. **Parkplatzdaten sind OpenStreetMap-Daten.** Vollständigkeit, Zugang und
   Öffnungszeiten sind nicht geprüft; die Oberfläche nennt die Quelle noch nicht
   bei jedem Eintrag
   ([#11](https://github.com/rabvz/harz-stamp-route-planner/issues/11)).
5. **Testabdeckung der Oberfläche.** Der Smoke-Test deckt den Hauptpfad
   inklusive Track-Bearbeitung und Höhenprofil ab; Import, Datenexport und
   Fehlerfälle sind nicht automatisiert
   ([#3](https://github.com/rabvz/harz-stamp-route-planner/issues/3)).
6. **Barrierefreiheit.** Tastaturbedienung ist für die Schaltflächen vorhanden,
   Wegpunkte lassen sich aber nur mit der Maus setzen und verschieben. Ein Test
   mit Screenreadern fehlt
   ([#4](https://github.com/rabvz/harz-stamp-route-planner/issues/4)).
7. **Mobiles Layout.** Der Breakpoint ist gesetzt, aber nicht auf echten Geräten
   geprüft ([#12](https://github.com/rabvz/harz-stamp-route-planner/issues/12)).

## Blockiert

| Thema | Hindernis | Nötig |
| --- | --- | --- |
| Öffentliches Release 0.1.0 | bewusste Freigabe | Entscheidung des Maintainers |

## Nächste Schritte

Siehe [roadmap.md](roadmap.md). Kurzfristig: Matrix über echte Distanzen,
Oberflächentests für Import und Datenexport, Barrierefreiheit der Track-Bearbeitung.
