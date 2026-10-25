# Changelog

Alle nennenswerten Änderungen an diesem Projekt. Format nach
[Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung nach
[Semantic Versioning](https://semver.org/lang/de/).

## [Unveröffentlicht]

### Hinzugefügt

- **Track-Bearbeitung.** Nach dem Routing lassen sich eigene Wegpunkte setzen,
  auf der Karte ziehen und wieder entfernen. Ein gezogener Punkt rastet auf der
  berechneten Weggeometrie ein, die auf OpenStreetMap-Wegen liegt; die Tour wird
  nach jeder Änderung neu geroutet. Wegpunkte erscheinen in der Reihenfolge der
  Tour und im GPX-Track.
- **Stempelpass.** Jede Stempelstelle lässt sich als „bereits gestempelt“
  markieren. Die Oberfläche zeigt den Fortschritt (x von 222), einen
  Fortschrittsbalken, einen Filter auf ungestempelte Stellen und Aktionen für
  die ganze Auswahl. Der Stand liegt in IndexedDB und im JSON-Export.
- **Höhenprofil der Tagestour.** Auf- und Abstieg sowie die Profilkurve entstehen
  aus der Höhe, die BRouter als dritte Koordinate liefert; gezeichnet als SVG
  ohne Diagramm-Paket. Ohne Routing wird kein Profil erfunden.
- **Höhe im GPX-Track.** Trackpunkte übernehmen die Höhe aus der Routing-Antwort;
  der Export schreibt `<ele>` je Punkt.
- **GitHub-Repository eingerichtet.** Beschreibung, Website, Themen, Discussions,
  fehlende Labels und Branch-Protection für `main` (Pull Request verpflichtend,
  Force-Push und Löschen gesperrt, auch für Admins). Offene Punkte sind als
  Issues [#2](https://github.com/rabvz/harz-stamp-route-planner/issues/2) bis
  [#15](https://github.com/rabvz/harz-stamp-route-planner/issues/15) erfasst.

### Geändert

- **Nur noch Tagestouren.** Die Mehrtagesplanung mit `maxDays`, Tagesaufteilung
  und Nachjustierung der Tagesgrenzen ist entfernt. `route-optimizer` liefert
  eine Tour mit Streckenlimit und Warnung; `plan` gibt `legs`, `distanceM`,
  `stampsVisited` und `oversized` zurück, keine `days` mehr. Eine Auswahl, die
  nicht in das Limit passt, bleibt vollständig und wird als zu lang markiert.
- **Zustandsformat 2.** Auswahl, Stempelpass und Einstellungen werden mit
  `version: 2` gespeichert. Gespeicherte Daten und Exporte der Version 1 werden
  beim Laden übernommen; `maxDayKm` wird zu `maxTourKm`, `maxDays` entfällt.
- **Oberfläche und Dokumentation** beschreiben den neuen Umfang: eine Tagestour,
  Track-Bearbeitung, Höhenprofil und Stempelpass. Screenshot, README, Roadmap,
  Entwicklungsstatus, Architektur, Routing-Design, Deployment, Attribution und
  die Issue-Vorlagen sind angepasst.

### Behoben

- **Veralteter Plan nach geänderten Eingaben.** Ändert sich die Auswahl oder der
  Startpunkt, wird eine bereits berechnete Tour verworfen und die Oberfläche
  sagt, dass neu gerechnet werden muss, statt eine Tour zu zeigen, die nicht mehr
  zur Auswahl passt.
- **Track nur mit Höhe, wenn es Höhe gibt.** Eine geschätzte Tour zeigt kein
  Höhenprofil und exportiert keinen Track; die Oberfläche kennzeichnet das.

## [0.1.0] – 2026-10-25

Erste öffentliche Fassung. Der Funktionsumfang ist bewusst als Alpha
gekennzeichnet; die Grenzen stehen in `README.md` und
`docs/development-status.md`.

### Hinzugefügt

- **Rust-Workspace** mit acht fachlich getrennten Crates und maschinell
  geprüften Abhängigkeitsrichtungen (`geo-engine`, `stamp-core`,
  `parking-search`, `stamp-clustering`, `routing-client`, `route-optimizer`,
  `gpx`, `wasm-bindings`).
- **Stempelkatalog** mit allen 222 Stempelstellen aus der amtlichen GPX-Datei
  des Harzer Verkehrsverbands (Stand 23.04.2025), inklusive Provenienzdatei,
  Prüfsummenbindung und Driftprüfung (`cargo xtask data --check`).
- **Tourenoptimierung**: Distanzmatrix, Nearest-Neighbour-Konstruktion,
  2-opt-Verbesserung mit fixiertem Start, Greedy-Tageseinteilung unter
  Tageslimit. Nicht erreichbare Stempel werden als Warnung gemeldet statt
  verworfen.
- **Routing entlang echter Wanderwege**: BRouter mit Profil `hiking-beta`,
  ohne API-Schlüssel. Eine Anfrage pro Tag über alle Wegpunkte; echte Distanz,
  Gehzeit und durchgehende Geometrie für Karte und GPX. Nach dem Routing werden
  die Tagesgrenzen an die echten Distanzen angepasst (bis zu acht Durchläufe),
  verbleibende Überschreitungen werden gemeldet. Ohne Netz bleibt die
  Luftlinien-Schätzung, klar gekennzeichnet.
- **Wanderparkplätze aus OpenStreetMap**: Overpass-Abfrage für
  `amenity=parking` und `highway=layby` im Umkreis der Auswahl, ausschließlich
  auf Klick, mit Cache und einem Wiederholungsversuch. Sortiert im Rust-Kern
  nach erreichbaren Stempeln, Fußweg und Parkplatzart; `access=private` und
  ähnliches werden verworfen.
- **Gruppierung** der Auswahl in räumlich zusammenhängende Tagestouren mit
  Durchmesser- und Stempellimit, deterministisch und ohne Verletzung der
  Garantien.
- **Parkplatz-Ranking** im Rust-Kern (`parking-search`) für die von außen
  gelieferten Kandidaten (Abdeckung, Fußweg, Art des Parkplatzes).
- **Routing-Abstraktion** mit Offline-Fallback (Luftlinie × 1,25), GeoJSON-Parser
  für BRouter/ORS/Valhalla (auch Zahlen als Zeichenkette) und Routencache.
- **GPX 1.1**: Import mit Zuordnung über `HWNnnn`-Namen, Export von Wegpunkten
  und Track aus der echten Weggeometrie, deterministische Ausgabe, Escaping von
  Sonderzeichen.
- **WebAssembly-ABI** ohne `wasm-bindgen`: vier C-Funktionen, JSON-Protokoll,
  layoutgenaue Allokation, Fehlerbehandlung ohne Panic über die Grenze.
- **Oberfläche** als statische Seite ohne Framework und ohne Bundler:
  Leaflet-Karte mit 222 Markern, Suche, Auswahl, Parkplatzliste, Planung,
  Gruppen, GPX-Export und -Import, Datenexport als JSON, IndexedDB-Persistenz,
  PWA-Manifest und Service Worker.
- **`cargo xtask`** als einziger Einstiegspunkt: `ci`, `fmt`, `clippy`, `test`,
  `data`, `examples`, `wasm`, `web`, `serve` sowie Prüfungen für Architektur,
  Lizenzen, GPX und Geheimnisse.
- **Browsertest** (`scripts/smoke-test.mjs`), der die Seite in einem echten
  Chromium über das DevTools-Protokoll prüft — inklusive Routingergebnis,
  Parkplatzliste, fehlgeschlagener Same-Origin-Anfragen und Ausnahmen. Mit
  `EXPECT_NETWORK=1` werden Routing und Parkplatzsuche streng geprüft.
- **CI/CD**: `ci.yml`, `security.yml` (cargo audit, cargo deny, Dependabot),
  `release.yml` (manuell, mit Prüfsummen) und `deploy-pages.yml` (nur `main`,
  geschütztes Environment).
- **Dokumentation**: README, Architektur, Beitragsleitfaden, Verhaltenskodex,
  Sicherheitsrichtlinie, Attribution, Roadmap, Entwicklungsstatus,
  Routing-Design, Datenquellen, Deployment, Entwicklungsumgebung.
- **GitHub-Community-Dateien**: Issue-Vorlagen für Fehler, Funktionswünsche,
  Datenfehler und Routingprobleme, Pull-Request-Vorlage, Dependabot.

### Behoben

- **Parkplatzsuche bei Überlastung von Overpass.** Der öffentliche Dienst
  antwortet bei ausgelastetem Backend innerhalb von Sekunden mit HTTP 502/504.
  Das wurde als „Parkplatzsuche fehlgeschlagen (HTTP 502)“ angezeigt — ohne
  Hinweis, dass es vorübergehend ist. Jetzt werden 5xx wie Auslastung behandelt:
  bis zu drei Versuche mit Pause, danach eine Meldung, die zum erneuten Klick
  auffordert. Verifiziert mit erzwungenem 502 (Meldung nach 4,1 s, Knopf wieder
  aktiv).
- **Veraltete Dateien nach einem Deployment.** Der Service Worker lieferte
  statische Dateien cache-first, sodass zurückkehrende Besucher nach einem
  Deployment weiter die alte `app.js` bekamen. Jetzt network-first mit
  Cache-Rückfall für den Offline-Betrieb; Cache-Version auf `v2` erhöht.

### Bekannte Einschränkungen

- Routing und Parkplatzsuche brauchen eine Internetverbindung: sie nutzen
  öffentliche Dienste (BRouter, Overpass). Ohne Netz bleibt die
  Luftlinien-Schätzung; der GPX-Export enthält dann keinen Track.
- Die Reihenfolge der Stempel wird auf der geschätzten Distanz berechnet, nicht
  auf der Wegematrix; die Tagesgrenzen werden anschließend nachjustiert.
- Parkplätze sind OpenStreetMap-Einträge, keine amtlichen Wanderparkplätze.
- Die Bedienoberfläche funktioniert offline, Kartenkacheln und Routing nicht.
- GitHub Pages, Discussions und Branch-Protection sind im Repository noch nicht
  aktiviert; siehe `docs/deployment.md`.

### Sicherheit

- Minimalrechte für `GITHUB_TOKEN` in allen Workflows, externe Actions über
  Commit-SHAs gepinnt.
- Keine Geheimnisse im Repository; die Anwendung benötigt keine.
- GPX-Importe werden als nicht vertrauenswürdige Eingabe validiert.

[Unveröffentlicht]: https://github.com/rabvz/harz-stamp-route-planner/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/rabvz/harz-stamp-route-planner/releases/tag/v0.1.0
