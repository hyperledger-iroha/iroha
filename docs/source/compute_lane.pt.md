---
lang: pt
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2026-01-03T18:07:56.917770+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Pista de computação (SSC-1)

A pista de computação aceita chamadas determinísticas no estilo HTTP, mapeia-as em Kotodama
pontos de entrada e registra medições/recebimentos para faturamento e revisão de governança.
Este RFC congela o esquema de manifesto, envelopes de chamada/recebimento, proteções de sandbox,
e padrões de configuração para a primeira versão.

## Manifesto

- Esquema: `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest`/
  `ComputeRoute`).
- `abi_version` está fixado em `1`; manifestos com uma versão diferente são rejeitados
  durante a validação.
- Cada rota declara:
  -`id` (`service`, `method`)
  - `entrypoint` (nome do ponto de entrada Kotodama)
  - lista de permissões de codecs (`codecs`)
  - Limites de TTL/gás/solicitação/resposta (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - classe de determinismo/execução (`determinism`, `execution_class`)
  - Descritores de modelo/entrada SoraFS (`input_limits`, `model` opcional)
  - família de preços (`price_family`) + perfil de recursos (`resource_profile`)
  - política de autenticação (`auth`)
- As proteções do sandbox residem no bloco manifesto `sandbox` e são compartilhadas por todos
  rotas (modo/aleatoriedade/armazenamento e rejeição não determinística de syscall).

Exemplo: `fixtures/compute/manifest_compute_payments.json`.

## Chamadas, solicitações e recebimentos

- Esquema: `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome` em
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()` produz o hash de solicitação canônica (os cabeçalhos são mantidos
  em um `BTreeMap` determinístico e a carga útil é transportada como `payload_hash`).
- `ComputeCall` captura o namespace/rota, codec, TTL/gás/cap de resposta,
  perfil de recursos + família de preços, autenticação (`Public` ou vinculado ao UAID
  `ComputeAuthn`), determinismo (`Strict` vs `BestEffort`), classe de execução
  dicas (CPU/GPU/TEE), bytes/pedaços de entrada SoraFS declarados, patrocinador opcional
  orçamento e o envelope de solicitação canônica. O hash de solicitação é usado para
  proteção de reprodução e roteamento.
- As rotas podem incorporar referências de modelo SoraFS opcionais e limites de entrada
  (caps em linha/bloco); regras de sandbox de manifesto bloqueiam dicas de GPU/TEE.
- `ComputePriceWeights::charge_units` converte dados de medição em computação faturada
  unidades via divisão de teto em ciclos e bytes de saída.
- `ComputeOutcome` relata `Success`, `Timeout`, `OutOfMemory`,
  `BudgetExhausted` ou `InternalError` e opcionalmente inclui hashes de resposta/
  tamanhos/codec para auditoria.

Exemplos:
- Ligue: `fixtures/compute/call_compute_payments.json`
- Recibo: `fixtures/compute/receipt_compute_payments.json`

## Sandbox e perfis de recursos- `ComputeSandboxRules` bloqueia o modo de execução para `IvmOnly` por padrão,
  semeia a aleatoriedade determinística do hash da solicitação, permite SoraFS somente leitura
  acesso e rejeita syscalls não determinísticos. As dicas de GPU/TEE são controladas por
  `allow_gpu_hints`/`allow_tee_hints` para manter a execução determinística.
- `ComputeResourceBudget` define limites por perfil em ciclos, memória linear, pilha
  tamanho, orçamento de IO e saída, além de alternadores para dicas de GPU e auxiliares WASI-lite.
- Os padrões enviam dois perfis (`cpu-small`, `cpu-balanced`) em
  `defaults::compute::resource_profiles` com substitutos determinísticos.

## Unidades de preços e faturamento

- Famílias de preços (`ComputePriceWeights`) mapeiam ciclos e bytes de saída na computação
  unidades; os padrões cobram `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` com
  `unit_label = "cu"`. As famílias são codificadas por `price_family` em manifestos e
  aplicado na admissão.
- Os registros de medição carregam `charged_units` mais ciclo bruto/entrada/saída/duração
  totais para reconciliação. As cobranças são amplificadas por classe de execução e
  multiplicadores de determinismo (`ComputePriceAmplifiers`) e limitados por
  `compute.economics.max_cu_per_call`; a saída é reprimida por
  `compute.economics.max_amplification_ratio` para amplificação de resposta vinculada.
- Os orçamentos do patrocinador (`ComputeCall::sponsor_budget_cu`) são aplicados contra
  limites por chamada/diário; as unidades faturadas não devem exceder o orçamento declarado do patrocinador.
- As atualizações dos preços de governança utilizam os limites da classe de risco em
  `compute.economics.price_bounds` e as famílias de linha de base registradas em
  `compute.economics.price_family_baseline`; usar
  `ComputeEconomics::apply_price_update` para validar deltas antes de atualizar
  o mapa familiar ativo. Uso de atualizações de configuração Torii
  `ConfigUpdate::ComputePricing`, e kiso aplica-o com os mesmos limites para
  manter as edições de governança determinísticas.

## Configuração

A nova configuração de computação reside em `crates/iroha_config/src/parameters`:

- Visualização do usuário: `Compute` (`user.rs`) com substituições de env:
  -`COMPUTE_ENABLED` (padrão `false`)
  -`COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  -`COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  -`COMPUTE_MAX_GAS_PER_CALL`
  -`COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  -`COMPUTE_AUTH_POLICY`
- Preço/economia: capturas `compute.economics`
  `max_cu_per_call`/`max_amplification_ratio`, divisão de taxas, limites de patrocinador
  (CU por chamada e diária), linhas de base da família de preços + classes/limites de risco para
  atualizações de governança e multiplicadores de classe de execução (GPU/TEE/melhor esforço).
- Real/padrões: `actual.rs` / `defaults.rs::compute` exposição analisada
  Configurações `Compute` (namespaces, perfis, famílias de preços, sandbox).
- Configurações inválidas (namespaces vazios, perfil/família padrão ausentes, limite TTL
  inversões) aparecem como `InvalidComputeConfig` durante a análise.

## Testes e acessórios

- Auxiliares determinísticos (`request_hash`, preços) e viagens de ida e volta de equipamentos ao vivo em
  `crates/iroha_data_model/src/compute/mod.rs` (ver `fixtures_round_trip`,
  `request_hash_is_stable`, `pricing_rounds_up_units`).
- Os fixtures JSON residem em `fixtures/compute/` e são exercidos pelo modelo de dados
  testes para cobertura de regressão.

## Equipamento e orçamentos de SLO- A configuração `compute.slo.*` expõe os botões SLO do gateway (fila em andamento
  profundidade, limite de RPS e metas de latência) em
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. Padrões: 32
  em voo, 512 na fila por rota, 200 RPS, p50 25ms, p95 75ms, p99 120ms.
