# Sicherheitsrichtlinie

## Unterstützte Versionen

Dieses Projekt wird als statische Webanwendung ausgeliefert. Es gibt keine
Serverkomponente und keine Datenbank. Sicherheitsupdates erscheinen als neue
Version auf GitHub Pages und als Release.

| Version | Unterstützt |
| --- | --- |
| 0.1.x | ja |

## Sicherheitsmodell in Kürze

- **Keine Geheimnisse im Client.** Eine statische Seite kann keine Schlüssel
  geheim halten. Deshalb enthält das Projekt keine API-Schlüssel; ein optionaler
  Routing-Dienst wird ohne Zugangsdaten angesprochen. Wer einen Dienst mit
  Schlüssel braucht, muss ihn über einen eigenen Proxy betreiben.
- **Keine Serverlogik.** Alles rechnet im Browser (Rust/WebAssembly). Es gibt
  keine Angriffsfläche durch eine Backend-API, weil es kein Backend gibt.
- **Keine personenbezogenen Daten auf Servern.** Auswahl und Einstellungen
  liegen in IndexedDB im Browser. Sie werden nur übertragen, wenn der Nutzer
  selbst eine Sicherungsdatei exportiert.
- **Unveränderliche Datenquelle.** Der Stempelkatalog wird zur Compile-Zeit
  eingebettet und durch eine Prüfsumme an die Quelldatei gebunden.
- **Wenig Abhängigkeiten.** Fünf direkte Rust-Crates, eine JavaScript-Bibliothek
  (Leaflet, lokal eingebunden). Kein npm-Baum, kein CDN zur Laufzeit.

## Meldung einer Schwachstelle

Bitte **kein öffentliches Issue** für Sicherheitslücken. Stattdessen:

1. GitHub Security Advisory im Repository erstellen
   („Report a vulnerability“, Tab *Security*), oder
2. eine E-Mail an die im Repository-Profil hinterlegte Kontaktadresse.

Bitte angeben: betroffene Version, Reproduktion, erwartete und beobachtete
Wirkung, und ob ein Proof of Concept existiert.

**Reaktionszeiten.** Eingegangene Meldungen werden innerhalb von sieben Tagen
gesichtet. Ein Fix wird angestrebt, sobald die Meldung bestätigt ist; es gibt
keine garantierte Frist, weil das Projekt ehrenamtlich gepflegt wird. Auf Wunsch
wird der Melder im Advisory genannt.

## Umfang

Als Schwachstelle gilt insbesondere:

- Ausführung von fremdem Code über manipulierte GPX-Dateien beim Import,
- Umgehung der Eingabevalidierung an der WebAssembly-Grenze (Speicherfehler,
  Absturz des Moduls),
- Auslesen von Daten aus dem Browser durch eine fehlerhafte
  Inhalts-Sicherheitsrichtlinie,
- Lieferkettenprobleme in Abhängigkeiten oder CI-Workflows.

Nicht als Schwachstelle gelten:

- fehlende Backend-Authentifizierung (es gibt kein Backend),
- das Fehlen von Kartenkacheln ohne Internetverbindung,
- ungenaue Distanzen bei fehlendem Routing-Dienst (dokumentierte Einschränkung),
- Angriffe, die bereits vollständigen Zugriff auf das Gerät des Nutzers
  voraussetzen.

## Härtung im Repository

Die folgenden Maßnahmen sind umgesetzt und werden in CI geprüft (siehe
`.github/workflows/security.yml`):

- `GITHUB_TOKEN` mit minimalen Rechten (`permissions: contents: read` als
  Standard in jedem Workflow),
- alle externen Actions über volle Commit-SHAs gepinnt, nicht über bewegliche
  Tags,
- Dependabot für Cargo und GitHub Actions,
- `cargo audit` für bekannte Schwachstellen in Abhängigkeiten,
- `cargo deny` für Lizenzen und erlaubte Quellen,
- keine Ausführung von Pull-Request-Code mit erhöhten Rechten; das
  Pages-Deployment läuft nur für Pushes auf `main` im geschützten Environment,
- GPX-Eingaben werden als nicht vertrauenswürdig behandelt und validiert.

## Bekannte Einschränkungen

- Der Rust-Kern wird mit `panic = "abort"` ausgeliefert. Ein interner Fehler
  beendet das WASM-Modul; die Seite zeigt dann eine Fehlermeldung, stürzt aber
  nicht ab. Details in `ARCHITECTURE.md`.
- Der Import fremder GPX-Dateien ist auf die Elemente beschränkt, die der
  Parser kennt; unbekannte Elemente werden übersprungen statt ausgewertet.
