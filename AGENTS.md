# AGENTS.md — verbindliche Anweisungen für KI-Agenten

Dieses Repository wird von Menschen **und** KI-Agenten gepflegt. Diese Datei ist
für Agenten verbindlich. Wenn eine Anweisung hier mit einer allgemeinen
Gewohnheit kollidiert, gilt diese Datei.

## 1. Pflichtablauf für jede Aufgabe

Jeder Agent arbeitet diese acht Schritte in dieser Reihenfolge ab. Kein Schritt
darf übersprungen, keiner vorgezogen werden.

1. **Projektstatus lesen.** `docs/development-status.md` und `CHANGELOG.md`
   lesen. Dort steht, was fertig, offen oder blockiert ist.
2. **Architekturregeln prüfen.** `ARCHITECTURE.md` lesen, insbesondere die
   Schichttabelle und die erlaubten Abhängigkeitsrichtungen.
3. **Nächste Aufgabe identifizieren.** Aus `docs/roadmap.md` oder dem Auftrag.
   Bei Unklarheit fragen, nicht raten.
4. **Betroffene Module untersuchen.** Die tatsächlichen Dateien lesen. Keine
   Aussage über Code, der nicht gelesen wurde.
5. **Begrenzten Plan schreiben.** Welche Dateien, welche Tests, welches Risiko.
   Ein Plan, der mehr als ein Modul gleichzeitig umbaut, ist zu groß.
6. **Implementieren.** Nur die geplanten Dateien. Keine unaufgeforderten
   Umformatierungen, keine Umbenennungen quer durch das Repository.
7. **Tests ausführen.** Mindestens `cargo xtask ci`. Bei Änderungen an der
   Oberfläche zusätzlich `node scripts/smoke-test.mjs`.
8. **Ergebnisse dokumentieren.** `CHANGELOG.md` und, wenn sich der Status
   ändert, `docs/development-status.md` aktualisieren.

## 2. Harte Regeln

- **Kein direkter Push auf `main`.** Änderungen laufen über Feature-Branches und
  Pull Requests. Kein Force-Push auf `main`.
- **Ein Thema pro Pull Request.** Keine Änderung an mehreren Crates, wenn sie
  nicht fachlich zusammengehört. Unkoordinierte Paralleländerungen sind zu
  unterlassen.
- **Keine erfundenen Daten.** Koordinaten, Stempelstellen, Parkplätze und
  Weggeometrie werden nie geschätzt und als Tatsache ausgegeben. Fehlt eine
  Quelle, wird die Lücke dokumentiert.
- **Keine unfertigen Funktionen als fertig darstellen.** Statusaussagen in
  README, Oberfläche und Changelog müssen dem tatsächlichen Zustand entsprechen.
- **Keine Geheimnisse.** Keine API-Schlüssel, Token oder Zugangsdaten im
  Repository, auch nicht in Kommentaren, Tests oder Beispielen.
- **Kein fremder Code ohne Lizenzprüfung.** Übernommene Dateien nur mit
  kompatibler Lizenz und Eintrag in `ATTRIBUTION.md`.
- **SPDX-Kopfzeile** in jeder neuen `.rs`-Datei:
  `// SPDX-License-Identifier: MIT OR Apache-2.0`.
- **Architekturgrenzen nicht aufweichen.** Eine neue Kante im Abhängigkeitsgraph
  ist eine Architekturentscheidung und braucht einen eigenen Pull Request
  inklusive Anpassung von `ARCHITECTURE.md` und der Tabelle in
  `xtask/src/checks.rs`.
- **Werkzeuge statt Behauptungen.** Qualität wird über `cargo xtask ci`
  belegt, nicht über Prosa.

## 3. Was ein Agent nicht ohne Rückfrage tut

- öffentliche Repositories, Releases oder Deployments anlegen oder verändern,
- Branch-Protection oder Repository-Einstellungen ändern,
- neue externe Abhängigkeiten einführen (auch kleine),
- die Toolchain-Version in `rust-toolchain.toml` anheben,
- Lizenzen ändern,
- Datenquellen austauschen oder den Datensatz neu erzeugen, wenn sich dadurch
  Koordinaten oder Stempelnummern ändern.

## 4. Prüfbefehle

```bash
cargo xtask ci                 # alles, was CI prüft
cargo xtask check-architecture # nur die Schichtregeln
cargo xtask check-licenses     # SPDX-Kopfzeilen und Lizenzmetadaten
cargo xtask check-gpx          # GPX-Rundlauf gegen echte Daten
cargo xtask data --check       # Katalog passt zur Quelle?
```

Für die Oberfläche:

```bash
cargo xtask web
cargo xtask serve &
node scripts/smoke-test.mjs http://127.0.0.1:8080
```

## 5. Wenn etwas blockiert ist

Blockierte Arbeit wird in `docs/development-status.md` unter „Blockiert“
eingetragen, mit dem konkreten Hindernis (fehlende Berechtigung, fehlende
Datenquelle, fehlende Entscheidung). Ein Agent, der blockiert ist, erfindet
keinen Ersatz und markiert die Funktion auch nicht als erledigt.

## 6. Definition of Done

Eine Aufgabe ist fertig, wenn:

1. `cargo xtask ci` erfolgreich läuft,
2. neue oder geänderte Logik Tests hat, die das Verhalten prüfen (nicht nur die
   Existenz eines Ergebnisses),
3. die Dokumentation den neuen Zustand beschreibt,
4. `CHANGELOG.md` einen Eintrag hat,
5. keine neuen Warnungen, keine `unsafe`-Blöcke außerhalb von
   `wasm-bindings/src/abi.rs` und keine neuen externen Abhängigkeiten ohne
   Begründung entstanden sind.
