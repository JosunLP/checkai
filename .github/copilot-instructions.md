# CheckAI Workspace Instructions

## Architektur

- `src/` enthält den Rust-Kern: Engine, REST-/WebSocket-API, Analyse, Export, Storage und CLI.
- `web/` ist die TypeScript/Vite-SPA. Das Build landet in `web/dist/` und wird vom Rust-Binary per `rust-embed` eingebettet.
- `wasm/` ist ein separates Crate für WebAssembly und teilt sich Kernlogik per `#[path = "../../src/..."]` mit dem Rust-Hauptprojekt. Änderungen an gemeinsamen Engine-Dateien können daher zugleich Server, Terminalmodus und npm/WASM-Paket beeinflussen.
- `npm/` verpackt das WASM-Ergebnis für Node.js. `docs/` ist die VitePress-Dokumentation.
- Die externe Agent-/Protokollreferenz liegt in `docs/AGENT.md`. Änderungen an API- oder Protokollstrukturen sollten damit konsistent bleiben.

## Bauen und Prüfen

- Rust-Checks:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features`
  - `cargo test --all-features`
  - `cargo build --release`
- Frontend (`web/`):
  - `bun install --frozen-lockfile`
  - `bun run check`
  - `bun run build`
- Dokumentation (`docs/`):
  - `bun install --frozen-lockfile`
  - `bun run docs:build`
- npm/WASM (`npm/`):
  - `bun run build`
- Wenn du `web/src/` änderst und das eingebettete UI verifizieren willst, baue `web/` neu, bevor du das Rust-Binary oder Release-Artefakte validierst.

## Konventionen

- Halte Änderungen klein und zielgerichtet. Dieses Repository kombiniert Rust-Backend, eingebettetes Frontend, WASM und Dokumentation; prüfe immer, welche Bereiche von einer Änderung mitbetroffen sind.
- `src/storage.rs` definiert ein versionsgebundenes Binärformat für Spielstände. Wenn sich das Format absichtlich ändert, braucht es eine klare Versionsanhebung und Migrationsstrategie.
- Bei dateibasierten Scans oder optionalen Datenquellen lieber Einzelfehler protokollieren und mit Teilresultaten weitermachen, wenn das Gesamtfeature trotzdem nutzbar bleibt.
- Die Codebasis verwendet in API-Pfaden häufig `Mutex<...>` mit `lock().unwrap()`. Halte Lock-Dauern kurz und vermeide zusätzliche Arbeit innerhalb kritischer Abschnitte.
- Für nutzersichtbare Texte gelten die Locale-Dateien in `locales/`. Wenn du sichtbare Rust- oder Web-Texte änderst, prüfe, ob Übersetzungen oder zumindest der englische Fallback angepasst werden müssen.
- Im Frontend gelten strenge TypeScript- und Lint-Regeln. Nutze die bestehenden Aliase und Patterns aus `web/src/`, statt neue Strukturkonventionen einzuführen.
- Verändere generierte oder Build-Ausgaben wie `target/`, `web/dist/` oder `npm/pkg/` nur, wenn die Aufgabe explizit Artefakte oder Release-Inhalte betrifft.

## Projektspezifische Stolperfallen

- `build.rs` stellt sicher, dass `web/dist/` existiert, aber ohne Frontend-Build kann das eingebettete UI trotzdem leer oder veraltet sein.
- Die Tablebase-Unterstützung darf aktuell nicht als vollständiges binäres Syzygy-Probing beschrieben werden; die Implementierung arbeitet laut Repo-Notizen derzeit mit analytischen/heuristischen Fallbacks statt echter `.rtbw`/`.rtbz`-Abfragen.
- Änderungen an gemeinsam genutzten Engine-Modulen wie `src/types.rs`, `src/game.rs`, `src/movegen.rs`, `src/eval.rs` oder `src/search.rs` sollten immer auch auf WASM-/npm-Auswirkungen geprüft werden.
- Wenn du Protokoll-, Analyse- oder Bewertungslogik änderst, halte Code, README und `docs/AGENT.md` synchron, damit externe Agenten nicht gegen veraltete Regeln integrieren.
