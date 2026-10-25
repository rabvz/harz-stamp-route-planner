# Attribution und Nutzungsrechte Dritter

Dieses Projekt steht unter **MIT OR Apache-2.0**. Diese Lizenz gilt für den
Quellcode. Sie gilt **nicht** automatisch für die unten aufgeführten Daten,
Karten, Bibliotheken und Bezeichnungen. Wer die Anwendung betreibt oder
weiterverbreitet, muss die hier genannten Bedingungen selbst einhalten.

## 1. Stempelstellen der Harzer Wandernadel

| Feld | Wert |
| --- | --- |
| Werk | Verzeichnis der Stempelstellen mit Koordinaten (222 Einträge) |
| Herausgeber | Harzer Verkehrsverband e.V., Harzer Wandernadel |
| Quelle | GPX-Archiv, Stand 23.04.2025 |
| Bezug | <https://www.harzer-wandernadel.de/wp-content/uploads/2025/05/GPX-Daten-Stempelstellen-ZIP-Archiv-gepackt-Stand-23.04.2025.zip> |
| Ablage im Repository | `data/official/HWN2025.gpx`, unverändert |
| Prüfsumme | `sha256:3e3667b13b8b7c69c25a95709cbc9f213d5be87f79f3fdcd921a64fade838bbc` |
| Erzeugtes Derivat | `data/catalog/hwn-2025-stamps.json` (via `cargo xtask data`) |

**Rechte.** Die Datei wird als Datenquelle verwendet und unverändert
mitgeführt, damit nachvollziehbar bleibt, worauf der Katalog beruht. Eine
eigenständige Erlaubnis zur Weiterverbreitung des Stempelverzeichnisses ist mit
dieser Veröffentlichung nicht verbunden; die Rechte liegen beim Herausgeber.
Das erzeugte Derivat übernimmt nur die enthaltenen Sachangaben (Nummer, Name,
Ort, Koordinaten, Höhe) in ein anderes Format.

**Wenn Sie die Anwendung weiterverbreiten** und die Stempeldaten mitliefern,
klären Sie die Nutzung mit dem Herausgeber. Wer nur den Quellcode ohne
`data/official/` und `data/catalog/` weitergibt, ist von dieser Frage nicht
betroffen; `cargo xtask data` benötigt allerdings die Quelldatei.

**Änderungen.** Der Katalog normalisiert Weißraum in Namen und Ortsangaben
(Zeilenumbrüche aus der Quelle werden zu Leerzeichen). Koordinaten, Nummern und
Höhen werden unverändert übernommen. Die Quelldatei selbst wird nicht
verändert.

## 2. Marken und Bezeichnungen

„Harzer Wandernadel“ und die zugehörigen Bezeichnungen sind Kennzeichen des
Harzer Verkehrsverbands. Dieses Projekt ist ein **unabhängiges Werkzeug** und
steht in keiner offiziellen Verbindung zu dem Verband, wird von ihm nicht
empfohlen und nicht unterstützt. Die Bezeichnung wird ausschließlich
beschreibend verwendet (welche Stempelstellen verwaltet werden), nicht als
Herkunftsangabe für die Software.

## 3. Kartenkacheln

| Feld | Wert |
| --- | --- |
| Werk | Rasterkacheln der OpenStreetMap-Standardkarte |
| Rechteinhaber | OpenStreetMap-Mitwirkende |
| Lizenz | Open Database License (ODbL) 1.0 |
| Bezug | Standard-Kachelserver, konfigurierbar in `web/js/config.js` |
| Ablage | nicht im Repository, nicht im Service-Worker-Cache |

Die Kacheln werden zur Laufzeit geladen. Die Anwendung archiviert sie nicht und
baut kein Offline-Kartenpaket; das wäre mit der
[Tile Usage Policy](https://operations.osmfoundation.org/policies/tiles/) der
OpenStreetMap Foundation nicht vereinbar. Die Kartenattribution ist in der
Oberfläche und auf `attribution.html` sichtbar.

## 4. Routing- und Parkplatzdienste

Diese Dienste ruft der Browser zur Laufzeit auf; es ist kein fremder Code
eingebunden.

### BRouter (Wegführung)

| Feld | Wert |
| --- | --- |
| Dienst | `https://brouter.de/brouter`, Profil `hiking-beta` |
| Software | BRouter, © Armin Falkner und Mitwirkende, **MIT-Lizenz** |
| Datengrundlage | OpenStreetMap, **ODbL 1.0** |
| Bezug | <https://brouter.de/brouter/> |
| Aufrufe | eine Anfrage pro Tag, nur beim Berechnen einer Tour |

Wer den öffentlichen Dienst in größerem Umfang nutzt, sollte eine eigene Instanz
betreiben; die Software ist frei. Die Projektkonfiguration lässt das über
`web/js/config.js` zu.

### Overpass / OpenStreetMap (Wanderparkplätze)

| Feld | Wert |
| --- | --- |
| Dienst | `https://overpass-api.de/api/interpreter` |
| Daten | © OpenStreetMap-Mitwirkende, **ODbL 1.0** |
| Bezug | <https://wiki.openstreetmap.org/wiki/Overpass_API> |
| Aufrufe | nur auf Klick, Ergebnis zehn Minuten zwischengespeichert |

Die Attribution „© OpenStreetMap-Mitwirkende“ steht in der Oberfläche direkt
unter der Parkplatzliste und im Fußbereich.

## 5. Bibliotheken

### Leaflet 1.9.4

- Lizenz: BSD-2-Clause
- Copyright: 2010–2023 Volodymyr Agafonkin, 2010–2011 CloudMade
- Ablage: `web/vendor/leaflet/` (JavaScript, CSS, Lizenztext)
- Bezug: <https://leafletjs.com/>

### Rust-Crates

Direkte Abhängigkeiten und ihre Lizenzen. Alle sind mit der Doppellizenz des
Projekts verträglich; `cargo deny` prüft das in CI.

| Crate | Version | Lizenz | Zweck |
| --- | --- | --- | --- |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | Serialisierung |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 | JSON an der WASM-Grenze |
| `quick-xml` | 0.42.0 | MIT OR Apache-2.0 | GPX-Parsing |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 | Fehlertypen |
| `sha2` | 0.11.1 | MIT OR Apache-2.0 | Prüfsumme der Quelldatei (nur xtask) |

Transitive Abhängigkeiten werden nicht hier, sondern maschinell gepflegt:
`cargo deny check licenses` und die von Dependabot erzeugten Pull Requests.
Eine vollständige Liste erzeugt:

```bash
cargo tree --workspace
cargo deny check licenses
```

### Schriften und Symbole

Es werden keine externen Schriften geladen. Die Oberfläche verwendet die
Systemschrift des Geräts. Die Symbole in `web/icons/` sind eigene SVG-Zeichnungen
unter der Projektlizenz.

## 6. Bezug zu Wikipedia oder anderen Textquellen

Es wurde kein fremder Fließtext übernommen. Beschreibungen in der Oberfläche
stammen aus dem Projekt oder aus den Sachangaben der Stempeldaten.

## 7. Meldung einer fehlenden Attribution

Wenn eine Quelle fehlt oder falsch zugeordnet ist, bitte ein Issue mit dem
Label `attribution` eröffnen. Solche Meldungen werden vor anderen Aufgaben
bearbeitet.
