---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pacote de modo direto
título: Pacote de contingência de modo direto do SoraFS (SNNet-5a)
sidebar_label: Pacote de modo direto
description: Configuração obrigatória, verificações de compliance e passos de rollout ao operar SoraFS em modo direto Torii/QUIC durante a transição SNNet-5a.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/direct_mode_pack.md`. Mantenha ambas as cópias sincronizadas.
:::

Os circuitos SoraNet seguem como padrão de transporte do SoraFS, mas o item de roadmap **SNNet-5a** exige um fallback regulado para que os operadores mantenham acesso de leitura determinístico enquanto o lançamento do anonimato se completa. Este pacote captura os botões CLI/SDK, perfis de configuração, testes de conformidade e o checklist de implantação necessário para rodar o SoraFS no modo direto Torii/QUIC sem tocar os transportes de privacidade.

O fallback é aplicado a staging e ambientes de produção regulamentados até que SNNet-5 e SNNet-9 passem pelos portões de prontidão. Mantenha os artefatos abaixo junto com o material de implantação do SoraFS para que os operadores alternem entre modos anônimos e diretos sob demanda.

## 1. Flags de CLI e SDK

- `sorafs_cli fetch --transport-policy=direct-only ...` desativa o agendamento de relés e impoe transportes Torii/QUIC. A ajuda do CLI agora lista `direct-only` como valor aceito.
- SDKs devem definir `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` sempre que exuserem um toggle de "modo direto". As ligações geradas em `iroha::ClientOptions` e `iroha_android` encaminham o mesmo enum.
- Chicotes de gateway (`sorafs_fetch`, ligações Python) podem interpretar o toggle direct-only via helpers Norito JSON compartilhados para que a automação receba o mesmo comportamento.

Documente o sinalizador em runbooks relacionados a parceiros e passe os toggles via `iroha_config` em vez de variáveis ​​de ambiente.

## 2. Perfis de política do gateway

Use Norito JSON para persistir a configuração determinística do orquestrador. O perfil de exemplo em `docs/examples/sorafs_direct_mode_policy.json` codifica:

- `transport_policy: "direct_only"` - provedores rejeitados que assim anunciam transportes de relé SoraNet.
- `max_providers: 2` - limita peers diretamente aos endpoints Torii/QUIC mais confiáveis. Ajuste conforme os acessos de compliance regionais.
- `telemetry_region: "regulated-eu"` - rótulo como métricas emitidas para que dashboards e auditorias distingam execuções de fallback.
- Orcamentos de nova tentativa conservadores (`retry_budget: 2`, `provider_failure_threshold: 3`) para evitar mascarar gateways mal configurados.

Carregue o JSON via `sorafs_cli fetch --config` (automatização) ou ligações do SDK (`config_from_json`) antes de exportar a política para os operadores. Persista a saida do scoreboard (`persist_path`) para trilhas de auditoria.Os botões de aplicação do lado do gateway estão em `docs/examples/sorafs_gateway_direct_mode.toml`. O template reflete a saida de `iroha app sorafs gateway direct-mode enable`, desabilitando checagens de envelope/admissão, conectando defaults de rate-limit e preenchendo a tabela `direct_mode` com hostnames derivados do plano e digests de manifest. Substitua os valores de placeholder pelo seu plano de implementação antes de versionar o trecho na gestão de configuração.

## 3. Conjunto de testes de conformidade

A prontidão do modo direto agora inclui cobertura tanto no orquestrador quanto nas caixas de CLI:

- `direct_only_policy_rejects_soranet_only_providers` garante que `TransportPolicy::DirectOnly` falha rapidamente quando cada anúncio candidato suporta relés SoraNet. [crates/sorafs_orchestrator/src/lib.rs:7238]
- `direct_only_policy_prefers_direct_transports_when_available` garante que os transportes Torii/QUIC sejam usados quando disponíveis e que os relés SoraNet sejam excluídos da sessão. [crates/sorafs_orchestrator/src/lib.rs:7285]
- `direct_mode_policy_example_is_valid` faz análise de `docs/examples/sorafs_direct_mode_policy.json` para garantir que a documentação permanece válida aos auxiliares. [crates/sorafs_orchestrator/src/lib.rs:7509] [docs/examples/sorafs_direct_mode_policy.json:1]
- `fetch_command_respects_direct_transports` exercita `sorafs_cli fetch --transport-policy=direct-only` contra um gateway Torii simulado, fornece um teste de fumaça para ambientes regulados que fixam transportes diretamente. [crates/sorafs_car/tests/sorafs_cli.rs:2733]
- `scripts/sorafs_direct_mode_smoke.sh` envolve o mesmo comando como o JSON de política e a persistência do scoreboard para automação de rollout.

Rode uma suíte focada antes de publicar atualizações:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Se a compilação do workspace falhar por mudanças no upstream, registre o erro bloqueador em `status.md` e rode novamente quando a dependência atualizar.

## 4. Executa automação de fumaça

A cobertura do CLI sozinha não revela regressos específicos do ambiente (por exemplo, deriva de política do gateway ou divergências de manifesto). Um helper de smoke dedicado vive em `scripts/sorafs_direct_mode_smoke.sh` e envolve `sorafs_cli fetch` com a política do orquestrador em modo direto, persistência do scoreboard e captura de resumo.

Exemplo de uso:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- O script respeita flags de CLI e arquivos de configuração key=value (veja `docs/examples/sorafs_direct_mode_smoke.conf`). Preencha o resumo do manifesto e as entradas de anúncio de provedor com valores de produção antes de rodar.
- `--policy` tem como padrão `docs/examples/sorafs_direct_mode_policy.json`, mas qualquer JSON de orquestrador produzido por `sorafs_orchestrator::bindings::config_to_json` pode ser fornecido. O CLI aceita a política via `--orchestrator-config=PATH`, habilitando runs reproduziveis sem ajustar flags manualmente.
- Quando `sorafs_cli` não está no `PATH`, o helper compila a partir do crate `sorafs_orchestrator` (perfil release) para que os smokes exercitem o encanamento de modo direto enviado.
-Saídas:
  - Montado de carga útil (`--output`, padrão `artifacts/sorafs_direct_mode/payload.bin`).
  - Resumo de busca (`--summary`, padrão ao lado do payload) contendo uma regiao de telemetria e relatorios de provedores usados ​​como evidencia de rollout.
  - Instantâneo do placar persistido no caminho declarado no JSON de política (por exemplo, `fetch_state/direct_mode_scoreboard.json`). Arquive junto ao resumo em tickets de mudança.
- Automacao do gate de adocao: apos o fetch, o helper invoca `cargo xtask sorafs-adoption-check` usando os caminhos persistidos de scoreboard e summary. O quórum exigido por padrão e o número de provedores fornecidos na linha de comando; sobrescreva com `--min-providers=<n>` quando precisar de uma amostra maior. Os relatórios de adoção são gravados ao lado do resumo (`--adoption-report=<path>` pode definir um local customizado) e o helper passa `--require-direct-only` por padrão (alinhado ao fallback) e `--require-telemetry` sempre que você fornecer a bandeira correspondente. Use `XTASK_SORAFS_ADOPTION_FLAGS` para repassar argumentos extras do xtask (por exemplo `--allow-single-source` durante um downgrade aprovado para que o gate tolere e imponha o fallback). Então pule o gate com `--skip-adoption-check` ao rodar diagnósticos locais; o roteiro exige que cada um seja regulado em modo direto incluindo o pacote de relatorio de adoção.

## 5. Lista de verificação de implementação1. **Freeze de configuração:** armazene o perfil JSON de modo direto no repositório `iroha_config` e registre o hash no ticket de mudança.
2. **Auditoria de gateway:** confirme que endpoints Torii aplicam TLS, TLVs de capacidade e registro de auditoria antes de mudar para modo direto. Publique o perfil de política do gateway para os operadores.
3. **Sign-off de compliance:** compartilhe o playbook atualizado com revisores de compliance/regulatórios e capture aprovações para operar fora da sobreposição de anonimato.
4. **Teste:** execute um conjunto de conformidade mais uma busca de teste contra provedores Torii confiáveis. Arquive resultados do placar e resumos do CLI.
5. **Cutover em produção:** anuncie uma janela de mudança, altere `transport_policy` para `direct_only` (se você tiver sido optado por `soranet-first`) e monitore os dashboards de modo direto (latência de `sorafs_fetch`, contadores de falha de fornecedores). Documente o plano de rollback para voltar ao SoraNet-first quando SNNet-4/5/5a/5b/6a/7/8/12/13 se graduarem em `roadmap.md:532`.
6. **Revisão pós-mudanca:** anexo snapshots do scoreboard, resumos de busca e resultados de monitoramento ao ticket de mudança. Atualize `status.md` com os dados efetivos e qualquer anomalia.

Mantenha o checklist junto ao runbook `sorafs_node_ops` para que os operadores possam ensaiar o fluxo antes de uma virada ao vivo. Quando SNNet-5 chegar a GA, retire o substituto após confirmar a paridade na telemetria de produção.

## 6. Requisitos de evidência e portão de adoção

Capturas em modo direto ainda precisam satisfazer o portão de adoção SF-6c. Bundle o scoreboard, o resumo, o envelope de manifesto e o relatorio de adoção em cada execução para que `cargo xtask sorafs-adoption-check` valide a postura de fallback. Campos ausentes fazem o gate falhar, então registram os metadados esperados nos tickets de mudança.- **Metados de transporte:** `scoreboard.json` deve declarar `transport_policy="direct_only"` (e virar `transport_policy_override=true` quando você forçar ou fazer downgrade). Mantenha os campos de anonimato pareados mesmo quando herdarem padrões para que os revisores vejam se houve desvio do plano de anonimato em fases.
- **Contadores de provedores:** Sessoes gateway-only devem persistir `provider_count=0` e preencher `gateway_provider_count=<n>` com o número de provedores Torii usados. Evite editar o JSON manualmente: o CLI/SDK já deriva das contagens e o portão de adoção rejeita capturas que omitem a separação.
- **Evidência de manifesto:** Quando gateways Torii participarem, passe o `--gateway-manifest-envelope <path>` aprovado (ou equivalente no SDK) para que `gateway_manifest_provided` mais `gateway_manifest_id`/`gateway_manifest_cid` sejam registrados em `scoreboard.json`. Garanta que `summary.json` carregue o mesmo `manifest_id`/`manifest_cid`; a verificação de adoção falha se qualquer arquivo omitir ou par.
- **Expectativas de telemetria:** Quando a telemetria acompanha uma captura, rode o portão com `--require-telemetry` para que o relatorio comprove que as métricas foram emitidas. Ensaios air-gapped podem omitir a bandeira, mas CI e tickets de mudanca devem ser documentados ausencia.

Exemplo:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

Anexo `adoption_report.json` junto ao placar, ao resumo, ao envelope de manifesto e ao pacote de toras de fumaça. Esses artefatos refletem o que o trabalho de adoção em CI (`ci/check_sorafs_orchestrator_adoption.sh`) aplica e mantém downgrades de modo direto auditáveis.