---
lang: pt
direction: ltr
source: docs/source/confidential_assets_rotation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd1e43316c492cc96ed107f6318841ad8db160735d4698c4f05562ff6127fda9
source_last_modified: "2026-01-22T15:38:30.658859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Manual de rotação de ativos confidenciais referenciado por `roadmap.md:M3`.

# Runbook de rotação de ativos confidenciais

Este manual explica como os operadores agendam e executam ativos confidenciais
rotações (conjuntos de parâmetros, verificação de chaves e transições de política) enquanto
garantindo que carteiras, clientes Torii e protetores de mempool permaneçam determinísticos.

## Ciclo de vida e status

Conjuntos de parâmetros confidenciais (`PoseidonParams`, `PedersenParams`, chaves de verificação)
treliça e auxiliar usados para derivar o status efetivo em uma determinada altura vivem em
`crates/iroha_core/src/state.rs:7540`–`7561`. Varredura pendente dos auxiliares de tempo de execução
transições assim que a altura alvo é atingida e registra falhas para posterior
retransmissões (`crates/iroha_core/src/state.rs:6725` – `6765`).

Incorporação de políticas de ativos
`pending_transition { transition_id, new_mode, effective_height, conversion_window }`
para que a governança possa agendar atualizações via
`ScheduleConfidentialPolicyTransition` e cancele-os se necessário. Veja
`crates/iroha_data_model/src/asset/definition.rs:320` e os espelhos Torii DTO
(`crates/iroha_torii/src/routing.rs:1539`–`1580`).

## Fluxo de trabalho de rotação

1. **Publicar novos pacotes de parâmetros.** Os operadores enviam
   Instruções `PublishPedersenParams`/`PublishPoseidonParams` (CLI
   `iroha app zk params publish ...`) para preparar novos grupos geradores com metadados,
   janelas de ativação/descontinuação e marcadores de status. O executor rejeita
   IDs duplicados, versões sem aumento ou transições de status incorretas por
   `crates/iroha_core/src/smartcontracts/isi/world.rs:2499` –`2635`, e o
   os testes de registro cobrem os modos de falha (`crates/iroha_core/tests/confidential_params_registry.rs:93` – `226`).
2. **Registre/verifique atualizações de chave.** `RegisterVerifyingKey` impõe back-end,
   compromisso e restrições de circuito/versão antes que uma chave possa entrar no
   registro (`crates/iroha_core/src/smartcontracts/isi/world.rs:2067` – `2137`).
   A atualização de uma chave descontinua automaticamente a entrada antiga e apaga bytes embutidos,
   conforme exercido por `crates/iroha_core/tests/zk_vk_deprecate_marks_status.rs:1`.
3. **Programe transições de política de ativos.** Assim que os novos IDs de parâmetro estiverem ativos,
   governança chama `ScheduleConfidentialPolicyTransition` com o desejado
   modo, janela de transição e hash de auditoria. O executor recusa conflitos
   transições ou ativos com excelente oferta transparente. Testes como
   `crates/iroha_core/tests/confidential_policy_gates.rs:300` – `384` verifique se
   transições abortadas limpam `pending_transition`, enquanto
   `confidential_policy_transition_reaches_shielded_only_on_schedule` em
   linhas385–433 confirma que as atualizações programadas mudam para `ShieldedOnly` exatamente em
   a altura efetiva.
4. **Aplicativo de política e proteção de mempool.** O executor do bloco varre todos os itens pendentes
   transições no início de cada bloco (`apply_policy_if_due`) e emite
   telemetria se uma transição falhar para que os operadores possam reagendar. Durante a admissão
   o mempool recusa transações cuja política efetiva mudaria no meio do bloco,
   garantindo a inclusão determinística em toda a janela de transição
   (`docs/source/confidential_assets.md:60`).

## Requisitos de carteira e SDK- Swift e outros SDKs móveis expõem auxiliares Torii para buscar a política ativa
  além de qualquer transição pendente, para que as carteiras possam avisar os usuários antes de assinar. Veja
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:309` (DTO) e o associado
  testes em `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift:591`.
- A CLI espelha os mesmos metadados via `iroha ledger assets data-policy get` (auxiliar em
  `crates/iroha_cli/src/main.rs:1497`–`1670`), permitindo que os operadores auditem o
  IDs de política/parâmetro conectados a uma definição de ativo sem explorar o
  loja de blocos.

## Cobertura de teste e telemetria

- `crates/iroha_core/tests/zk_ledger_scaffold.rs:288` –`345` verifica essa política
  as transições se propagam em instantâneos de metadados e são limpas depois de aplicadas.
- `crates/iroha_core/tests/zk_dedup.rs:1` prova que o cache `Preverify`
  rejeita gastos duplos/provas duplas, incluindo cenários de rotação onde
  compromissos diferem.
-`crates/iroha_core/tests/zk_confidential_events.rs` e
  `zk_shield_transfer_audit.rs` tampa blindagem ponta a ponta → transferência → desproteção
  fluxos, garantindo que a trilha de auditoria sobreviva nas rotações de parâmetros.
-`dashboards/grafana/confidential_assets.json` e
  `docs/source/confidential_assets.md:401` documenta o CommitmentTree &
  medidores de cache de verificador que acompanham cada execução de calibração/rotação.

## Propriedade do runbook

- **DevRel / Wallet SDK Leads:** mantenha snippets do SDK + guias de início rápido que mostram
  como trazer à tona transições pendentes e reproduzir o mint → transferir → revelar
  testes localmente (rastreados sob `docs/source/project_tracker/confidential_assets_phase_c.md:M3.2`).
- **Gerenciamento de Programas / TL de Ativos Confidenciais:** aprovar solicitações de transição, manter
  `status.md` atualizado com as próximas rotações e garantir que as isenções (se houver) sejam
  registrado junto com o livro de calibração.