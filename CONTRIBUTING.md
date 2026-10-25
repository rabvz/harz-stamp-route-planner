# Beitragen

Danke, dass Sie mitwirken wollen. Dieses Projekt plant Wandertouren zu den
Stempelstellen der Harzer Wandernadel. Beiträge sind willkommen — Code,
Dokumentation, Fehlerberichte und Korrekturen an den Daten.

## Bevor Sie beginnen

- Lesen Sie den [Verhaltenskodex](CODE_OF_CONDUCT.md).
- Prüfen Sie, ob es bereits ein Issue gibt. Wenn nicht: neues Issue mit
  passendem Template.
- Bei größeren Änderungen (neue Funktion, neue Abhängigkeit, Architektur):
  **zuerst ein Issue**, damit die Richtung abgestimmt ist, bevor Arbeit
  entsteht.

## Entwicklungsumgebung

```bash
git clone https://github.com/rabvz/harz-stamp-route-planner.git
cd harz-stamp-route-planner
cargo xtask ci
```

Voraussetzungen: Rust gemäß `rust-toolchain.toml` (die Datei pinnt Version,
Komponenten und das WASM-Target; rustup richtet alles automatisch ein) und für
den optionalen Browsertest Node.js 22.

Falls auf Ihrem Rechner kein MSVC-Linker vorhanden ist, funktioniert der
GNU-Host-Toolchain mit selbst mitgeliefertem Linker — Details in
[docs/development.md](docs/development.md).

## Arbeitsweise

1. **Branch anlegen**, niemals direkt auf `main`:

   ```bash
   git switch -c feature/kurze-beschreibung
   ```

2. **Eine Sache pro Pull Request.** Ein Thema, ein Branch, ein PR.
3. **Pläne klein halten.** Wenn ein PR mehr als ein Modul umbaut, ist er zu groß.
4. **Tests schreiben**, die Verhalten prüfen — nicht nur die Existenz eines
   Ergebnisses. Bei Geometrie heißt das: konkrete Entfernungen, Reihenfolgen und
   Grenzen prüfen.
5. **`cargo xtask ci` muss grün sein.** Das ist dieselbe Prüfung, die CI
   ausführt.
6. **CHANGELOG aktualisieren**, wenn sich etwas für Nutzer ändert.

## Was wir besonders brauchen

- **Datenkorrekturen**: Eine Stempelstelle ist umgezogen, umbenannt oder neu.
  Bitte mit Quelle (Link zur offiziellen Seite oder GPX-Datum) im Issue.
- **Routing-Anbindung**: Eine dokumentierte, lizenzkonforme Anbindung echter
  Wanderwege (siehe [docs/routing-design.md](docs/routing-design.md)).
- **Parkplatzdaten**: Eine Quelle, die eine Weitergabe erlaubt, oder die
  Anbindung an OpenStreetMap/Overpass.
- **Barrierefreiheit**: Die Oberfläche ist tastaturbedienbar, aber nicht
  getestet mit Screenreadern.
- **Tests**: Randfälle in GPX-Import, Gruppierung und Tagestour.

## Stil

Rust:

- `cargo fmt` und `cargo clippy -- -D warnings` sind Pflicht.
- Jede `.rs`-Datei beginnt mit `// SPDX-License-Identifier: MIT OR Apache-2.0`.
- Öffentliche Elemente brauchen Dokumentationskommentare (`#![deny(missing_docs)]`).
- Fehler als typisierte `enum`s mit `thiserror`, keine Panics in Bibliotheken.
- `unsafe` nur in `crates/wasm-bindings/src/abi.rs`, mit Begründung
  (`// SAFETY:`).

JavaScript:

- ES-Module, keine Frameworks, keine Build-Kette außer `cargo xtask web`.
- Kein `innerHTML` mit ungefilterten Daten — `escapeHtml` verwenden.
- Kommentare auf Deutsch oder Englisch, aber einheitlich pro Datei.

Dokumentation:

- Deutsch, sachlich, ohne Werbesprache.
- Behauptungen müssen belegbar sein. Wenn etwas nicht fertig ist, steht das da.

## Pull Request

Nutzen Sie die Vorlage in `.github/PULL_REQUEST_TEMPLATE.md`. Ein PR gilt als
fertig, wenn:

- [ ] `cargo xtask ci` lokal grün ist,
- [ ] neue Logik Tests hat,
- [ ] die Dokumentation den neuen Zustand beschreibt,
- [ ] `CHANGELOG.md` ergänzt ist,
- [ ] keine neuen externen Abhängigkeiten ohne Begründung dazugekommen sind.

## Commits

Kurze, sachliche Betreffzeile im Imperativ, danach optional ein Absatz zur
Begründung:

```text
Gruppierung: Durchmesserlimit vor dem Beitritt prüfen

Bisher wurde das Limit nach dem Einfügen geprüft und der Stempel
anschließend wieder entfernt. Jetzt wird vorher geprüft, was die
Garantie unmittelbar einsichtig macht.
```

## Lizenz Ihres Beitrags

Mit einem Pull Request erklären Sie sich einverstanden, dass Ihr Beitrag unter
**MIT OR Apache-2.0** veröffentlicht wird. Es ist keine zusätzliche
Vereinbarung nötig (kein CLA).

## Fragen

Bei Unklarheiten: Issue mit dem Label `question` eröffnen. Fragen sind
ausdrücklich erwünscht — sie zeigen, wo die Dokumentation nicht reicht.
