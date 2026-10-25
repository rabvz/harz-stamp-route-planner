# Architektur

Dieses Dokument beschreibt den Aufbau des Planers, die Regeln, die er
einhalten muss, und warum die Grenzen dort liegen, wo sie liegen. Die Regeln
sind nicht nur beschrieben, sondern maschinell geprüft
(`cargo xtask check-architecture`).

## 1. Leitgedanke

Der fachliche Kern ist eine reine Bibliothek: Koordinaten hinein, Plan heraus.
Alles, was I/O, Oberfläche oder Plattform betrifft, liegt außen. Daraus folgen
drei Eigenschaften, die das Projekt tragen:

1. **Derselbe Kern läuft überall.** Nativ in Tests, als WebAssembly im Browser,
   später unverändert hinter einem optionalen Backend.
2. **Tests brauchen keine Umgebung.** Kein Netz, kein Browser, keine Datenbank.
3. **Fehler sind Daten.** Der Kern gibt Ergebnisse und Warnungen zurück, keine
   Panics, keine Bildschirmausgaben.

## 2. Schichten

```text
                        ┌──────────────────────┐
                        │ wasm-bindings (JSON) │  ← einzige FFI-Grenze
                        └──────────┬───────────┘
             ┌─────────────────────┼─────────────────────┐
             │                     │                     │
      ┌──────▼──────┐      ┌───────▼───────┐     ┌───────▼────────┐
      │route-optimizer│    │stamp-clustering│    │ parking-search │
      └──┬────┬─────┘      └──────┬────────┘     └───────┬────────┘
         │    │                   │                      │
         │  ┌─▼──────────┐  ┌─────▼─────┐                │
         │  │routing-client│ │ stamp-core│◄───────────────┘
         │  └─────┬──────┘  └─────┬─────┘
         │        │               │
    ┌────▼────────▼───────────────▼────┐        ┌──────────┐
    │            geo-engine            │◄───────┤   gpx    │
    └──────────────────────────────────┘        └──────────┘
```

| Crate | Verantwortung | darf abhängen von |
| --- | --- | --- |
| `geo-engine` | Koordinaten, Distanzen, Bounding-Box, Projektion, Polylinien | – |
| `stamp-core` | Stempelmodell, Katalog, Auswahl | `geo-engine` |
| `parking-search` | Ranking von Parkplatzkandidaten | `geo-engine`, `stamp-core` |
| `stamp-clustering` | Gruppierung der Auswahl | `geo-engine`, `stamp-core` |
| `routing-client` | Provider-Abstraktion, GeoJSON-Parser, Cache | `geo-engine` |
| `route-optimizer` | Reihenfolge, 2-opt, Tageseinteilung | `geo-engine`, `stamp-core`, `routing-client` |
| `gpx` | GPX 1.1 lesen und schreiben | `geo-engine`, `stamp-core` |
| `wasm-bindings` | JSON-ABI, Request-Dispatch | alle oben genannten || `xtask` | Entwickleraufgaben | `gpx`, `stamp-core` |

**Verboten** und durch den Check abgesichert:

- eine Bibliothek, die von `wasm-bindings` abhängt (Umkehrung der Grenze),
- eine Bibliothek, die die Oberfläche (`web/`) kennt,
- ein `cdylib`-Target außerhalb von `wasm-bindings`,
- Zyklen.

Eine neue Kante ist eine Architekturentscheidung: eigener Pull Request,
Anpassung dieser Tabelle, Anpassung von `ALLOWED_INTERNAL_DEPENDENCIES` in
`xtask/src/checks.rs`.

## 3. Warum kein `wasm-bindgen`

Der Browser braucht genau eine Funktion: JSON hinein, JSON heraus. `wasm-bindgen`
würde dafür eine zweite Toolchain (`wasm-bindgen-cli`) einführen, deren Version
exakt zur Crate passen muss — eine zusätzliche Fehlerquelle in CI und für
Beitragende, ohne funktionalen Gewinn.

Stattdessen vier C-Funktionen (`hsr_alloc`, `hsr_free`, `hsr_invoke`,
`hsr_abi_version`). Der Loader in `web/js/wasm.js` ist rund 120 Zeilen und
kommentiert. Der typisierte Teil der API liegt in `crates/wasm-bindings/src/api.rs`
und ist normaler Rust-Code — also nativ testbar.

Die Allokation ist bewusst trivial: ein Layout (`len` Bytes, Ausrichtung 1) für
beide Richtungen. Ein früherer Entwurf gab `String`-Puffer heraus und
rekonstruierte sie mit geratener Kapazität; bei Abweichung brach das Modul mit
einem `unreachable`-Trap ab. Dieser Fehlertyp ist für `cargo test` unsichtbar,
deshalb gibt es `scripts/smoke-test.mjs`.

## 4. Datenfluss

```text
GPX der Stempelstellen          web/js/app.js                Rust/WASM
──────────────────────          ─────────────                ─────────
data/official/HWN2025.gpx
        │ cargo xtask data
        ▼
data/catalog/hwn-2025-stamps.json
        │ include_str!
        ▼
StampCatalog::embedded()  ──►  catalog()  ──►  Marker + Liste
                                   │
                    Auswahl ◄──────┘
                       │
                       ├──► clusters()  ──►  Gruppen
                       ├──► plan()      ──►  Tage, Distanzen, Warnungen
                       └──► exportGpx() ──►  GPX-Text
                                   ▲
               importGpx()  ───────┘
```

