---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Relatorio de paridade GA do Orchestrator SoraFS

A paridade deterministica de multi-fetch agora e monitorada por SDK para que engenheiros de release confirmem que
bytes de payload, chunk receipts, provider reports e resultados de scoreboard permanecem alinhados entre
implementacoes. Cada harness consome o bundle canonico multi-provider em
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, que empacota o plano SF1, provider metadata, telemetry snapshot e
opcoes do orchestrator.

## Base Rust

- **Comando:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Escopo:** Executa o plano `MultiPeerFixture` duas vezes via o orchestrator in-process, verificando
  bytes de payload montados, chunk receipts, provider reports e resultados de scoreboard. A instrumentacao
  tambem acompanha a concorrencia de pico e o tamanho efetivo do working-set (`max_parallel x max_chunk_length`).
- **Performance guard:** Cada execucao deve concluir em 2 s no hardware de CI.
- **Working set ceiling:** Com o perfil SF1 o harness aplica `max_parallel = 3`, resultando em uma
  janela <= 196608 bytes.

Exemplo de saida de log:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Harness do SDK JavaScript

- **Comando:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Escopo:** Reproduz o mesmo fixture via `iroha_js_host::sorafsMultiFetchLocal`, comparando payloads,
  receipts, provider reports e snapshots do scoreboard entre execucoes consecutivas.
- **Performance guard:** Cada execucao deve finalizar em 2 s; o harness imprime a duracao medida e o
  teto de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Exemplo de linha de resumo:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harness do SDK Swift

- **Comando:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Escopo:** Executa a suite de paridade definida em `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  reproduzindo o fixture SF1 duas vezes pelo bridge Norito (`sorafsLocalFetch`). O harness verifica bytes de payload,
  chunk receipts, provider reports e entradas do scoreboard usando a mesma provider metadata deterministica e
  telemetry snapshots das suites Rust/JS.
- **Bridge bootstrap:** O harness descompacta `dist/NoritoBridge.xcframework.zip` sob demanda e carrega o slice macOS via
  `dlopen`. Quando o xcframework esta ausente ou nao tem bindings SoraFS, faz fallback para
  `cargo build -p connect_norito_bridge --release` e linka contra `target/release/libconnect_norito_bridge.dylib`,
  entao nenhum setup manual e necessario na CI.
- **Performance guard:** Cada execucao deve terminar em 2 s no hardware de CI; o harness imprime a duracao medida e o
  teto de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Exemplo de linha de resumo:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harness das bindings Python

- **Comando:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Escopo:** Exercita o wrapper de alto nivel `iroha_python.sorafs.multi_fetch_local` e suas dataclasses
  tipadas para que o fixture canonico passe pela mesma API que consumidores de wheel usam. O teste
  reconstroi a provider metadata a partir de `providers.json`, injeta a telemetry snapshot e verifica
  bytes de payload, chunk receipts, provider reports e conteudo do scoreboard igual as suites Rust/JS/Swift.
- **Pre-req:** Execute `maturin develop --release` (ou instale o wheel) para que `_crypto` exponha o
  binding `sorafs_multi_fetch_local` antes de chamar pytest; o harness se auto-ignora quando o binding
  nao estiver disponivel.
- **Performance guard:** Mesmo orcamento <= 2 s da suite Rust; pytest registra a contagem de bytes
  montados e o resumo de participacao de providers para o artefato de release.

O release gating deve capturar o summary output de cada harness (Rust, Python, JS, Swift) para que o
relatorio arquivado possa comparar receipts de payload e metricas de forma uniforme antes de promover
um build. Execute `ci/sdk_sorafs_orchestrator.sh` para rodar todas as suites de paridade (Rust, Python
bindings, JS, Swift) em uma unica passada; artefatos de CI devem anexar o trecho de log desse helper
mais o `matrix.md` gerado (tabela SDK/status/duration) ao ticket de release para que reviewers possam
auditar a matriz de paridade sem reexecutar a suite localmente.
