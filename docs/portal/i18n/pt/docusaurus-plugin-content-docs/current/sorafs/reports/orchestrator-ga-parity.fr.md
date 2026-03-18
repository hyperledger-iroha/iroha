---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Relatório de paridade GA SoraFS Orchestrator

A paridade determinada de multi-fetch é desordenada automaticamente pelo SDK depois que eles
engenheiros de lançamento podem confirmar que os bytes de carga útil, recibos de pedaços,
relatórios do provedor e resultados do placar restantes alinhados entre
implementações. Chaque aproveitar consomme le bundle multi-provider canonique dans
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, que reagrupa o plano SF1,
o provedor de metadados, o instantâneo de telemetria e as opções de orquestração.

## Linha de base de ferrugem

- **Comando:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Escopo:** Execute o plano `MultiPeerFixture` dois vezes por meio do orquestrador em processo,
  verificar bytes de montagens de carga útil, recibos de pedaços, relatórios de provedores e outros
  resultados do placar. A instrumentação serve também para a concorrência de pontas
  e a cauda efetiva do conjunto de trabalho (`max_parallel × max_chunk_length`).
- **Proteção de desempenho:** Toda execução deve ser concluída em 2 segundos no CI de hardware.
- **Teto do conjunto de trabalho:** Com o perfil SF1, o arnês impõe `max_parallel = 3`,
  não possui uma janela ≤ 196608 bytes.

Exemplo de saída de registro:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Equipamento do SDK JavaScript

- **Comando:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Escopo:** Rejoue la même fixture via `iroha_js_host::sorafsMultiFetchLocal`,
  cargas úteis comparativas, recibos, relatórios de provedores e instantâneos de placar entre
  execuções consecutivas.
- **Guarda de desempenho:** Cada execução termina em 2 s; le arnês imprime la
  duração medida e o plafond de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Exemplo de linha de resumo:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Equipamento do SDK Swift

- **Comando:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Escopo:** Execute o conjunto de paridade definido em `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  rejouant la fixture SF1 dois fois via le bridge Norito (`sorafsLocalFetch`). Le arnês verificado
  bytes de carga útil, recibos de pedaços, relatórios de provedores e entradas de placar usando
  O mesmo provedor de metadados determina e instantâneos de telemetria que os pacotes Rust/JS.
- **Bootstrap da ponte:** O chicote descompactado `dist/NoritoBridge.xcframework.zip` conforme exigido e carregado
  le fatia macOS via `dlopen`. Quando l'xcframework é deficiente ou não há ligações SoraFS, il
  bascule em `cargo build -p connect_norito_bridge --release` e se deite à
  `target/release/libconnect_norito_bridge.dylib`, sem manual de configuração e CI.
- **Proteção de desempenho:** Toda execução deve terminar em 2 s no hardware CI ; le arnês imprime la
  duração medida e o plafond de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Exemplo de linha de resumo:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Chicote de ligações Python- **Comando:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Escopo:** Exerça o wrapper no alto nível `iroha_python.sorafs.multi_fetch_local` e seus tipos de classes de dados
  depois que o fixture canônico passa pela mesma API que os consumidores de roda. Le teste reconstruído
  o provedor de metadados baseado em `providers.json` injeta o instantâneo de telemetria e verifica os bytes de carga útil,
  recibos de pedaços, relatórios de provedores e conteúdo do placar, como os pacotes Rust/JS/Swift.
- **Pré-requisito:** Execute `maturin develop --release` (ou instale a roda) para que `_crypto` exponha a ligação
  `sorafs_multi_fetch_local` antes de invocar pytest ; O salto automático do chicote quando a ligação é indisponível.
- **Guarda de desempenho:** Même budget ≤ 2 s que la suite Rust ; pytest registra o nome dos bytes montados
  e o currículo de participação dos fornecedores para o artefato de lançamento.

O release gating doit capturer o resumo da saída de cada chicote (Rust, Python, JS, Swift) depois disso
o relatório arquivado pode comparar receitas de carga útil e métricas de maneira uniforme antes de
promova uma construção. Execute `ci/sdk_sorafs_orchestrator.sh` para lancer cada conjunto de paridade
(Rust, ligações Python, JS, Swift) em um único passe ; les artefatos CI doivent joindre l'extrait
log deste auxiliar mais o `matrix.md` gerado (tabela SDK/status/duração) no ticket de lançamento depois que
os revisores podem auditar a matriz de paridade sem relançar a localização da suíte.