---
lang: pt
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-18T17:14:31.034360+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Fluxo de trabalho de desenvolvimento de AGENTES

Este runbook consolida as barreiras de contribuição do roteiro de AGENTES para que
novos patches seguem os mesmos portões padrão.

## Metas de início rápido

- Execute `make dev-workflow` (wrapper em torno de `scripts/dev_workflow.sh`) para executar:
  1.`cargo fmt --all`
  2.`cargo clippy --workspace --all-targets --locked -- -D warnings`
  3.`cargo build --workspace --locked`
  4.`cargo test --workspace --locked`
  5. `swift test` de `IrohaSwift/`
- `cargo test --workspace` é de longa duração (geralmente horas). Para iterações rápidas,
  use `scripts/dev_workflow.sh --skip-tests` ou `--skip-swift` e execute o programa completo
  sequência antes do envio.
- Se `cargo test --workspace` travar em bloqueios de diretório de construção, execute novamente com
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (ou conjunto
  `CARGO_TARGET_DIR` para um caminho isolado) para evitar contenção.
- Todas as etapas de carga utilizam `--locked` para respeitar a política de manutenção do repositório
  `Cargo.lock` intocado. Prefira estender as caixas existentes em vez de adicionar
  novos membros do espaço de trabalho; busque aprovação antes de introduzir uma nova caixa.

## Guarda-corpos- `make check-agents-guardrails` (ou `ci/check_agents_guardrails.sh`) falha se um
  branch modifica `Cargo.lock`, introduz novos membros do espaço de trabalho ou adiciona novos
  dependências. O script compara a árvore de trabalho e `HEAD` com
  `origin/main` por padrão; defina `AGENTS_BASE_REF=<ref>` para substituir a base.
-`make check-dependency-discipline` (ou `ci/check_dependency_discipline.sh`)
  compara as dependências `Cargo.toml` com a base e falha em novas caixas; definir
  `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>` para reconhecer intencional
  acréscimos.
- `make check-missing-docs` (ou `ci/check_missing_docs_guard.sh`) bloqueia novos
  Entradas `#[allow(missing_docs)]`, sinalizadores tocaram caixas (`Cargo.toml` mais próximo)
  cujo `src/lib.rs`/`src/main.rs` não possui documentos `//!` em nível de caixa e rejeita novos
  itens públicos sem documentos `///` relativos à referência base; definir
  `MISSING_DOCS_GUARD_ALLOW=1` somente com aprovação do revisor. O guarda também
  verifica se `docs/source/agents/missing_docs_inventory.{json,md}` estão atualizados;
  regenerar com `python3 scripts/inventory_missing_docs.py`.
- `make check-tests-guard` (ou `ci/check_tests_guard.sh`) sinaliza caixas cujas
  as funções alteradas do Rust não possuem evidências de teste de unidade. Os mapas de guarda mudaram de linha
  para funções, passa se os testes de caixa forem alterados no diff e, caso contrário, verifica
  arquivos de teste existentes para correspondência de chamadas de função para cobertura pré-existente
  conta; caixas sem nenhum teste correspondente falharão. Conjunto `TEST_GUARD_ALLOW=1`
  somente quando as alterações forem verdadeiramente neutras em termos de teste e o revisor concordar.
-`make check-docs-tests-metrics` (ou `ci/check_docs_tests_metrics_guard.sh`)
  impõe a política do roteiro de que os marcos acompanham a documentação,
  testes e métricas/painéis. Quando `roadmap.md` muda em relação a
  `AGENTS_BASE_REF`, o guarda espera pelo menos uma alteração de documento, uma alteração de teste,
  e uma alteração de métricas/telemetria/painel. Conjunto `DOC_TEST_METRIC_GUARD_ALLOW=1`
  somente com aprovação do revisor.
- `make check-todo-guard` (ou `ci/check_todo_guard.sh`) falha quando marcadores TODO
  desaparecer sem acompanhar as alterações de documentos/testes. Adicionar ou atualizar cobertura
  ao resolver um TODO ou defina `TODO_GUARD_ALLOW=1` para remoções intencionais.
