---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Rapport de parité GA SoraFS Orchestrator

La parité déterministe multi-fetch est désormais suivie par SDK afin que les
release engineers puissent confirmer que les bytes de payload, chunk receipts,
provider reports et résultats de scoreboard restent alignés entre
implémentations. Chaque harness consomme le bundle multi-provider canonique dans
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, qui regroupe le plan SF1,
la metadata provider, le telemetry snapshot et les options d'orchestrator.

## Rust Baseline

- **Command:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Scope:** Exécute le plan `MultiPeerFixture` deux fois via l'orchestrator in-process,
  en vérifiant les bytes de payload assemblés, chunk receipts, provider reports et
  résultats de scoreboard. L'instrumentation suit aussi la concurrence de pointe
  et la taille effective du working-set (`max_parallel × max_chunk_length`).
- **Performance guard:** Chaque exécution doit finir en 2 s sur le hardware CI.
- **Working set ceiling:** Avec le profil SF1, le harness impose `max_parallel = 3`,
  donnant une fenêtre ≤ 196 608 bytes.

Sample log output:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK Harness

- **Command:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Scope:** Rejoue la même fixture via `iroha_js_host::sorafsMultiFetchLocal`,
  comparant payloads, receipts, provider reports et scoreboard snapshots entre
  exécutions consécutives.
- **Performance guard:** Chaque exécution doit finir en 2 s ; le harness imprime la
  durée mesurée et le plafond de bytes réservés (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Example summary line:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK Harness

- **Command:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Scope:** Exécute la suite de parité définie dans `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  rejouant la fixture SF1 deux fois via le bridge Norito (`sorafsLocalFetch`). Le harness vérifie
  les bytes de payload, chunk receipts, provider reports et entrées de scoreboard en utilisant les
  mêmes metadata provider déterministes et telemetry snapshots que les suites Rust/JS.
- **Bridge bootstrap:** Le harness décompresse `dist/NoritoBridge.xcframework.zip` à la demande et charge
  le slice macOS via `dlopen`. Lorsque l'xcframework est manquant ou n'a pas les bindings SoraFS, il
  bascule sur `cargo build -p connect_norito_bridge --release` et se lie à
  `target/release/libconnect_norito_bridge.dylib`, sans setup manuel en CI.
- **Performance guard:** Chaque exécution doit finir en 2 s sur le hardware CI ; le harness imprime la
  durée mesurée et le plafond de bytes réservés (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Example summary line:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python Bindings Harness

- **Command:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Scope:** Exerce le wrapper haut niveau `iroha_python.sorafs.multi_fetch_local` et ses dataclasses typées
  afin que la fixture canonique passe par la même API que les consommateurs de wheel. Le test reconstruit
  la metadata provider depuis `providers.json`, injecte le telemetry snapshot et vérifie les bytes de payload,
  chunk receipts, provider reports et contenu du scoreboard comme les suites Rust/JS/Swift.
- **Pre-req:** Exécutez `maturin develop --release` (ou installez la wheel) pour que `_crypto` expose le binding
  `sorafs_multi_fetch_local` avant d'invoquer pytest ; le harness auto-skip lorsque le binding est indisponible.
- **Performance guard:** Même budget ≤ 2 s que la suite Rust ; pytest logge le nombre de bytes assemblés
  et le résumé de participation des providers pour l'artefact de release.

Le release gating doit capturer le summary output de chaque harness (Rust, Python, JS, Swift) afin que
le rapport archivé puisse comparer receipts de payload et métriques de manière uniforme avant de
promouvoir un build. Exécutez `ci/sdk_sorafs_orchestrator.sh` pour lancer chaque suite de parité
(Rust, Python bindings, JS, Swift) en une seule passe ; les artefacts CI doivent joindre l'extrait
log de ce helper plus le `matrix.md` généré (table SDK/status/durée) au ticket de release afin que
les reviewers puissent auditer la matrice de parité sans relancer la suite localement.
