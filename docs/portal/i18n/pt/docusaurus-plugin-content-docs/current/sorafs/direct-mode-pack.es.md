---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pacote de modo direto
título: Pacote de contingência de modo direto de SoraFS (SNNet-5a)
sidebar_label: pacote de modo direto
description: Configuração necessária, testes de cumprimento e etapas de execução para operar SoraFS no modo direto Torii/QUIC durante a transição SNNet-5a.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/direct_mode_pack.md`. Mantenha ambas as cópias sincronizadas.
:::

Os circuitos SoraNet continuam sendo o transporte predeterminado para SoraFS, mas o item do roteiro **SNNet-5a** requer um fallback regulado para que os operadores mantenham um acesso de leitura determinado enquanto completam a aplicação de anonimato. Este pacote contém os botões CLI / SDK, os perfis de configuração, as verificações de cumprimento e a lista de aplicativos necessários para executar SoraFS no modo direto Torii/QUIC sem realizar os transportes de privacidade.

O substituto aplica-se a testes e ambientes de produção regulamentados até que SNNet-5 e SNNet-9 superem suas portas de preparação. Mantenha os artefatos de baixo junto com o material habitual de despliegue de SoraFS para que os operadores possam alternar entre os modos anônimo e direto sob demanda.

## 1. Sinalizadores de CLI e SDK

- `sorafs_cli fetch --transport-policy=direct-only ...` desativa a programação de relés e força o transporte Torii/QUIC. A ajuda do CLI agora lista `direct-only` como valor aceito.
- O SDK deve estabelecer `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` sempre que expõe uma alternância de "modo direto". As ligações geradas em `iroha::ClientOptions` e `iroha_android` reenviam a mesma enumeração.
- Os chicotes de gateway (`sorafs_fetch`, ligações de Python) podem interpretar a alternância direta somente por meio dos auxiliares Norito JSON compartilhados para que a automatização receba o mesmo comportamento.

Documente a bandeira em runbooks orientados aos parceiros e canalize os alternadores através de `iroha_config` em vez de variáveis ​​de ambiente.

## 2. Perfis de política do gateway

Use JSON de Norito para persistir uma configuração determinante do orquestrador. O perfil de exemplo em `docs/examples/sorafs_direct_mode_policy.json` codificado:

- `transport_policy: "direct_only"` — rechaza provedores que só anunciaram transportes de relé SoraNet.
- `max_providers: 2` — limita os peers direcionados aos endpoints Torii/QUIC mais confiáveis. Ajuste de acordo com as concessões de conformidade regionais.
- `telemetry_region: "regulated-eu"` — etiqueta as questões emitidas para painéis e auditorias que distinguem as ações de fallback.
- Prescrições de intenções conservadoras (`retry_budget: 2`, `provider_failure_threshold: 3`) para evitar mascarar gateways mal configurados.

Carregue o JSON por meio de `sorafs_cli fetch --config` (automatização) ou das ligações do SDK (`config_from_json`) antes de expor a política aos operadores. Persista a saída do placar (`persist_path`) para os resultados da auditoria.Os botões de aplicação do lado do gateway estão identificados em `docs/examples/sorafs_gateway_direct_mode.toml`. A planta reflete a saída de `iroha app sorafs gateway direct-mode enable`, desativando as comprovações de envelope/admissão, transferindo os valores por defeito de limite de taxa e poblando a tabela `direct_mode` com nomes de host descritos no plano e resumos do manifesto. Substitua os valores do marcador de posição pelo seu plano de implementação antes de versão do fragmento no gerenciamento de configuração.

## 3. Suíte de testes de cumprimento

A preparação do modo direto agora inclui cobertura tanto no orquestrador quanto nas caixas de CLI:

- `direct_only_policy_rejects_soranet_only_providers` garante que `TransportPolicy::DirectOnly` falha rapidamente quando cada anúncio de candidato solo suporta relés SoraNet.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` certifique-se de que você use transportes Torii/QUIC quando estiver presente e que os relés SoraNet sejam excluídos da sessão.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` analisa `docs/examples/sorafs_direct_mode_policy.json` para garantir que a documentação seja mantida alinhada com os auxiliares de utilidade.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` executa `sorafs_cli fetch --transport-policy=direct-only` contra um gateway Torii simulado, proporcionando uma experiência de humor para ambientes regulados que fijan transporta diretamente.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` envia o mesmo comando com o JSON de política e a persistência do placar para a automatização do lançamento.

Ejecuta la suite enfocada antes de publicar atualizações:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Se a compilação do espaço de trabalho falhar por mudanças no upstream, registre o erro bloqueado em `status.md` e execute novamente quando a dependência for atualizada.

## 4. Execuções automatizadas de fumaça

A cobertura de CLI por si só não revela restrições específicas do ambiente (por exemplo, derivadas de políticas de gateway ou desajustes de manifestos). Um ajudante de fumaça dedicado vive em `scripts/sorafs_direct_mode_smoke.sh` e envolve `sorafs_cli fetch` com a política de orquestrador de modo direto, a persistência do placar e a captura de currículos.

Exemplo de uso:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- O script respeita tantos sinalizadores CLI como arquivos de configuração key=value (consulte `docs/examples/sorafs_direct_mode_smoke.conf`). Releia o resumo do manifesto e as entradas de anúncios do provedor com valores de produção antes de executar.
- `--policy` por defeito é `docs/examples/sorafs_direct_mode_policy.json`, mas você pode fornecer qualquer JSON do orquestrador produzido por `sorafs_orchestrator::bindings::config_to_json`. A CLI aceita a política via `--orchestrator-config=PATH`, habilitando execuções reproduzíveis sem ajustar sinalizadores à mão.
- Quando `sorafs_cli` não está em `PATH`, o ajudante é compilado a partir da caixa `sorafs_orchestrator` (perfil release) para que os testes de humor exerçam o encanamento do modo direto que você envia.
-Salidas:
  - Carga útil empacotada (`--output`, por defeito `artifacts/sorafs_direct_mode/payload.bin`).
  - Resumo de busca (`--summary`, por defeito junto com a carga útil) que contém a região de telemetria e os relatórios de fornecedores usados ​​como evidência de implementação.
  - Instantâneo do placar persistido na rota declarada no JSON da política (por exemplo, `fetch_state/direct_mode_scoreboard.json`). Arquivamos junto com o currículo e tickets de mudança.
- Automatização da porta de adoção: uma vez que a busca termina o auxiliar invoca `cargo xtask sorafs-adoption-check` usando as rotas persistentes de placar e resumo. O quorum exigido por defeito é o número de provedores fornecidos na linha de comandos; anúlalo con `--min-providers=<n>` quando você precisa de uma exibição para prefeito. Os relatórios de adoção são escritos junto com o currículo (`--adoption-report=<path>` pode fornecer uma localização personalizada) e o auxiliar passa `--require-direct-only` por defeito (coincidindo com o substituto) e `--require-telemetry` sempre que exibe a bandeira correspondente. Use `XTASK_SORAFS_ADOPTION_FLAGS` para reenviar argumentos adicionais de xtask (por exemplo, `--allow-single-source` durante um downgrade aprovado para que o portão tolere e cumplir o fallback). Apenas omita o portão com `--skip-adoption-check` para executar diagnósticos locais; o roteiro exige que cada execução regulada de modo direto inclua o pacote de informações de adoção.

## 5. Lista de verificação de despliegue1. **Congelação de configuração:** salve o perfil JSON de modo direto em seu repositório `iroha_config` e registre o hash em seu ticket de mudança.
2. **Auditoria do gateway:** confirme que os endpoints Torii aplicam TLS, TLVs de capacidade e registro de auditoria antes de mudar para o modo direto. Publica o perfil da política do gateway para as operadoras.
3. **Aprovação de conformidade:** compare o playbook atualizado com revisores de conformidade/reguladores e capture as aprovações para operar fora da sobreposição de anonimato.
4. **Teste:** executa o conjunto de cumprimento mais uma busca em teste contra provedores de confiança Torii. Arquiva as saídas do placar e os resumos da CLI.
5. **Corte na produção:** anuncia a janela de mudança, mudança `transport_policy` para `direct_only` (se tiver optado por `soranet-first`) e monitora os dashboards de modo direto (latência de `sorafs_fetch`, contadores de falhas de fornecedores). Documente o plano de reversão para retornar ao SoraNet-first uma vez que SNNet-4/5/5a/5b/6a/7/8/12/13 se formou em `roadmap.md:532`.
6. **Revisão pós-mudança:** adiciona instantâneos do placar, resumos de busca e resultados de monitoramento ao ticket de mudança. Atualiza `status.md` com a data efetiva e qualquer anomalia.

Mantenha a lista de verificação junto com o runbook `sorafs_node_ops` para que os operadores possam ensaiar o fluxo antes de uma mudança ao vivo. Quando o SNNet-5 chega ao GA, o substituto após a confirmação da paridade na telemetria de produção é retirado.

## 6. Requisitos de evidência e portão de adoção

As capturas de modo direto ainda devem cumprir o portão de adoção SF-6c. Agrupe o placar, o currículo, o envelope de manifesto e o relatório de adoção em cada execução para que `cargo xtask sorafs-adoption-check` possa validar a postura de fallback. Os campos faltantes fizeram cair o portão, então registre os metadados esperados nos tickets de mudança.- **Metadados de transporte:** `scoreboard.json` deve declarar `transport_policy="direct_only"` (e ativar `transport_policy_override=true` quando forçar o downgrade). Mantenha os campos de política de anonimato emparejados, mesmo quando estes são padrões para que os revisores possam se desviar do plano de anonimato por etapas.
- **Contadores de provedores:** As sessões solo-gateway devem persistir `provider_count=0` e marcar `gateway_provider_count=<n>` com o número de provedores Torii usados. Evite editar o JSON manualmente: o CLI/SDK deriva do conteúdo e da porta de adoção rechaça capturas que omitem a separação.
- **Evidência do manifesto:** Quando os gateways participantes Torii, passe o `--gateway-manifest-envelope <path>` firmado (ou equivalente do SDK) para que `gateway_manifest_provided` e os `gateway_manifest_id`/`gateway_manifest_cid` sejam registrados em `scoreboard.json`. Certifique-se de que `summary.json` leve o mesmo `manifest_id`/`manifest_cid`; a prova de adoção falhou se qualquer um dos arquivos omitisse o par.
- **Expectativas de telemetria:** Quando a telemetria acompanha a captura, execute o portão com `--require-telemetry` para que o relatório possa ser emitido. Ensaios em ambientes isolados podem omitir a bandeira, mas CI e ingressos de mudança devem documentar a ausência.

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

Adjunto `adoption_report.json` junto com o placar, o resumo, o envelope de manifesto e o pacote de toras de fumaça. Esses artefatos refletem o que aplica o trabalho de adoção no CI (`ci/check_sorafs_orchestrator_adoption.sh`) e mantêm os downgrades auditáveis ​​​​de modo direto.