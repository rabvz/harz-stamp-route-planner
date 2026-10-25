# Entwicklungsumgebung

## Voraussetzungen

| Werkzeug | Version | Zweck |
| --- | --- | --- |
| Rust | gemäß `rust-toolchain.toml` (1.99.1) | Kern, Tests, WASM |
| `wasm32-unknown-unknown` | Target, in `rust-toolchain.toml` | Browser-Build |
| Node.js | 22 oder neuer | optionaler Browsertest, JS-Syntaxprüfung |
| Git | 2.40+ | |

Alle Rust-Komponenten (`rustfmt`, `clippy`, das WASM-Target) stehen in
`rust-toolchain.toml`. `rustup` installiert sie automatisch, sobald ein
Cargo-Befehl im Repository läuft.

## Erste Schritte

```bash
git clone https://github.com/rabvz/harz-stamp-route-planner.git
cd harz-stamp-route-planner
cargo xtask ci
```

Beim ersten Lauf werden Abhängigkeiten geladen und das WASM-Modul gebaut. Danach
sind weitere Läufe schnell.

## Ohne Systemänderung: werkseigene Toolchain

Wer die Rust-Installation nicht im Benutzerprofil haben möchte, kann sie in das
Arbeitsverzeichnis legen. `CARGO_HOME` und `RUSTUP_HOME` zeigen dann auf
`.toolchain/`, das in `.gitignore` steht.

```powershell
# Windows (PowerShell), aus dem Repository-Wurzelverzeichnis
New-Item -ItemType Directory -Force .toolchain | Out-Null
Invoke-WebRequest `
  https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe `
  -OutFile .toolchain/rustup-init.exe
$env:CARGO_HOME = "$PWD\.toolchain\cargo"
$env:RUSTUP_HOME = "$PWD\.toolchain\rustup"
$env:PATH = "$env:CARGO_HOME\bin;$env:PATH"
& ./.toolchain/rustup-init.exe -y --no-modify-path --profile minimal `
  --default-toolchain stable -t wasm32-unknown-unknown -c clippy -c rustfmt
```

Danach in jeder neuen Shell:

```powershell
$env:CARGO_HOME = "$PWD\.toolchain\cargo"
$env:RUSTUP_HOME = "$PWD\.toolchain\rustup"
$env:PATH = "$env:CARGO_HOME\bin;$env:PATH"
```

### Kein MSVC-Linker vorhanden

Wer kein Visual Studio und keine Build Tools installieren will, kann den
GNU-Host-Toolchain verwenden. Er bringt einen eigenen Linker mit
(`rust-mingw`, `rust-lld`) und braucht keine Systemkomponenten:

```powershell
rustup toolchain install 1.99.1-x86_64-pc-windows-gnu --profile minimal `
  -c clippy -c rustfmt -t wasm32-unknown-unknown
rustup override set 1.99.1-x86_64-pc-windows-gnu
```

`rustup override` gilt nur für dieses Verzeichnis und steht nicht im Repository.
`rust-toolchain.toml` bleibt unverändert, damit CI und andere Rechner den
normalen Host verwenden.

**Wichtig:** Der Override hängt am Verzeichnis, nicht am Repository. Nach einem
neuen `git clone` auf einem Rechner ohne MSVC-Toolchain muss er dort erneut
gesetzt werden, sonst schlägt der erste Build mit
`linker link.exe not found` fehl.

Hinweis: `rust-toolchain.toml` pinnt die Version. `rustup override` hat Vorrang
davor, aber die in der Datei genannten Targets und Komponenten werden dann nicht
automatisch geprüft — deshalb im Installationsbefehl ausdrücklich mitgeben.

## Befehle

```bash
cargo xtask help
```

