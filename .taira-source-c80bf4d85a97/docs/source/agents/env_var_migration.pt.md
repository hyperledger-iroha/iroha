---
lang: pt
direction: ltr
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9ce6010594e495116c1397b984000d1ee5d45d064294eca046f8dc762fa73b6
source_last_modified: "2026-01-04T10:50:53.607349+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Env → Configurar Rastreador de Migração

Este rastreador resume as alternâncias de variáveis de ambiente voltadas para a produção
por `docs/source/agents/env_var_inventory.{json,md}` e a migração pretendida
caminho para `iroha_config` (ou escopo explícito de desenvolvimento/somente teste).


Nota: `ci/check_env_config_surface.sh` agora falha quando novo ambiente de **produção**
calços aparecem em relação a `AGENTS_BASE_REF`, a menos que `ENV_CONFIG_GUARD_ALLOW=1` seja
conjunto; documente adições intencionais aqui antes de usar a substituição.

## Migrações concluídas- **Desativação da ABI IVM** — `IVM_ALLOW_NON_V1_ABI` removido; o compilador agora rejeita
  ABIs não-v1 incondicionalmente com um teste de unidade protegendo o caminho do erro.
- **IVM debug banner env shim** — Removido o opt-out do env `IVM_SUPPRESS_BANNER`;
  a supressão de banner permanece disponível por meio do configurador programático.
- **IVM cache/dimensionamento** — Dimensionamento de cache/provador/GPU encadeado por meio
  `iroha_config` (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`,
  `accel.max_gpus`) e removeu correções de ambiente de tempo de execução. Os anfitriões agora ligam
  `ivm::ivm_cache::configure_limits` e `ivm::zk::set_prover_threads`, testes usam
  `CacheLimitsGuard` em vez de substituições de env.
- **Conectar raiz da fila** — Adicionado `connect.queue.root` (padrão:
  `~/.iroha/connect`) para a configuração do cliente e encadeou-o através da CLI e
  Diagnóstico JS. Ajudantes JS resolvem a configuração (ou um `rootDir` explícito) e
  honre apenas `IROHA_CONNECT_QUEUE_ROOT` em dev/test via `allowEnvOverride`;
  os modelos documentam o botão para que os operadores não precisem mais de substituições de ambiente.
- **Opt-in de rede Izanami** — Adicionado um sinalizador CLI/config `allow_net` explícito para
  a ferramenta de caos Izanami; execuções agora requerem `allow_net=true`/`--allow-net` e
- **Bipe de banner IVM** — Substituído o env shim `IROHA_BEEP` por orientado por configuração
  `ivm.banner.{show,beep}` alterna (padrão: verdadeiro/verdadeiro). Banner/bipe de inicialização
  a fiação agora lê a configuração apenas na produção; compilações dev/test ainda honram
  a substituição de env para alternâncias manuais.
- **Substituição de spool DA (somente testes)** — A substituição `IROHA_DA_SPOOL_DIR` agora é
  cercado atrás de ajudantes `cfg(test)`; o código de produção sempre origina o spool
  caminho da configuração.
- **Intrínsecos da criptografia** — Substituído `IROHA_DISABLE_SM_INTRINSICS` /
  `IROHA_ENABLE_SM_INTRINSICS` com o orientado por configuração
  Política `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`) e
  removeu a proteção `IROHA_SM_OPENSSL_PREVIEW`. Os anfitriões aplicam a política em
  inicialização, bancadas/testes podem optar por meio de `CRYPTO_SM_INTRINSICS` e OpenSSL
  a visualização agora respeita apenas o sinalizador de configuração.
  Izanami já requer configuração `--allow-net`/persistida e os testes agora dependem de
  esse botão em vez do ambiente ambiente alterna.
- **Ajuste de GPU FastPQ** — Adicionado `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}`
  botões de configuração (padrões: `None`/`None`/`false`/`false`/`false`) e encadeá-los através da análise CLI
  Os calços `FASTPQ_METAL_*` / `FASTPQ_DEBUG_*` agora se comportam como substitutos de desenvolvimento/teste e
  são ignorados quando a configuração é carregada (mesmo quando a configuração os deixa indefinidos); documentos/inventário foram
  atualizado para sinalizar a migração.【crates/irohad/src/main.rs:2609】【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】
  (`IVM_DECODE_TRACE`, `IVM_DEBUG_WSV`, `IVM_DEBUG_COMPACT`, `IVM_DEBUG_INVALID`,
  `IVM_DEBUG_REGALLOC`, `IVM_DEBUG_METAL_ENUM`, `IVM_DEBUG_METAL_SELFTEST`,
  `IVM_FORCE_METAL_ENUM`, `IVM_FORCE_METAL_SELFTEST_FAIL`, `IVM_FORCE_CUDA_SELFTEST_FAIL`,
  `IVM_DISABLE_METAL`, `IVM_DISABLE_CUDA`) agora estão protegidos por compilações de depuração/teste por meio de um
  helper para que os binários de produção os ignorem enquanto preservam os botões para diagnóstico local. Ambiente
  o inventário foi regenerado para refletir o escopo somente de desenvolvimento/teste.- **Atualizações de dispositivos FASTPQ** — `FASTPQ_UPDATE_FIXTURES` agora aparece apenas na integração FASTPQ
  testes; as fontes de produção não leem mais a alternância de ambiente e o inventário reflete apenas o teste
  escopo.
- **Atualização de inventário + detecção de escopo** — As ferramentas de inventário de ambiente agora marcam arquivos `build.rs` como
  escopo de construção e rastreia módulos de chicote `#[cfg(test)]`/integração para alternar somente teste (por exemplo,
  `IROHA_TEST_*`, `IROHA_RUN_IGNORED`) e sinalizadores de construção CUDA aparecem fora da contagem de produção.
  Inventário regenerado em 07 de dezembro de 2025 (518 refs/144 vars) para manter o env-config guard diff verde.
