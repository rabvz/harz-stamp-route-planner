# Harzer Wandernadel Tourenplaner

Ein Tourenplaner für die **Harzer Wandernadel**: Stempelstellen auswählen, Tage
einteilen, Wanderparkplätze finden und die Tour als GPX exportieren. Die
Berechnung läuft vollständig im Browser als Rust/WebAssembly-Modul; es gibt
keinen Server, kein Konto und keine Telemetrie.

[![CI](https://github.com/rabvz/harz-stamp-route-planner/actions/workflows/ci.yml/badge.svg)](https://github.com/rabvz/harz-stamp-route-planner/actions/workflows/ci.yml)
[![Lizenz: MIT OR Apache-2.0](https://img.shields.io/badge/Lizenz-MIT%20OR%20Apache--2.0-blue.svg)](#lizenz)

> **Projektstatus: Alpha.** Der Planer rechnet, exportiert und läuft als
> statische Seite. Was noch fehlt, steht ungeschönt unter
> [Grenzen](#grenzen-und-offene-punkte) — insbesondere echte Weggeometrie ohne
> konfigurierten Routing-Dienst und ein mitgeliefertes Parkplatzverzeichnis.

![Screenshot des Planers: Karte mit Stempelstellen, Auswahl und berechneter Tour](docs/images/screenshot.png)

## Funktionen

| Funktion | Status | Anmerkung |
| --- | --- | --- |
| 222 Stempelstellen mit Koordinaten | **fertig** | aus der amtlichen GPX-Datei, Stand 23.04.2025 |
| Interaktive Karte mit Auswahl | **fertig** | Klick auf Marker oder Liste |
| Suche und Filter | **fertig** | nach Name, Ort und HWN-Nummer |
| Automatische Gruppierung | **fertig** | deterministisch, mit Durchmesser- und Stempel-Limit |
| Tourenoptimierung | **fertig** | Nearest-Neighbour + 2-opt, Tageslimit |
| Tageseinteilung mit Warnungen | **fertig** | überschreitbare Tage werden gemeldet, nicht verschwiegen |
| GPX-Export (Wegpunkte) | **fertig** | GPX 1.1, deterministisch |
| GPX-Export (Track) | **eingeschränkt** | nur mit konfiguriertem Routing-Dienst |
| GPX-Import | **fertig** | ordnet `HWNnnn`-Namen Stempelstellen zu |
| Parkplatzsuche | **eingeschränkt** | Startpunkt per Kartenklick; kein mitgeliefertes Verzeichnis |
| Offline-Bedienung (PWA) | **fertig** | Oberfläche und Rechenkern; **Karte nicht** |
| Persönliche Daten lokal | **fertig** | IndexedDB, Export und Import als JSON |

## Live-Demo

Geplant unter <https://rabvz.github.io/harz-stamp-route-planner/>. Der
Deployment-Workflow (`.github/workflows/deploy-pages.yml`) baut die Seite bei
jedem Push auf `main`; solange GitHub Pages im Repository nicht aktiviert ist,
ist die Adresse nicht erreichbar.

## Schnellstart für Entwickler

Voraussetzungen: ein Rust-Toolchain (siehe `rust-toolchain.toml`) und – für den
optionalen Browser-Smoke-Test – Node.js 22.

```bash
cargo xtask ci        # alle Prüfungen: fmt, clippy, Tests, Daten, WASM, Web
cargo xtask web       # baut dist/ (statische Seite inklusive WASM)
cargo xtask serve     # liefert dist/ auf http://127.0.0.1:8080
```

`cargo xtask help` listet alle Befehle. Weitere Hinweise, auch zur Installation
einer werkseigenen Toolchain ohne Systemänderung, stehen in
[docs/development.md](docs/development.md).

### Browser-Smoke-Test

`cargo test` prüft den Rust-Kern, nicht die Seite. Der folgende Test startet ein
Headless-Chromium und prüft Start, Auswahl, Planung, Gruppierung und GPX-Export:

```bash
cargo xtask web
cargo xtask serve      # in einem zweiten Terminal
node scripts/smoke-test.mjs http://127.0.0.1:8080
```

## Aufbau

Die Crates sind nach fachlicher Verantwortung getrennt; Abhängigkeiten zeigen
nur in eine Richtung:

```text
crates/
  geo-engine/        Koordinaten, Distanzen, Bounding-Boxes, Polylinien
  stamp-core/        Stempelmodell, Katalog, Auswahl
  parking-search/    Ranking von Parkplatzkandidaten
  stamp-clustering/  Gruppierung in Tagestouren
  routing-client/    Provider-Abstraktion, GeoJSON-Parser, Cache
  route-optimizer/   Reihenfolge, 2-opt, Tageseinteilung
  gpx/               GPX lesen und schreiben
  wasm-bindings/     JSON-ABI für den Browser
xtask/               Entwickleraufgaben (ci, test, wasm, web, data, ...)
web/                 statische Oberfläche (HTML, CSS, JS, Leaflet)
```

Details und die erzwungenen Regeln: [ARCHITECTURE.md](ARCHITECTURE.md).

## Daten und Rechte

- **Stempelstellen**: amtliche GPX-Datei des Harzer Verkehrsverbands, Stand
  23.04.2025. Die Datei liegt unverändert unter `data/official/`, Herkunft und
  Prüfsumme in `data/official/PROVENANCE.json`.
- **Kartenkacheln**: OpenStreetMap, ODbL, zur Laufzeit geladen und nicht
  archiviert.
- **Leaflet**: BSD-2-Clause, lokal eingebunden.
- **Projektcode**: MIT OR Apache-2.0.

Die Projektlizenz gilt **nicht** für die Stempeldaten oder die Kartenkacheln.
Vollständige Angaben: [ATTRIBUTION.md](ATTRIBUTION.md).

## Datenschutz

Auswahl, Startpunkt und Einstellungen liegen ausschließlich im Browser
(IndexedDB). Es gibt keine Konten, keine Analyse und keine Cookies. Nach außen
gehen nur zwei Arten von Anfragen, beide optional beziehungsweise unvermeidlich
für die Karte:

1. Kartenkacheln an den konfigurierten Kachel-Server (Standard: OpenStreetMap).
2. Anfragen an einen Routing-Dienst — nur wenn einer in `web/js/config.js`
   eingetragen ist.

## Grenzen und offene Punkte

Diese Liste ist Teil der Dokumentation, nicht ein Zukunftsversprechen:

- **Weggeometrie**: Ohne konfigurierten Routing-Dienst rechnet der Planer mit
  Luftlinie × 1,25. Die Oberfläche kennzeichnet das als Schätzung, und der
  GPX-Export enthält dann bewusst keinen erfundenen Track. Wie ein Dienst
  angebunden wird, steht in [docs/routing-design.md](docs/routing-design.md).
- **Parkplätze**: Es ist kein Parkplatzverzeichnis enthalten. Startpunkte werden
  per Kartenklick gesetzt; Kandidaten aus OpenStreetMap können über die API des
  Planers eingespeist werden (`parking`-Operation).
- **Offline-Karte**: Die Bedienoberfläche und der Rechenkern funktionieren
  offline, die Kartenkacheln nicht. Ein Kachel-Archiv wäre mit der
  Nutzungsrichtlinie von OpenStreetMap nicht vereinbar.
- **Navigation**: Der Planer plant Touren, er navigiert nicht.
- **Datenaktualität**: Neue oder eingezogene Stempelstellen erscheinen erst nach
  einem neuen Datensatz (`cargo xtask data`).

## Beitragen

Fehlerberichte, Datenkorrekturen und Pull Requests sind willkommen. Bitte zuerst
[CONTRIBUTING.md](CONTRIBUTING.md) und den
[Verhaltenskodex](CODE_OF_CONDUCT.md) lesen. Änderungen laufen über Feature-
Branches und Pull Requests; `cargo xtask ci` muss grün sein.

Für KI-Agenten gilt zusätzlich [AGENTS.md](AGENTS.md).

## Roadmap

Nächste Schritte stehen in [docs/roadmap.md](docs/roadmap.md), der aktuelle
Umsetzungsstand in [docs/development-status.md](docs/development-status.md).

## Lizenz

Lizenziert unter **MIT OR Apache-2.0**. Sie dürfen wählen.

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)

Sofern nicht ausdrücklich anders angegeben, gilt die Lizenz nur für den
Quellcode dieses Projekts, nicht für die eingebundenen Daten und Karten.
