# Deployment

Die Anwendung ist eine statische Seite. Sie braucht keinen Server, keine
Datenbank, keine Umgebungsvariablen und keine Geheimnisse. Genau deshalb passt
sie zu GitHub Pages.

## 1. GitHub Pages

### Was der Workflow tut

`.github/workflows/deploy-pages.yml` läuft bei jedem Push auf `main` und von Hand:

1. Repository auschecken,
2. Rust-Toolchain gemäß `rust-toolchain.toml` einrichten (inklusive
   `wasm32-unknown-unknown`),
3. Caches für Cargo und Rustup wiederherstellen,
4. `cargo xtask ci` ausführen — das Deployment wird nicht gebaut, wenn eine
   Prüfung fehlschlägt,
5. `cargo xtask web` ausführen und `dist/` als Pages-Artefakt hochladen,
6. im Environment `github-pages` veröffentlichen.

Der Workflow hat ausschließlich die Rechte, die er braucht:

```yaml
permissions:
  contents: read
  pages: write
  id-token: write
```

`contents: read` gilt für den Build; `pages: write` und `id-token: write` nur im
Deploy-Job.

### Repository-Einstellungen

Diese Schritte liegen außerhalb des Codes. Stand 2026-10-25 sind sie
eingerichtet:

| Einstellung | Wert |
| --- | --- |
| Pages | Quelle: GitHub Actions, aktiv |
| Environment `github-pages` | vorhanden; kein Required Reviewer |
| Workflow-Berechtigungen | in allen Workflows explizit auf `contents: read` gesetzt |
| Branch-Protection `main` | Pull Request verpflichtend, Force-Push verboten, Löschen verboten, Unterhaltungen müssen aufgelöst sein, gilt auch für Admins |
| Beschreibung, Website, Themen | im Repository hinterlegt |
| Discussions | aktiviert; GitHub legt die Standardkategorien an |
| Secret scanning und Push protection | aktiviert |
| Dependabot-Alerts und Security-Updates | aktiviert |
| Secret-scanning validity checks | im Repository nicht verfügbar (Status `disabled`) |
| Branches nach dem Merge löschen | aktiviert |

Ein Required Reviewer für `github-pages` wäre zusätzlicher Schutz. Er ist
bewusst nicht gesetzt, damit ein Solo-Betrieb Deployments nicht blockiert.

#### Issues, Pull Requests und Discussions

| Einstellung | Ort | Wert |
| --- | --- | --- |
| Issues | Settings → General → Features | aktiviert |
| Pull Requests | dito | aktiviert (nicht abschaltbar) |
| Discussions | dito | aktiviert, Standardkategorien (u. a. Q&A, Ideas) |
| Wiki | dito | nicht nötig, Dokumentation liegt in `docs/` |
| Secret scanning, Push protection | Settings → Code security | aktiviert |
| Dependabot alerts, security updates | dito | aktiviert |

#### Labels

Diese Labels werden von den Vorlagen in `.github/ISSUE_TEMPLATE/` und von
`dependabot.yml` erwartet; sie sind im Repository angelegt:

| Label | Farbe | Bedeutung |
| --- | --- | --- |
| `bug` | `#d73a4a` | Fehler |
| `data` | `#0e8a16` | Stempel- oder Datensatzfehler |
| `routing` | `#1d76db` | Wege, Distanzen, Tagestour |
| `enhancement` | `#a2eeef` | Funktionswunsch |
| `documentation` | `#0075ca` | Dokumentation |
| `accessibility` | `#f143ab` | Barrierefreiheit |
| `triage` | `#fbca04` | noch nicht gesichtet |
| `good first issue` | `#7057ff` | gut geeigneter Einstieg |
| `help wanted` | `#008672` | Unterstützung gesucht |
| `attribution` | `#5319e7` | Rechte- oder Quellenfrage |
| `dependencies` | `#0366d6` | von Dependabot gesetzt |

Geeignete Einstiegspunkte für `good first issue`: Dokumentationskorrekturen,
Übersetzung der Oberfläche, ein zusätzlicher Testfall für den GPX-Import, eine
bessere Fehlermeldung beim Import fremder Dateien.

#### Branch-Protection für `main`

Eingestellt (Settings → Branches beziehungsweise Rules):

- Ziel: `main`
- „Require a pull request before merging“ — **ohne** Pflichtzustimmung, damit
  der Solo-Betrieb arbeitsfähig bleibt
- „Dismiss stale pull request approvals when new commits are pushed“
- „Block force pushes“
- „Restrict deletions“
- „Require conversation resolution before merging“
- „Do not allow bypassing the above settings“ (gilt auch für Admins)

