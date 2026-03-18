---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#Relatório de paridade GA do Orchestrator SoraFS

A paridade determinística de multi-fetch agora e monitorada pelo SDK para que os engenheiros de lançamento confirmem que
bytes de carga útil, recebimentos de pedaços, relatórios de provedores e resultados de placar mantidos alinhados entre
implementações. Cada chicote consome o pacote canônico multi-provedor em
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, que embala o plano SF1, metadados do provedor, instantâneo de telemetria e
opcoes do orquestrador.

## Ferrugem Base

- **Comando:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Escopo:** Executa o plano `MultiPeerFixture` duas vezes via o orquestrador em processo, verificando
  bytes de payload montados, recebimentos de pedaços, relatórios de provedores e resultados de placar. Uma instrumentação
  também acompanha a concorrência de pico e o tamanho efetivo do conjunto de trabalho (`max_parallel x max_chunk_length`).
- **Guarda de desempenho:** Cada execução deve ser concluída em 2 segundos no hardware de CI.
- **Working set teto:** Com o perfil SF1 o chicote aplica `max_parallel = 3`, resultando em uma
  janela <= 196608 bytes.

Exemplo de dito de log:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Aproveite o SDK JavaScript

- **Comando:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Escopo:** Reproduza o mesmo fixture via `iroha_js_host::sorafsMultiFetchLocal`, comparando payloads,
  recibos, relatórios de provedores e instantâneos do placar entre execuções consecutivas.
- **Performance guard:** Cada execução deve finalizar em 2 s; o arnês imprime a duração medida e o
  limite de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Exemplo de linha de resumo:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Aproveite o SDK Swift

- **Comando:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Escopo:** Executa um conjunto de paridade definido em `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  reproduzindo o fixture SF1 duas vezes pela ponte Norito (`sorafsLocalFetch`). O chicote verifica bytes de carga útil,
  chunk recibos, relatórios de provedores e entradas do placar usando os mesmos metadados determinísticos do provedor e
  instantâneos de telemetria das suítes Rust/JS.
- **Bridge bootstrap:** O chicote descompacta `dist/NoritoBridge.xcframework.zip` sob demanda e carrega o slice macOS via
  `dlopen`. Quando o xcframework está ausente ou não tem binds SoraFS, faz fallback para
  `cargo build -p connect_norito_bridge --release` e link contra `target/release/libconnect_norito_bridge.dylib`,
  então nenhum manual de configuração e necessário no CI.
- **Performance guard:** Cada execução deve terminar em 2 s no hardware de CI; o arnês imprime a duração medida e o
  limite de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Exemplo de linha de resumo:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Aproveite as ligações do Python- **Comando:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Escopo:** Exercício o wrapper de alto nível `iroha_python.sorafs.multi_fetch_local` e suas classes de dados
  dicas para que o fixture canônico passe pela mesma API que os consumidores de roda usam. O teste
  reconstroi os metadados de um provedor a partir de `providers.json`, injeta um instantâneo de telemetria e verifica
  bytes de payload, chunk recibos, relatórios de provedores e conteúdo do scoreboard igual às suítes Rust/JS/Swift.
- **Pré-requisito:** Execute `maturin develop --release` (ou instale o wheel) para que `_crypto` exponha o
  ligação `sorafs_multi_fetch_local` antes de chamar pytest; o chicote se auto-ignora quando o bind
  não esteja disponível.
- **Performance guard:** Mesmo orcamento <= 2 s da suíte Rust; pytest registra a contagem de bytes
  montados e o resumo de participação de provedores para os artistas de liberação.

O release gating deve capturar o resumo da saída de cada chicote (Rust, Python, JS, Swift) para que o
relatório arquivado pode comparar recibos de carga útil e métricas de uniforme antes de promover
hum construir. Execute `ci/sdk_sorafs_orchestrator.sh` para rodar todas as suítes de paridade (Rust, Python
binds, JS, Swift) em uma única última; artefatos de CI devem anexar o trecho de log desse helper
mais o `matrix.md` gerado (tabela SDK/status/duration) ao ticket de release para que os revisores possam
auditar a matriz de paridade sem reexecutar a suíte localmente.