- **Proteção de liberação de correção de ambiente de topologia P2P** — `IROHA_P2P_TOPOLOGY_UPDATE_MS` agora aciona um determinístico
  erro de inicialização em compilações de lançamento (somente aviso em depuração/teste), portanto, os nós de produção dependem apenas de
  `network.peer_gossip_period_ms`. O inventário ambiental foi regenerado para refletir o guarda e o
  o classificador atualizado agora abrange os alternadores protegidos por `cfg!` como depuração/teste.

## Migrações de alta prioridade (caminhos de produção)

- _Nenhum (inventário atualizado com detecção cfg!/debug; env-config guard green após endurecimento de correção P2P)._

## Somente desenvolvimento/teste alterna para fence

- Varredura atual (7 de dezembro de 2025): sinalizadores CUDA somente de compilação (`IVM_CUDA_*`) têm escopo como `build` e o
  alternadores de chicote (`IROHA_TEST_*`, `IROHA_RUN_IGNORED`, `IROHA_SKIP_BIND_CHECKS`) agora registram-se como
  `test`/`debug` no inventário (incluindo calços protegidos por `cfg!`). Nenhuma cerca adicional é necessária;
  mantenha adições futuras atrás de ajudantes `cfg(test)`/somente de bancada com marcadores TODO quando os calços forem temporários.

## Ambientes de tempo de construção (deixe como estão)

- Ambientes de carga/recurso (`CARGO_*`, `OUT_DIR`, `DOCS_RS`, `PROFILE`, `CUDA_HOME`,
  `CUDA_PATH`, `JSONSTAGE1_CUDA_ARCH`, `FASTPQ_SKIP_GPU_BUILD`, etc.) permanecem
  preocupações com o script de construção e estão fora do escopo da migração de configuração de tempo de execução.

## Próximas ações

1) Execute `make check-env-config-surface` após as atualizações da superfície de configuração para capturar novos calços de ambiente de produção
   antecipadamente e atribuir proprietários/ETAs de subsistemas.  
2) Atualize o inventário (`make check-env-config-surface`) após cada varredura para
   o rastreador permanece alinhado com os novos guardrails e o env-config guard diff permanece livre de ruído.