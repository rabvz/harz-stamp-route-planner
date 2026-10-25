<!--
  Danke für den Pull Request. Bitte die Checkliste ausfüllen; sie ist Teil der
  Prüfung und keine Formalie.
-->

## Was ändert dieser Pull Request?

<!-- Ein bis drei Sätze. Was war vorher, was ist jetzt? -->

## Warum?

<!--
  Bezug zum Issue („Closes #12“), oder die Begründung, falls es keines gibt.
  Bei Architektur- oder Datenänderungen bitte ausführlicher.
-->

## Art der Änderung

- [ ] Fehlerbehebung
- [ ] Neue Funktion
- [ ] Änderung an Daten oder Datensatz (`data/official/`, `data/catalog/`)
- [ ] Dokumentation
- [ ] Werkzeuge / CI
- [ ] Architektur (Abhängigkeiten zwischen Crates)

## Betroffene Crates

<!-- Zutreffendes ankreuzen, damit klar ist, wie breit die Änderung ist. -->

- [ ] `geo-engine`
- [ ] `stamp-core`
- [ ] `parking-search`
- [ ] `stamp-clustering`
- [ ] `routing-client`
- [ ] `route-optimizer`
- [ ] `gpx`
- [ ] `wasm-bindings`
- [ ] `xtask`
- [ ] `web/`
- [ ] Dokumentation / Metadaten

## Test-Checkliste

- [ ] `cargo xtask ci` läuft lokal ohne Fehler durch
- [ ] Neue oder geänderte Logik hat Tests, die **Verhalten** prüfen
      (Entfernungen, Reihenfolgen, Grenzen – nicht nur „Ergebnis vorhanden“)
- [ ] Bei Änderungen an der Oberfläche: `node scripts/smoke-test.mjs` geprüft
- [ ] Bei Änderungen am Datensatz: `cargo xtask data --check` ist grün
- [ ] Bei Änderungen an Abhängigkeiten: `cargo deny check` geprüft und die
      Begründung unten eingetragen

## Dokumentation

- [ ] `CHANGELOG.md` ergänzt
- [ ] `docs/development-status.md` aktualisiert, falls sich der Status ändert
- [ ] `README.md` oder `docs/` angepasst, falls sich Verhalten oder Grenzen ändern
- [ ] `ATTRIBUTION.md` ergänzt, falls neue Daten oder Bibliotheken dazukommen
- [ ] `ARCHITECTURE.md` und `xtask/src/checks.rs` angepasst, falls sich die
      Schichtregeln ändern

## Sicherheit und Rechte

- [ ] Keine Geheimnisse, Schlüssel oder Zugangsdaten enthalten
- [ ] Kein fremder Code ohne kompatible Lizenz übernommen
- [ ] Neue Abhängigkeiten sind begründet (Name, Zweck, Lizenz, Alternative)
- [ ] Daten stammen aus einer genannten, zulässigen Quelle

## Anmerkungen für die Prüfung

<!--
  Was ist riskant? Wo sollte die prüfende Person genauer hinsehen?
  Was wurde bewusst nicht gemacht?
-->