| Befehl | Wirkung |
| --- | --- |
| `cargo xtask ci` | alle Prüfungen, in der Reihenfolge wie CI |
| `cargo xtask fmt` | formatieren |
| `cargo xtask fmt --check` | Formatierung prüfen |
| `cargo xtask clippy` | Clippy mit `-D warnings` |
| `cargo xtask test` | Testsuite des Workspace |
| `cargo xtask data` | Stempelkatalog aus der GPX-Quelle neu erzeugen |
| `cargo xtask data --check` | prüfen, ob der Katalog zur Quelle passt |
| `cargo xtask check-architecture` | Schichtregeln prüfen |
| `cargo xtask check-licenses` | SPDX-Kopfzeilen und Metadaten prüfen |
| `cargo xtask check-gpx` | GPX-Rundlauf gegen echte und erzeugte Daten |
| `cargo xtask wasm` | WASM-Modul nach `web/pkg/` bauen |
| `cargo xtask web` | `dist/` bauen |
| `cargo xtask serve` | `dist/` auf <http://127.0.0.1:8080> ausliefern; mit `--port N` auf einem anderen Port |

### Warum `target/xtask-runner`

`cargo xtask` baut den Runner in ein eigenes Zielverzeichnis. Mehrere Aufgaben
rufen `cargo` erneut auf; ohne Trennung würde ein verschachtelter Build die
laufende `xtask.exe` ersetzen wollen, was Windows mit „Zugriff verweigert“
ablehnt. Die Alias-Definition steht in `.cargo/config.toml`.

## Tests

```bash
cargo test --workspace            # alles
cargo test -p route-optimizer     # ein Crate
cargo test -p gpx -- --nocapture  # mit Ausgabe
```

Die Tests verwenden keine echten Zeitpunkte, keine Zufallszahlen und kein Netz.
Wo echte Stempeldaten nötig sind, wird `StampCatalog::embedded()` benutzt; wo es
um einen Algorithmus geht, stehen synthetische Fixtures in `test_support.rs`.
Beides ist bewusst: Algorithmustests sollen nicht brechen, wenn der Datensatz
aktualisiert wird.

## Oberfläche

`web/` wird nicht gebaut, sondern nur kopiert. Das WASM-Modul ist die einzige
erzeugte Datei.

```bash
cargo xtask web
cargo xtask serve
```

Die Entwicklungsdateien liegen direkt in `web/`; nach Änderungen an JS oder CSS
genügt ein Neuladen (der Service Worker ist im lokalen Betrieb aktiv — im
Zweifel *DevTools → Application → Service Workers → Update on reload*).

### Browsertest

```bash
node scripts/smoke-test.mjs http://127.0.0.1:8080
```

Sucht ein Chromium (Chrome, Edge, Chromium) oder nutzt `CHROME_PATH`, startet es
headless und prüft über das DevTools-Protokoll: Katalog geladen, 222 Marker,
Auswahl per Klick, Plan berechnet, Gruppen gebildet, GPX-Export gültig, kein
Seitenfehler.

### Screenshot für die Dokumentation

```bash
node scripts/screenshot.mjs http://127.0.0.1:8080 docs/images/screenshot.png
```

Erzeugt das Bild, das im README verwendet wird, aus der laufenden Anwendung —
kein gestelltes Mockup.

## Fehlersuche

| Symptom | Ursache | Abhilfe |
| --- | --- | --- |
| `linker 'link.exe' not found` | MSVC-Toolchain ohne Build Tools | GNU-Toolchain verwenden (oben) |
| `error: failed to remove ... xtask.exe` | Alias ohne eigenes Zielverzeichnis | `.cargo/config.toml` prüfen |
| `no default is configured` | `RUSTUP_HOME` zeigt nicht auf `.toolchain/rustup` | Umgebungsvariablen setzen |
| `module not found` im Browser | `web/pkg/route_planner.wasm` fehlt | `cargo xtask wasm` |
| `unreachable` beim Start | ABI-/Speicherfehler im WASM-Modul | `node scripts/smoke-test.mjs`; siehe `ARCHITECTURE.md` |
| Katalogtest schlägt fehl | `data/catalog` passt nicht zur Quelle | `cargo xtask data` |
