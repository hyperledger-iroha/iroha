<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
id: direct-mode-pack
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/direct_mode_pack.md`. Mantenha ambas as copias sincronizadas.
:::

Os circuitos SoraNet seguem como transporte padrao do SoraFS, mas o item de roadmap **SNNet-5a** exige um fallback regulado para que operadores mantenham acesso de leitura deterministico enquanto o rollout de anonimato se completa. Este pacote captura os knobs de CLI/SDK, perfis de configuracao, testes de compliance e o checklist de deploy necessario para rodar o SoraFS em modo direto Torii/QUIC sem tocar os transportes de privacidade.

O fallback se aplica a staging e ambientes de producao regulada ate que SNNet-5 a SNNet-9 passem pelos gates de prontidao. Mantenha os artefatos abaixo junto com o material de deploy do SoraFS para que operadores alternem entre modos anonimo e direto sob demanda.

## 1. Flags de CLI e SDK

- `sorafs_cli fetch --transport-policy=direct-only ...` desativa o agendamento de relays e impoe transportes Torii/QUIC. A ajuda do CLI agora lista `direct-only` como valor aceito.
- SDKs devem definir `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` sempre que expuserem um toggle de "modo direto". As bindings geradas em `iroha::ClientOptions` e `iroha_android` encaminham o mesmo enum.
- Harnesses de gateway (`sorafs_fetch`, bindings Python) podem interpretar o toggle direct-only via helpers Norito JSON compartilhados para que a automacao receba o mesmo comportamento.

Documente o flag em runbooks voltados a parceiros e passe os toggles via `iroha_config` em vez de variaveis de ambiente.

## 2. Perfis de politica do gateway

Use Norito JSON para persistir configuracao deterministica do orquestrador. O perfil de exemplo em `docs/examples/sorafs_direct_mode_policy.json` codifica:

- `transport_policy: "direct_only"` - rejeita provedores que so anunciam transportes de relay SoraNet.
- `max_providers: 2` - limita peers diretos aos endpoints Torii/QUIC mais confiaveis. Ajuste conforme as concessoes de compliance regionais.
- `telemetry_region: "regulated-eu"` - rotula as metricas emitidas para que dashboards e auditorias distingam execucoes de fallback.
- Orcamentos de retry conservadores (`retry_budget: 2`, `provider_failure_threshold: 3`) para evitar mascarar gateways mal configurados.

Carregue o JSON via `sorafs_cli fetch --config` (automacao) ou bindings do SDK (`config_from_json`) antes de expor a politica a operadores. Persista a saida do scoreboard (`persist_path`) para trilhas de auditoria.

Os knobs de enforcement do lado do gateway estao em `docs/examples/sorafs_gateway_direct_mode.toml`. O template espelha a saida de `iroha sorafs gateway direct-mode enable`, desabilitando checagens de envelope/admission, conectando defaults de rate-limit e preenchendo a tabela `direct_mode` com hostnames derivados do plano e digests de manifest. Substitua os valores de placeholder pelo seu plano de rollout antes de versionar o trecho na gestao de configuracao.

## 3. Suite de testes de compliance

A prontidao do modo direto agora inclui cobertura tanto no orquestrador quanto nos crates de CLI:

- `direct_only_policy_rejects_soranet_only_providers` garante que `TransportPolicy::DirectOnly` falhe rapido quando cada advert candidato so suporta relays SoraNet. [crates/sorafs_orchestrator/src/lib.rs:7238]
- `direct_only_policy_prefers_direct_transports_when_available` garante que transportes Torii/QUIC sejam usados quando disponiveis e que relays SoraNet sejam excluidos da sessao. [crates/sorafs_orchestrator/src/lib.rs:7285]
- `direct_mode_policy_example_is_valid` faz parse de `docs/examples/sorafs_direct_mode_policy.json` para garantir que a documentacao permanece alinhada aos helpers. [crates/sorafs_orchestrator/src/lib.rs:7509] [docs/examples/sorafs_direct_mode_policy.json:1]
- `fetch_command_respects_direct_transports` exercita `sorafs_cli fetch --transport-policy=direct-only` contra um gateway Torii simulado, fornecendo um smoke test para ambientes regulados que fixam transportes diretos. [crates/sorafs_car/tests/sorafs_cli.rs:2733]
- `scripts/sorafs_direct_mode_smoke.sh` envolve o mesmo comando com o JSON de politica e a persistencia do scoreboard para automacao de rollout.

Rode a suite focada antes de publicar atualizacoes:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Se a compilacao do workspace falhar por mudancas upstream, registre o erro bloqueador em `status.md` e rode novamente quando a dependencia atualizar.

## 4. Runs automatizados de smoke

A cobertura do CLI sozinha nao revela regressoes especificas do ambiente (por exemplo, drift de politica do gateway ou divergencias de manifest). Um helper de smoke dedicado vive em `scripts/sorafs_direct_mode_smoke.sh` e envolve `sorafs_cli fetch` com a politica do orquestrador em modo direto, persistencia do scoreboard e captura de resumo.

Exemplo de uso:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- O script respeita flags de CLI e arquivos de configuracao key=value (veja `docs/examples/sorafs_direct_mode_smoke.conf`). Preencha o digest do manifest e as entradas de advert de provedor com valores de producao antes de rodar.
- `--policy` tem como padrao `docs/examples/sorafs_direct_mode_policy.json`, mas qualquer JSON de orquestrador produzido por `sorafs_orchestrator::bindings::config_to_json` pode ser fornecido. O CLI aceita a politica via `--orchestrator-config=PATH`, habilitando runs reproduziveis sem ajustar flags manualmente.
- Quando `sorafs_cli` nao esta no `PATH`, o helper compila a partir do crate `sorafs_orchestrator` (perfil release) para que os smokes exercitem o plumbing de modo direto enviado.
- Saidas:
  - Payload montado (`--output`, padrao `artifacts/sorafs_direct_mode/payload.bin`).
  - Resumo de fetch (`--summary`, padrao ao lado do payload) contendo a regiao de telemetria e relatorios de provedores usados como evidencia de rollout.
  - Snapshot de scoreboard persistido no caminho declarado no JSON de politica (por exemplo, `fetch_state/direct_mode_scoreboard.json`). Arquive junto ao resumo em tickets de mudanca.
- Automacao do gate de adocao: apos o fetch, o helper invoca `cargo xtask sorafs-adoption-check` usando os caminhos persistidos de scoreboard e summary. O quorum requerido por padrao e o numero de provedores fornecidos na linha de comando; sobrescreva com `--min-providers=<n>` quando precisar de uma amostra maior. Relatorios de adocao sao gravados ao lado do resumo (`--adoption-report=<path>` pode definir um local customizado) e o helper passa `--require-direct-only` por padrao (alinhado ao fallback) e `--require-telemetry` sempre que voce fornecer o flag correspondente. Use `XTASK_SORAFS_ADOPTION_FLAGS` para repassar argumentos extras do xtask (por exemplo `--allow-single-source` durante um downgrade aprovado para que o gate tolere e imponha o fallback). So pule o gate com `--skip-adoption-check` ao rodar diagnosticos locais; o roadmap exige que cada run regulado em modo direto inclua o bundle de relatorio de adocao.

## 5. Checklist de rollout

1. **Freeze de configuracao:** armazene o perfil JSON de modo direto no repositorio `iroha_config` e registre o hash no ticket de mudanca.
2. **Auditoria de gateway:** confirme que endpoints Torii aplicam TLS, TLVs de capacidade e logging de auditoria antes de virar para modo direto. Publique o perfil de politica do gateway para os operadores.
3. **Sign-off de compliance:** compartilhe o playbook atualizado com revisores de compliance/regulatorios e capture aprovacoes para operar fora do overlay de anonimato.
4. **Dry run:** execute a suite de compliance mais um fetch de staging contra provedores Torii confiaveis. Arquive outputs de scoreboard e resumos do CLI.
5. **Cutover em producao:** anuncie a janela de mudanca, altere `transport_policy` para `direct_only` (se voce tinha optado por `soranet-first`) e monitore os dashboards de modo direto (latencia de `sorafs_fetch`, contadores de falha de provedores). Documente o plano de rollback para voltar ao SoraNet-first quando SNNet-4/5/5a/5b/6a/7/8/12/13 graduarem em `roadmap.md:532`.
6. **Revisao pos-mudanca:** anexe snapshots do scoreboard, resumos de fetch e resultados de monitoramento ao ticket de mudanca. Atualize `status.md` com a data efetiva e qualquer anomalia.

Mantenha o checklist junto ao runbook `sorafs_node_ops` para que operadores possam ensaiar o fluxo antes de uma virada ao vivo. Quando SNNet-5 chegar a GA, retire o fallback apos confirmar paridade na telemetria de producao.

## 6. Requisitos de evidencia e gate de adocao

Capturas em modo direto ainda precisam satisfazer o gate de adocao SF-6c. Bundle o scoreboard, o resumo, o envelope de manifest e o relatorio de adocao em cada run para que `cargo xtask sorafs-adoption-check` valide a postura de fallback. Campos ausentes fazem o gate falhar, entao registre o metadata esperado nos tickets de mudanca.

- **Metadados de transporte:** `scoreboard.json` deve declarar `transport_policy="direct_only"` (e virar `transport_policy_override=true` quando voce forcar o downgrade). Mantenha os campos de politica de anonimato pareados mesmo quando herdarem defaults para que revisores vejam se houve desvio do plano de anonimato em fases.
- **Contadores de provedores:** Sessoes gateway-only devem persistir `provider_count=0` e preencher `gateway_provider_count=<n>` com o numero de provedores Torii usados. Evite editar o JSON manualmente: o CLI/SDK ja deriva as contagens e o gate de adocao rejeita capturas que omitem a separacao.
- **Evidencia de manifest:** Quando gateways Torii participarem, passe o `--gateway-manifest-envelope <path>` assinado (ou equivalente no SDK) para que `gateway_manifest_provided` mais `gateway_manifest_id`/`gateway_manifest_cid` sejam registrados em `scoreboard.json`. Garanta que `summary.json` carregue o mesmo `manifest_id`/`manifest_cid`; a checagem de adocao falha se qualquer arquivo omitir o par.
- **Expectativas de telemetria:** Quando a telemetria acompanhar a captura, rode o gate com `--require-telemetry` para que o relatorio prove que metricas foram emitidas. Ensaios air-gapped podem omitir o flag, mas CI e tickets de mudanca devem documentar a ausencia.

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

Anexe `adoption_report.json` junto ao scoreboard, ao summary, ao envelope de manifest e ao bundle de logs de smoke. Esses artefatos espelham o que o job de adocao em CI (`ci/check_sorafs_orchestrator_adoption.sh`) aplica e mantem downgrades de modo direto auditaveis.
