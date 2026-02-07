---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Relatório de paridade GA do Orchestrator SoraFS

A paridade determinística de multi-fetch agora é rastreada pelo SDK para que eles
os engenheiros de lançamento podem confirmar que os bytes de carga útil, recibos de pedaços,
relatórios de provedores e resultados de placar permanentes alinhados entre
implementações. Cada chicote consome el pacote multi-provedor canonico bajo
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, que empaque o plano SF1,
metadados do provedor, instantâneo de telemetria e opções do orquestrador.

## Linha de base de ferrugem

- **Comando:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Escopo:** Execute o plano `MultiPeerFixture` duas vezes por meio do orquestrador em processo,
  verificando bytes de carga útil ensamblados, recibos de pedaços, relatórios de provedores e
  resultados do placar. A instrumentação também rastrea concorrência pico
  e o tamanho efetivo do conjunto de trabalho (`max_parallel x max_chunk_length`).
- **Proteção de desempenho:** Cada execução deve ser concluída em 2 segundos no CI de hardware.
- **Conjunto de trabalho teto:** Com o perfil SF1 e o chicote aplica `max_parallel = 3`,
  dando uma janela <= 196608 bytes.

Exemplo de saída de registro:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Equipamento do SDK JavaScript

- **Comando:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Escopo:** Reproduzir o equipamento mismo via `iroha_js_host::sorafsMultiFetchLocal`,
  comparando cargas úteis, recibos, relatórios de provedores e instantâneos de placar entre
  ejecuções consecutivas.
- **Proteção de desempenho:** Cada execução deve ser finalizada em 2 s; el arnês imprime la
  duração medida e tecnologia de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Exemplo de linha de resumo:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Equipamento do SDK Swift

- **Comando:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Escopo:** Executa o conjunto de paridade definido em `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  reproduzindo o dispositivo SF1 duas vezes ao longo da ponte Norito (`sorafsLocalFetch`). O arnês
  verifica bytes de carga útil, recibos de pedaços, relatórios de provedores e entradas de placar usando la
  mesmo provedor de metadados determinísticos e instantâneos de telemetria que são suítes Rust/JS.
- **Bootstrap da ponte:** El chicote descomprime `dist/NoritoBridge.xcframework.zip` baixa demanda e carga
  el slice macOS via `dlopen`. Se o xcframework estiver faltando ou não houver ligações SoraFS, faça um substituto para
  `cargo build -p connect_norito_bridge --release` e link contra
  `target/release/libconnect_norito_bridge.dylib`, sem manual de configuração em CI.
- **Proteção de desempenho:** Cada execução deve terminar em 2 s no hardware CI; el arnês imprime la
  duração medida e tecnologia de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Exemplo de linha de resumo:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Chicote de ligações Python- **Comando:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Escopo:** Executa o wrapper de alto nível `iroha_python.sorafs.multi_fetch_local` e suas classes de dados
  dicas para que o dispositivo canônico flua pela mesma API que consome as rodas. O teste
  reconstruir metadados do provedor de `providers.json`, inserir o instantâneo de telemetria e verificar
  bytes de carga útil, recebimentos de blocos, relatórios de provedores e conteúdo do placar igual ao dos
  suítes Rust/JS/Swift.
- **Pré-requisito:** Execute `maturin develop --release` (ou instale a roda) para que `_crypto` exponha o
  ligação `sorafs_multi_fetch_local` antes de invocar o pytest; el chicote se auto-salta quando el
  vinculativo não está disponível.
- **Proteção de desempenho:** El mismo presupuesto <= 2 s que la suite Rust; pytest registra o conteúdo de
  bytes ensamblados e o resumo da participação dos provedores para o artefato de lançamento.

O gate de liberação deve capturar o resumo da saída de cada chicote (Rust, Python, JS, Swift) para que
o relatório arquivado pode comparar recibos de carga útil e métricas de uniforme antes de
promover uma construção. Execute `ci/sdk_sorafs_orchestrator.sh` para executar cada suíte de festa
(Rust, ligações Python, JS, Swift) em uma única etapa; os artefatos de CI devem ser adicionados ao
extrato de log desse auxiliar mas o `matrix.md` gerado (tabela SDK/estado/duração) no ticket
release para que os revisores auditem a matriz de paridade sem reejecutar a suíte localmente.