- Execute o equipamento de banco leve para capturar resumos de SLO e uma solicitação/saída
  snapshot: `cargo run -p xtask --bin compute_gateway -- bench [manifest_path]
  [iterações] [simultaneidade] [out_dir]` (defaults: `fixtures/compute/manifest_compute_payments.json`,
  128 iterações, simultaneidade 16, saídas em
  `artifacts/compute_gateway/bench_summary.{json,md}`). O banco usa
  cargas úteis determinísticas (`fixtures/compute/payload_compute_payments.json`) e
  cabeçalhos por solicitação para evitar colisões de repetição durante o exercício
  Pontos de entrada `echo`/`uppercase`/`sha3`.

## Dispositivos de paridade SDK/CLI

- Os fixtures canônicos vivem sob `fixtures/compute/`: manifesto, chamada, carga útil e
  o layout de resposta/recebimento estilo gateway. Os hashes de carga devem corresponder à chamada
  `request.payload_hash`; a carga útil auxiliar reside em
  `fixtures/compute/payload_compute_payments.json`.
- A CLI envia `iroha compute simulate` e `iroha compute invoke`:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` ao vivo em
  `javascript/iroha_js/src/compute.js` com testes de regressão em
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift: `ComputeSimulator` carrega os mesmos fixtures, valida hashes de carga útil,
  e simula os pontos de entrada com testes em
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- Todos os auxiliares CLI/JS/Swift compartilham os mesmos equipamentos Norito para que os SDKs possam
  validar a construção da solicitação e o tratamento de hash off-line sem atingir um
  gateway em execução.