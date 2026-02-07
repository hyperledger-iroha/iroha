---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pacote de modo direto
título: Pacote de resposta em modo direto SoraFS (SNNet-5a)
sidebar_label: Modo pacote direto
descrição: Requisitos de configuração, controles de conformidade e etapas de implantação durante a exploração de SoraFS em modo direto Torii/QUIC durante a transição SNNet-5a.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/direct_mode_pack.md`. Gardez les duas cópias sincronizadas até o retrato do conjunto histórico da Esfinge.
:::

Os circuitos SoraNet continuam sendo transportados por padrão para SoraFS, mas o item do roteiro **SNNet-5a** exige uma resposta regulamentada para que os operadores possam manter um acesso em uma palestra determinada até que a implantação do anonimato termine. Este pacote reúne os botões CLI/SDK, os perfis de configuração, os testes de conformidade e a lista de verificação de implementação necessária para executar SoraFS no modo direto Torii/QUIC sem toque em transporte confidencial.

A resposta é aplicada em ambientes de preparação e produção regulamentados, desde que SNNet-5 e SNNet-9 sejam franqueados em seus portões de prontidão. Guarde os artefatos aqui com o material de implantação SoraFS habitual para que os operadores possam navegar entre os modos anônimos e diretos à demanda.

## 1. Sinalizadores CLI e SDK

- `sorafs_cli fetch --transport-policy=direct-only ...` desativa a ordem de relais e impõe os transportes Torii/QUIC. O assistente do CLI lista `direct-only` desordenado como valor aceito.
- O SDK deve definir `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` para expor uma alternância de "modo direto". As ligações geradas em `iroha::ClientOptions` e `iroha_android` propagam a mesma enumeração.
- Os chicotes de gateway (`sorafs_fetch`, ligações Python) podem analisar a alternância direta somente por meio dos auxiliares Norito JSON compartilhados para que a automação obtenha o mesmo comportamento.

Documente a bandeira nos runbooks orientados para os parceiros e faça as alternâncias via `iroha_config` tanto quanto as variáveis ​​de ambiente.

## 2. Perfis de política do gateway

Use JSON Norito para persistir uma configuração determinada do orquestrador. O perfil de exemplo na codificação `docs/examples/sorafs_direct_mode_policy.json`:

- `transport_policy: "direct_only"` — rejeite os provedores que não anunciaram o transporte relacionado ao SoraNet.
- `max_providers: 2` — limita os pares direciona os endpoints auxiliares Torii/QUIC arquivos mais fiáveis. Ajuste as restrições de conformidade regional de acordo com as regras.
- `telemetry_region: "regulated-eu"` — etiqueta as métricas emitidas para que os painéis e auditorias distingam as execuções de réplica.
- Orçamentos de novos conservadores (`retry_budget: 2`, `provider_failure_threshold: 3`) para evitar mascarar gateways mal configurados.

Carregue o JSON via `sorafs_cli fetch --config` (automatização) ou através do SDK de ligação (`config_from_json`) antes de expor a política aos operadores. Mantenha a classificação do placar (`persist_path`) nas pistas de auditoria.As configurações do aplicativo côté gateway foram capturadas em `docs/examples/sorafs_gateway_direct_mode.toml`. O modelo reflete a saída de `iroha app sorafs gateway direct-mode enable`, desativa as verificações de envelope/admissão, altera os valores padrão de limite de taxa e substitui a tabela `direct_mode` com os nomes de host derivados do plano e dos resumos do manifesto. Substitua os valores de espaço reservados com seu plano de implementação antes de atualizar o extrato no gerenciamento de configuração.

## 3. Conjunto de testes de conformidade

A prontidão do modo direto inclui a distribuição de uma cobertura no orquestrador e nas caixas CLI:

- `direct_only_policy_rejects_soranet_only_providers` garante que `TransportPolicy::DirectOnly` seja ouvido rapidamente quando cada anúncio de candidato não for responsável pelo relacionamento SoraNet.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` garante que os transportes Torii/QUIC sejam utilizados quando estiverem disponíveis e que os relés SoraNet sejam exclusivos da sessão.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` analisa `docs/examples/sorafs_direct_mode_policy.json` para garantir que a documentação esteja alinhada com os utilitários auxiliares.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` exerce `sorafs_cli fetch --transport-policy=direct-only` contra um gateway Torii simulado, fornecendo um teste de fumaça para os ambientes regulamentados que transmitem os transportes diretos.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` envia o mesmo comando com o JSON da política e a persistência do placar para a automação do lançamento.

Execute a suíte ciblée antes de publicar as mises do dia:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Se a compilação do espaço de trabalho for causada por alterações no upstream, registre o erro bloqueado em `status.md` e execute um após a dependência atual.

## 4. Execuções de fumaça automatizadas

A cobertura CLI apenas não revela as regressões específicas ao ambiente (por exemplo, derivadas de políticas de gateway ou inadequações de manifesto). Um ajudante de fumaça morreu em `scripts/sorafs_direct_mode_smoke.sh` e no envelope `sorafs_cli fetch` com a política de orquestrador em modo direto, a persistência do placar e a captura do currículo.

Exemplo de utilização:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- O script respeita os sinalizadores CLI e os arquivos de configuração key=value (veja `docs/examples/sorafs_direct_mode_smoke.conf`). Leia o resumo do manifesto e as entradas de anúncios dos fornecedores com os valores de produção antes da execução.
- `--policy` é apontado por padrão em relação a `docs/examples/sorafs_direct_mode_policy.json`, mas todo JSON do orquestrador produzido por `sorafs_orchestrator::bindings::config_to_json` pode ser encontrado. A CLI aceita a política via `--orchestrator-config=PATH`, permitindo a execução reproduzível sem ajustar as bandeiras à principal.
- Quando `sorafs_cli` não está em `PATH`, ele ajuda a construir a partir da caixa `sorafs_orchestrator` (liberação de perfil) para que a fumaça exerça o encanamento no modo direto livre.
- Sorteios:
  - Montagem de carga útil (`--output`, por padrão `artifacts/sorafs_direct_mode/payload.bin`).
  - Resumo de busca (`--summary`, por padrão no local da carga útil) contendo a região de telefonia e os relatórios de provedores utilizados para a evidência de implementação.
  - O instantâneo do placar persiste no caminho declarado no JSON da política (por exemplo, `fetch_state/direct_mode_scoreboard.json`). Arquive-o com o currículo nos bilhetes de mudança.
- Automatização da porta de adoção: uma vez que a busca terminou, o ajudante invoca `cargo xtask sorafs-adoption-check` usando os caminhos persistentes do placar e do resumo. O quórum exigido por padrão corresponde ao nome dos provedores fornecidos na linha de comando; substitua-o por `--min-providers=<n>` quando você precisar de um échantillon plus grande. Os relatórios de adoção são escritos no côté do currículo (`--adoption-report=<path>` podem definir um local personalizado) e o auxiliar passa `--require-direct-only` por padrão (alinhado na réplica) e `--require-telemetry` quando você fornece a bandeira correspondente. Use `XTASK_SORAFS_ADOPTION_FLAGS` para transmitir argumentos xtask suplementares (por exemplo, `--allow-single-source` durante um downgrade aprovado para que o portão tolere e imponha a resposta). Não sautez a porta de adoção com `--skip-adoption-check` que durante o diagnóstico local; o roteiro exige que cada execução regulamentada em modo direto inclua o pacote de relacionamento de adoção.

## 5. Checklist de implantação1. **Gel de configuração:** armazene o perfil JSON do modo direto em seu depósito `iroha_config` e envie o hash em seu ticket de alteração.
2. **Gateway de auditoria:** confirme se os endpoints Torii aplicam TLS, TLV de capacidade e registro de auditoria antes de baixá-los no modo direto. Publique o perfil do gateway político para os operadores.
3. **Conformidade de validação:** compartilhe o playbook agora com os controles de conformidade / regulamentos e capture as aprovações para operar fora da sobreposição anônima.
4. **Teste:** execute o conjunto de conformidade e busque o teste com os provedores de confiança Torii. Arquive as classificações do placar e os currículos CLI.
5. **Produção básica:** anuncie a janela de mudança, passe `transport_policy` para `direct_only` (se você optar por `soranet-first`) e observe os painéis no modo direto (latência `sorafs_fetch`, compteurs d'échec dos provedores). Documente o plano de reversão para retornar à SoraNet - primeiro uma vez que SNNet-4/5/5a/5b/6a/7/8/12/13 foi aprovado em `roadmap.md:532`.
6. **Revisar pós-mudança:** anexe os instantâneos do placar, os currículos de busca e os resultados de monitoramento do ticket de mudança. Coloque o `status.md` no dia atual com a data efetiva e qualquer anomalia.

Guarde a lista de verificação no topo do runbook `sorafs_node_ops` para que os operadores possam repetir o fluxo antes de uma roda basculante. Quando SNNet-5 passou GA, retire a resposta depois de confirmar a paridade na transmissão de produção.

## 6. Exigências de pré-prova e porta de adoção

As capturas no modo direto devem ser sempre satisfatórias para a adoção do SF-6c. Reagrupe o placar, o resumo, o envelope de manifesto e o relatório de adoção para cada execução até `cargo xtask sorafs-adoption-check` para validar a postura de resposta. Les champs manquants font ecoou le gate, consignez donc les métadonnées presentes nos bilhetes de troca.- **Métados de transporte :** `scoreboard.json` faça o declarador `transport_policy="direct_only"` (e basculante `transport_policy_override=true` quando você forçar o downgrade). Gardez os campeões de política de anonimato associados também são suficientes para herdar os valores por padrão, pois os leitores voam se você tiver desviado do plano de anonimato por etapas.
- **Computadores de provedores:** As sessões somente gateway devem persistir `provider_count=0` e substituir `gateway_provider_count=<n>` com o nome de provedores Torii utilizados. Evite editar o JSON na página principal: o CLI/SDK derrubou os computadores e o portão de adoção rejeitou as capturas que realizaram a separação.
- **Pressione o manifesto:** Quando os gateways Torii forem participantes, passe o sinal `--gateway-manifest-envelope <path>` (ou o SDK equivalente) para que `gateway_manifest_provided` e os `gateway_manifest_id`/`gateway_manifest_cid` sejam registrados em `scoreboard.json`. Certifique-se de que `summary.json` porta o mesmo `manifest_id`/`manifest_cid` ; a verificação da adoção é ouvida se um arquivo for colocado no par.
- **Atendimentos de telemetria:** Quando a telemetria acompanha a captura, execute o portão com `--require-telemetry` para que o relacionamento de adoção prove que as métricas foram emitidas. As repetições em air-gapped podem omitir a bandeira, mas o CI e os bilhetes de mudança devem documentar a ausência.

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

Anexe `adoption_report.json` com o placar, o resumo, o envelope de manifesto e o pacote de toras de fumaça. Esses artefatos refletem que o trabalho de adoção do CI (`ci/check_sorafs_orchestrator_adoption.sh`) impõe e torna os downgrades em modo auditável diretamente.