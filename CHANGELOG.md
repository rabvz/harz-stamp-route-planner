# Changelog

Alle nennenswerten Änderungen an diesem Projekt. Format nach
[Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung nach
[Semantic Versioning](https://semver.org/lang/de/).

## [Unveröffentlicht]

### Hinzugefügt

- nichts offen

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
- **Gruppierung** der Auswahl in räumlich zusammenhängende Tagestouren mit
  Durchmesser- und Stempellimit, deterministisch und ohne Verletzung der
  Garantien.
- **Parkplatz-Ranking** für von außen gelieferte Kandidaten (OpenStreetMap-
  Tauglichkeit, Abdeckung, Fußweg).
- **Routing-Abstraktion** mit Offline-Fallback (Luftlinie × 1,25), GeoJSON-Parser
  für BRouter/ORS/Valhalla und Routencache.
- **GPX 1.1**: Import mit Zuordnung über `HWNnnn`-Namen, Export von Wegpunkten
  und optionalem Track, deterministische Ausgabe, Escaping von Sonderzeichen.
- **WebAssembly-ABI** ohne `wasm-bindgen`: vier C-Funktionen, JSON-Protokoll,
  layoutgenaue Allokation, Fehlerbehandlung ohne Panic über die Grenze.
- **Oberfläche** als statische Seite ohne Framework und ohne Bundler:
  Leaflet-Karte mit 222 Markern, Suche, Auswahl, Planung, Gruppen, GPX-Export
  und -Import, Datenexport als JSON, IndexedDB-Persistenz, PWA-Manifest und
  Service Worker.
- **`cargo xtask`** als einziger Einstiegspunkt: `ci`, `fmt`, `clippy`, `test`,
  `data`, `wasm`, `web`, `serve` sowie Prüfungen für Architektur, Lizenzen und
  GPX.
- **Browsertest** (`scripts/smoke-test.mjs`), der die Seite in einem echten
  Chromium über das DevTools-Protokoll prüft.
- **CI/CD**: `ci.yml`, `security.yml` (cargo audit, cargo deny, Dependabot),
  `release.yml` (manuell, mit Prüfsummen) und `deploy-pages.yml` (nur `main`,
  geschütztes Environment).
- **Dokumentation**: README, Architektur, Beitragsleitfaden, Verhaltenskodex,
  Sicherheitsrichtlinie, Attribution, Roadmap, Entwicklungsstatus,
  Routing-Design, Datenquellen, Deployment, Entwicklungsumgebung.
- **GitHub-Community-Dateien**: Issue-Vorlagen für Fehler, Funktionswünsche,
  Datenfehler und Routingprobleme, Pull-Request-Vorlage, Dependabot.

### Bekannte Einschränkungen

- Ohne konfigurierten Routing-Dienst beruhen Distanzen auf Luftlinie × 1,25; der
  GPX-Export enthält dann keinen Track. Die Oberfläche kennzeichnet das.
- Es ist kein Parkplatzverzeichnis enthalten; Startpunkte werden per Kartenklick
  gesetzt.
- Die Bedienoberfläche funktioniert offline, die Kartenkacheln nicht.
- GitHub Pages, Discussions und Branch-Protection sind im Repository noch nicht
  aktiviert; siehe `docs/deployment.md`.

### Sicherheit

- Minimalrechte für `GITHUB_TOKEN` in allen Workflows, externe Actions über
  Commit-SHAs gepinnt.
- Keine Geheimnisse im Repository; die Anwendung benötigt keine.
- GPX-Importe werden als nicht vertrauenswürdige Eingabe validiert.

[Unveröffentlicht]: https://github.com/rabvz/harz-stamp-route-planner/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/rabvz/harz-stamp-route-planner/releases/tag/v0.1.0
