# Roadmap

Die Reihenfolge ist eine Absicht, kein Versprechen. Was bereits umgesetzt ist,
steht in [development-status.md](development-status.md).

## 0.1 – Grundgerüst (abgeschlossen)

- Rust-Workspace mit getrennten Crates und erzwungenen Schichtregeln
- Katalog der 222 Stempelstellen aus der amtlichen GPX-Datei
- Tourenoptimierung mit Streckenlimit und nachvollziehbaren Warnungen
- Gruppierung, GPX-Import und -Export
- Statische Oberfläche mit Karte, PWA-Grundlage
- `cargo xtask` als einziger Einstiegspunkt für alle Prüfungen
- CI, Sicherheitsprüfung, Release- und Pages-Workflow

## 0.2 – Ehrliches Routing

Ziel: echte Wanderwege statt Luftlinie, ohne Server und ohne Schlüssel im
Client.

- [x] Overpass-Anbindung für Wanderparkplätze in der Oberfläche (nur auf
      ausdrückliche Nutzeraktion, mit Attribution und Zwischenspeicherung)
- [x] Dokumentierte Referenzkonfiguration für einen öffentlichen oder
      selbst betriebenen Routing-Dienst
- [x] Tagestour statt Mehrtagesplanung: eine Tour, ein Streckenlimit, eine
      Warnung statt stillem Kürzen
- [x] Track-Bearbeitung: Wegpunkte setzen, ziehen und entfernen; Einrasten auf
      der berechneten Weggeometrie
- [x] Höhenprofil der Tagestour aus der Track-Geometrie
- [ ] Matrix-Aufbau für die Optimierung über echte Distanzen, mit Fortschritt und
      Abbruchmöglichkeit (#2)
- [ ] Anzeige, welche Stempelstellen ein Parkplatz zu Fuß erschließt (#5)
- [ ] Einrasten auf beliebige nahe Wege erweitern (#13)
- [ ] Hinweis auf die Datenherkunft bei jedem Parkplatz-Eintrag (#11)

## 0.3 – Verlässlichkeit

- [ ] Automatisierte Oberflächentests für Import, Datenexport und Fehlerfälle
      (#3)
- [ ] Test mit Screenreader und Tastaturdurchlauf (#4)
- [ ] Fehlerbericht-Feld für „Stempelstelle stimmt nicht“ direkt aus der Karte
      (#14)
- [ ] Prüfung, dass der Datensatz nach einem Operator-Update vollständig
      übernommen wird (Diff-Bericht bei `cargo xtask data`, #6)
- [ ] Benchmarks für 222 Stempel auf einem Mobilgerät (#7)
- [ ] Mobiles Layout und Touch-Bedienung auf echten Geräten prüfen (#12)

## 0.4 – Teilen und Weiterverwenden

- [ ] Tour als Link teilen (Zustand in der URL, ohne Server, #8)
- [ ] Druckansicht je Tagestour (#9)
- [ ] Optionales Backend für Mehrbenutzerbetrieb, ohne den Kern neu zu schreiben
      (die Architektur lässt es zu, siehe `ARCHITECTURE.md`, #10)

## Stempelpass

- [x] Stempel als „bereits gestempelt“ markieren, Fortschritt x/222
- [x] Filter auf ungestempelte Stempel
- [ ] Stempelpass als Nachweis exportieren (#15)

## Ideen ohne Zeitplan

- Stempelstellen nach Landkreis oder Wandergebiet filtern
- Vorschläge „alle Stempel in diesem Kartenausschnitt“
- Wanderwege aus OpenStreetMap (Wegrelationen) als Alternative zur
  Routing-Anbindung
- Übersetzung der Oberfläche (derzeit nur Deutsch)

## Nicht geplant

- **Offline-Kartenpakete.** Widerspricht der Nutzungsrichtlinie von
  OpenStreetMap; ein eigener Kachel-Dienst wäre eine eigene Infrastruktur.
- **Navigation.** Der Planer plant; das Führen während der Tour gehört in eine
  Navigations-App.
- **Konten und Cloud-Speicherung.** Der Local-First-Ansatz ist eine bewusste
  Entscheidung, kein Zwischenstand.
