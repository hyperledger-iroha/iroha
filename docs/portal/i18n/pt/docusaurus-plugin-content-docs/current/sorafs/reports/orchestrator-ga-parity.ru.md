---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Selecione a partição GA para o orquestrador SoraFS

Determinar o parâmetro multi-fetch que está disponível no SDK, é o que você precisa
engenheiros de lançamento podem usar, quais bytes de carga útil, recebimentos de pedaços, provedor
relatórios e placar de resultados остаются согласованными между реализациями.
O chicote de fios использует канонический pacote multi-provedor de
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, который включает plano SF1,
metadados do provedor, instantâneo de telemetria e orquestrador de operações.

## Linha de base de ferrugem

- **Comando:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Escopo:** Запускает план `MultiPeerFixture` дважды через orquestrador em processo,
  prover bytes de carga útil, recibos de pedaços, relatórios de provedores e resultados
  placar. A ferramenta também permite obter picos de confiabilidade e eficiência
  conjunto de trabalho de tamanho (`max_parallel × max_chunk_length`).
- **Proteção de desempenho:** O programa dura cerca de 2 s no hardware CI.
- **Teto do conjunto de trabalho:** Para o chicote de fios SF1 применяет `max_parallel = 3`,
  давая окно ≤ 196608 bytes.

Exemplo de saída de registro:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Equipamento do SDK JavaScript

- **Comando:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Escopo:** Проигрывает ту же fixture через `iroha_js_host::sorafsMultiFetchLocal`,
  сравнивая cargas úteis, recibos, relatórios de provedores e instantâneos de placar
  последовательными запусками.
- **Proteção de desempenho:** Каждый запуск должен завершиться за 2 s; arnês печатает
  измеренную длительность и потолок bytes reservados (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Exemplo de linha de resumo:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Equipamento do SDK Swift

- **Comando:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Escopo:** Baixe o conjunto de paridade de `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  O dispositivo de fixação SF1 é instalado na ponte Norito (`sorafsLocalFetch`). Arnês testado
  bytes de carga útil, recibos de pedaços, relatórios de provedores e placar de entradas, используя ту же
  Determina metadados do provedor e instantâneos de telemetria, isso e suítes Rust/JS.
- **Bootstrap da ponte:** Harness распаковывает `dist/NoritoBridge.xcframework.zip` по требованию и
  загружает macOS slice через `dlopen`. Когда xcframework отсутствует ou não содержит ligações SoraFS,
  use substituto para `cargo build -p connect_norito_bridge --release` e link com
  `target/release/libconnect_norito_bridge.dylib`, não instalado no CI.
- **Proteção de desempenho:** Каждый запуск должен завершиться 2 s no hardware CI; arnês печатает
  измеренную длительность и потолок bytes reservados (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Exemplo de linha de resumo:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Chicote de ligações Python- **Comando:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Escopo:** Проверяет wrapper de alto nível `iroha_python.sorafs.multi_fetch_local` e digitado
  dataclasses, чтобы каноническая fixture são fornecidos por sua API, что вызывают
  roda dos consumidores. Teste os metadados do provedor de transferência de `providers.json`, inativo
  instantâneo de telemetria e prove bytes de carga útil, recibos de pedaços, relatórios de provedores e
  O placar também é compatível com as suítes Rust/JS/Swift.
- **Pré-requisito:** Abra `maturin develop --release` (ou roda de ajuste), instale `_crypto`
  открыл vinculativo `sorafs_multi_fetch_local` por pytest; chicote de fios automático,
  ligação когда недоступен.
- **Guarda de desempenho:** Тот же бюджет ≤ 2 s, что и у Rust suite; arquivo de log pytest
  собранных bytes e provedores de serviços de resumo para liberar artefato.

Release gating должен захватывать saída de resumo каждого chicote (Rust, Python, JS, Swift), чтобы
архивированный отчет мог сравнивать recibos de carga útil e métricas единообразно перед продвижением
construir. Use `ci/sdk_sorafs_orchestrator.sh` para usar seus conjuntos de paridade
(Rust, ligações Python, JS, Swift) no processo; Artefatos CI должны приложить trecho de log
deste ajudante e gerador `matrix.md` (também SDK/status/duração) para liberar ticket,
Esses revisores podem auditar a matriz de paridade de acordo com o programa local.