- `make check-std-only` (ou `ci/check_std_only.sh`) bloqueia `no_std`/`wasm32`
  cfgs para que o espaço de trabalho permaneça apenas `std`. Defina `STD_ONLY_GUARD_ALLOW=1` apenas para
  experimentos de IC sancionados.
- `make check-status-sync` (ou `ci/check_status_sync.sh`) mantém o roteiro aberto
  seção livre de itens concluídos e requer `roadmap.md`/`status.md` para
  mudem juntos para que o plano/status permaneça alinhado; definir
  `STATUS_SYNC_ALLOW_UNPAIRED=1` apenas para raras correções de erros de digitação somente de status após
  fixando `AGENTS_BASE_REF`.
- `make check-proc-macro-ui` (ou `ci/check_proc_macro_ui.sh`) executa o trybuild
  Conjuntos de UI para caixas de derivação/proc-macro. Execute-o ao tocar em proc-macros para
  manter o diagnóstico `.stderr` estável e detectar regressões de UI em pânico; definir
  `PROC_MACRO_UI_CRATES="crate1 crate2"` para focar em caixas específicas.
- Reconstruções `make check-env-config-surface` (ou `ci/check_env_config_surface.sh`)
  o inventário de alternância de ambiente (`docs/source/agents/env_var_inventory.{json,md}`),
  falha se estiver obsoleto, **e** falha quando novos shims de ambiente de produção aparecem
  relativo a `AGENTS_BASE_REF` (detectado automaticamente; definido explicitamente quando necessário).
  Atualize o rastreador após adicionar/remover pesquisas de ambiente via
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  use `ENV_CONFIG_GUARD_ALLOW=1` somente após documentar botões de env intencionaisno rastreador de migração.
- `make check-serde-guard` (ou `ci/check_serde_guard.sh`) regenera o serde
  inventário de uso (`docs/source/norito_json_inventory.{json,md}`) em um temporário
  local, falha se o estoque confirmado estiver obsoleto e rejeita qualquer novo
  produção `serde`/`serde_json` em relação a `AGENTS_BASE_REF`. Definir
  `SERDE_GUARD_ALLOW=1` somente para experimentos de CI após preencher um plano de migração.
- `make guards` impõe a política de serialização Norito: nega novos
  Uso de `serde`/`serde_json`, auxiliares AoS ad-hoc e dependências SCALE externas
  as bancadas Norito (`scripts/deny_serde_json.sh`,
  `scripts/check_no_direct_serde.sh`, `scripts/deny_handrolled_aos.sh`,
  `scripts/check_no_scale.sh`).
- **Política de interface do usuário Proc-macro:** cada caixa proc-macro deve enviar um `trybuild`
  chicote de fios (`tests/ui.rs` com globos de aprovação/falha) atrás do `trybuild-tests`
  recurso. Coloque amostras de caminho feliz em `tests/ui/pass`, casos de rejeição em
  `tests/ui/fail` com saídas `.stderr` comprometidas e mantém diagnósticos
  sem pânico e estável. Atualizar jogos com
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (opcionalmente com
  `CARGO_TARGET_DIR=target-codex` para evitar destruir compilações existentes) e
  evite depender de construções de cobertura (são esperados guardas `cfg(not(coverage))`).
  Para macros que não emitem um ponto de entrada binário, prefira
  `// compile-flags: --crate-type lib` em jogos para manter o foco nos erros. Adicionar
  novos casos negativos sempre que os diagnósticos mudam.
- CI executa os scripts de proteção via `.github/workflows/agents-guardrails.yml`
  portanto, as solicitações pull falham rapidamente quando as políticas são violadas.
- O gancho git de amostra (`hooks/pre-commit.sample`) executa guardrail, dependência,
  scripts missing-docs, std-only, env-config e status-sync para que os contribuidores
  detectar violações de políticas antes da CI. Mantenha a trilha TODO para qualquer propósito intencional
  acompanhamentos em vez de adiar grandes mudanças silenciosamente.