Statusprüfungen sind **nicht** als Pflicht hinterlegt. Ein falsch benannter
Kontext würde sonst jeden Pull Request blockieren. Sobald die Checks stabil sind,
lassen sich `ci (ubuntu-latest)`, `ci (windows-latest)`, `browser smoke test`,
`dependency advisories`, `committed credentials` und `licences and sources`
nachtragen. Die Namen stammen aus `.github/workflows/ci.yml` und
`.github/workflows/security.yml`.

### URL

Die Seite ist unter

```text
https://rabvz.github.io/harz-stamp-route-planner/
```

erreichbar. Die Anwendung nutzt ausschließlich relative Pfade und funktioniert
deshalb auch in einem Unterverzeichnis. Es ist kein `base`-Pfad zu
konfigurieren, weil es keinen Bundler gibt.

## 2. Andere statische Hoster

Jeder Hoster, der Dateien ausliefert, funktioniert: Netlify, Cloudflare Pages,
ein eigener nginx, ein S3-Bucket.

```bash
cargo xtask web      # erzeugt dist/
```

Den Inhalt von `dist/` hochladen. Zwei Dinge müssen stimmen:

| Anforderung | Warum |
| --- | --- |
| `application/wasm` für `.wasm` | sonst schlägt `WebAssembly.instantiateStreaming` fehl (der Code fällt dann auf `arrayBuffer` zurück, aber langsam) |
| `index.html` als 404-Fallback | die Seite ist eine SPA; `cargo xtask web` kopiert `index.html` bereits nach `404.html` |

Bei einem Unterpfad muss der Service-Worker-Scope passen; das ist über relative
Pfade abgedeckt.

## 3. Lokale Prüfung vor dem Deployment

```bash
cargo xtask ci
cargo xtask web
cargo xtask serve
node scripts/smoke-test.mjs http://127.0.0.1:8080
```

Der letzte Schritt startet ein Headless-Chromium und prüft Start, Auswahl,
Stempelpass, Planung, Routing, Höhenprofil, Track-Bearbeitung, Gruppierung,
GPX-Export, Manifest und die Persistenz nach einem Neuladen. Er ersetzt keinen
Test auf einem echten Mobilgerät, fängt aber die Fehlerklasse ab, die
`cargo test` nicht sieht (WebAssembly-Grenze, fehlende Datei, falscher
MIME-Typ).

## 4. Releases

`.github/workflows/release.yml` wird **nur von Hand** ausgelöst
(`workflow_dispatch`) und nur für einen Tag, der dem Muster `v*` folgt, zum
Beispiel `v0.1.0`. Es entsteht kein Release pro Commit.

Der Workflow:

1. prüft aus, baut und testet (`cargo xtask ci`),
2. baut `dist/`, packt es als `harz-stamp-route-planner-<tag>.zip`,
3. erzeugt `SHA256SUMS`,
4. hängt Beispieldateien an (`examples/*.gpx`),
5. veröffentlicht ein GitHub Release mit Notizen aus `CHANGELOG.md`.

### Ablauf für ein Release

1. `CHANGELOG.md` und `docs/development-status.md` aktualisieren,
2. Version in `Cargo.toml` (`workspace.package.version`) anheben,
3. `cargo xtask ci` lokal,
4. Commit, Pull Request, Merge,
5. Tag setzen und pushen:
   ```bash
   git tag -a v0.1.0 -m "Version 0.1.0"
   git push origin v0.1.0
   ```
6. Release-Workflow mit diesem Tag starten.

## 5. Was dieses Projekt nicht deployt

- **kein Backend**, keine Datenbank, keine Warteschlange,
- **keine Geheimnisse**: es gibt keine `.env`, keine Repository-Secrets für den
  Betrieb. Der Workflow braucht nur `GITHUB_TOKEN`,
- **keine Telemetrie** und keine Analyse-Dienste.

## 6. Fehlersuche

| Symptom | Ursache | Abhilfe |
| --- | --- | --- |
| „Rechenkern: Fehler“ | WASM fehlt oder ABI-Version passt nicht | `cargo xtask web` erneut; Browser-Konsole prüfen |
| Karte bleibt grau | Kachelserver blockiert oder offline | erwartetes Verhalten ohne Netz; Attribution bleibt sichtbar |
| Seite lädt, aber keine Stempel | Katalog nicht eingebettet | `cargo xtask data` und neu bauen |
| Deployment bricht ab | `cargo xtask ci` rot | Fehler lokal reproduzieren; Pages veröffentlicht nichts |
| 404 auf Unterseiten | Hoster liefert kein `404.html` | Hoster konfigurieren oder Hash-Navigation verwenden (die App nutzt keine Pfade) |