Der Katalog wird zur Compile-Zeit eingebettet. Damit braucht der Browser keine
Netzverbindung, um Stempelstellen anzuzeigen, und der Datensatz ist an die
Binärdatei gebunden — man kann nicht versehentlich mit einem anderen Katalog
rechnen, als dem getesteten.

## 5. Algorithmen und ihre Grenzen

### Optimierung

Ein Tagesausflug startet und endet am selben Parkplatz. Die Reihenfolge der
Stempel ist damit ein offener Pfad mit festem Start; mehrere Tage machen daraus
ein Fahrzeugrouting-Problem mit Kapazitätsgrenze — NP-schwer. Umgesetzt ist:

1. **Distanzmatrix** über Parkplatz und Stempel. Der Aufrufer darf sie liefern
   (echte Routingdistanzen); sonst füllt `StraightLineProvider` sie mit
   Luftlinie × 1,25 und das Ergebnis wird als Schätzung markiert.
2. **Nearest-Neighbour** für die Startreihenfolge, Gleichstand nach
   Stempelnummer.
3. **2-opt** auf dem offenen Pfad mit fixiertem Start, nur bei echter
   Verkürzung; das garantiert Termination und Konvergenz.
4. **Greedy-Tageinteilung** unter dem Tageslimit.

**Danach übernimmt die Oberfläche das Routing** (`web/js/app.js`,
`web/js/routing.js`): Jede Tagesroute geht als Liste von Wegpunkten an BRouter
zurück und liefert echte Distanz, Gehzeit und Geometrie. Weil echte Wege oft
doppelt so lang sind wie die Schätzung, werden die Tagesgrenzen anschließend
nachjustiert: Der letzte Stempel eines zu langen Tages wandert in den Folgetag,
beide Tage werden neu geroutet, höchstens acht Durchläufe. Was danach noch über
dem Limit liegt, wird gemeldet.

Bewusste Entscheidung: Der **Kern** optimiert auf der Matrix, nicht auf der
Straßengeometrie; das Routing ist ein nachgelagerter Schritt in der
Oberfläche. Eine Optimierung auf echten Distanzen wäre eine Matrix mit
`(n+1)²` Anfragen — möglich (die `plan`-Operation nimmt eine Matrix entgegen),
aber für zehn Stempel über hundert Abfragen. Die Trennung hält den Kern
testbar und die Last auf öffentlichen Diensten niedrig. Details:
`docs/routing-design.md`.

### Gruppierung

Deterministische Greedy-Expansion plus Merge-Pass. Jede Bedingung (Stempelbudget,
Durchmesser) wird **vor** dem Beitritt geprüft, die Garantien gelten also per
Konstruktion und nicht per Reparatur. Ein nachvollziehbarer Algorithmus ist hier
mehr wert als ein marginal besseres Partitionsergebnis.

### Warum Warnungen statt Fehler

Wenn eine gewählte Stempelstelle im Tageslimit nicht erreichbar ist, wäre ein
Abbruch die schlechteste Antwort: der Nutzer verliert seine Auswahl. Der Plan
wird geliefert, der Tag als `oversized` markiert und die Einschränkung benannt.
Dasselbe gilt für zu viele Tage.

## 6. Determinismus

Alle Berechnungen sind reproduzierbar:

- Sortierung und Gleichstände brechen immer auf eine stabile Größe
  (Stempelnummer, Knotenindex, Kandidaten-ID).
- Keine Hashmap-Iteration in Ergebnispfaden (Ausnahmen: interner Index, dort
  ohne Auswirkung auf die Ausgabe).
- Der GPX-Writer gibt kein `<time>` aus, solange keiner übergeben wird.
- `data/catalog/hwn-2025-stamps.json` wird aus der Quelle erzeugt;
  `cargo xtask data --check` schlägt bei Abweichung fehl.

## 7. Fehlerbehandlung

- Bibliotheken geben typisierte Fehler zurück (`thiserror`), keine Panics.
- `unsafe` ist ausschließlich in `crates/wasm-bindings/src/abi.rs` erlaubt und
  dort begründet; alle anderen Crates setzen `#![forbid(unsafe_code)]`.
- An der FFI-Grenze wird `catch_unwind` verwendet, damit ein Fehler im Kern
  nicht die Seite abreißt. Im Release-Profil (`panic = "abort"`) greift das
  nicht — deshalb prüft der Smoke-Test das echte Artefakt.

## 8. Oberfläche

`web/` ist bewusst ohne Framework und ohne Bundler: ES-Module, reines CSS,
Leaflet lokal eingebunden. Gründe: kein Build-Schritt außer `cargo xtask web`,
keine `node_modules` im Repository, keine CDN-Abhängigkeit zur Laufzeit, und die
Seite ist mit einem statischen Host (GitHub Pages) direkt lauffähig.

Der Service Worker cached die Bedienoberfläche und das WASM-Modul, **nicht** die
Kartenkacheln. Ein Kachel-Archiv wäre mit der Nutzungsrichtlinie von
OpenStreetMap nicht vereinbar; die Dokumentation sagt das offen.
