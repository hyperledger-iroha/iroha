---
lang: pt
direction: ltr
source: docs/source/nexus_operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 413953b2ca41875bb960be56995aa03dbaa58af4a30f927c24d1e8815c7da472
source_last_modified: "2025-11-08T16:26:57.335679+00:00"
translation_last_reviewed: 2026-01-01
---

# Runbook de operacoes de Nexus (NX-14)

**Link do roadmap:** NX-14 - documentacao de Nexus e runbooks de operadores
**Status:** Rascunho 2026-03-24 - alinhado com `docs/source/nexus_overview.md` e
com o fluxo de onboarding em `docs/source/sora_nexus_operator_onboarding.md`.
**Audiencia:** Operadores de rede, engenheiros SRE/on-call, coordenadores de governanca.

Este runbook resume o ciclo de vida operacional dos nos Sora Nexus (Iroha 3).
Ele nao substitui a especificacao detalhada (`docs/source/nexus.md`) nem os guias
por lane (ex., `docs/source/cbdc_lane_playbook.md`), mas reune os checklists
concretos, os hooks de telemetria e os requisitos de evidencia que precisam ser
cumpridos antes de admitir ou atualizar um no.

## 1. Ciclo de vida operacional

| Etapa | Checklist | Evidencia |
|-------|-----------|----------|
| **Pre-flight** | Validar hashes/assinaturas de artefatos, confirmar `profile = "iroha3"`, e preparar templates de config. | Saida de `scripts/select_release_profile.py`, log de checksum, bundle de manifest assinado. |
| **Alinhamento de catalogo** | Atualizar o catalogo de lane + dataspace em `[nexus]`, politica de roteamento e limites de DA para corresponder ao manifest emitido pelo conselho. | Saida de `irohad --sora --config ... --trace-config` armazenada com o ticket. |
| **Smoke e cutover** | Executar `irohad --sora --config ... --trace-config`, rodar smoke test de CLI (ex., `FindNetworkStatus`), verificar endpoints de telemetria, depois solicitar admissao. | Log de smoke-test + confirmacao de silencio no Alertmanager. |
| **Estado estavel** | Monitorar dashboards/alertas, rotacionar chaves conforme a cadencia de governanca, e manter configs + runbooks em sync com revisoes de manifest. | Atas de revisao trimestral, capturas de dashboards vinculadas, e IDs de tickets de rotacao. |

Instrucoes detalhadas de onboarding (incluindo substituicao de chaves, exemplos
de politica de roteamento e validacao de release profile) ficam em
`docs/source/sora_nexus_operator_onboarding.md`. Consulte esse documento quando
formatos de artefatos ou scripts mudarem.

## 2. Gestao de mudancas e hooks de governanca

1. **Atualizacoes de release**
   - Acompanhar anuncios em `status.md` e `roadmap.md`.
   - Cada PR de release deve anexar o checklist preenchido de
     `docs/source/sora_nexus_operator_onboarding.md`.
2. **Mudancas de lane manifest**
   - A governanca publica bundles de manifest assinados via Space Directory.
   - Operadores verificam assinaturas, atualizam entradas de catalogo e arquivam
     os manifests em `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuracao**
   - Toda mudanca em `config/config.toml` requer um ticket referenciando o lane ID
     e o alias de dataspace.
   - Manter uma copia redigida do config efetivo no ticket quando o no entra ou
     passa por upgrade.
4. **Simulacoes de rollback**
   - Realizar rehearsals trimestrais de rollback (parar o no, restaurar o bundle
     anterior, reaplicar config, reexecutar smoke). Registrar resultados em
     `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Aprovacoes de compliance**
   - Lanes privadas/CBDC devem obter sign-off de compliance antes de alterar
     politica de DA ou knobs de redacao de telemetria. Referenciar
     `docs/source/cbdc_lane_playbook.md#governance-hand-offs`.

## 3. Cobertura de telemetria e SLO

Dashboards e regras de alerta sao versionados em `dashboards/` e documentados em
`docs/source/nexus_telemetry_remediation_plan.md`. Operadores DEVEM:

- Assinar destinos PagerDuty/on-call em `dashboards/alerts/nexus_audit_rules.yml`
  e as regras de saude de lane em `dashboards/alerts/torii_norito_rpc_rules.yml`
  (cobre o transporte Torii/Norito).
- Publicar os seguintes boards Grafana no portal de operacoes:
  - `nexus_lanes.json` (altura da lane, backlog, paridade DA).
  - `nexus_settlement.json` (latencia de settlement, deltas de tesouraria).
  - `android_operator_console.json` / dashboards de SDK quando a lane depende de
    telemetria mobile.
- Manter exporters OTEL alinhados com `docs/source/torii/norito_rpc_telemetry.md`
  quando o transporte binario Torii estiver habilitado.
- Executar o checklist de remediation de telemetria ao menos trimestralmente
  (Secao 5 em `docs/source/nexus_telemetry_remediation_plan.md`) e anexar o
  formulario preenchido as atas de revisao de ops.

### Metricas chave

| Metrica | Descricao | Limite de alerta |
|--------|-----------|-----------------|
| `nexus_lane_height{lane_id}` | Altura de cabeca por lane; detecta validadores travados. | Alertar se nao houver aumento por 3 slots consecutivos. |
| `nexus_da_backlog_chunks{lane_id}` | Chunks de DA nao processados por lane. | Alertar acima do limite configurado (padrao: 64 public, 8 private). |
| `nexus_settlement_latency_seconds{lane_id}` | Tempo entre commit de lane e settlement global. | Alertar >900 ms P99 (public) ou >1200 ms (private). |
| `torii_request_failures_total{scheme="norito_rpc"}` | Contagem de erros Norito RPC. | Alertar se a razao de erros de 5 minutos >2%. |
| `telemetry_redaction_override_total` | Overrides emitidos para redacao de telemetria. | Alertar imediatamente (Sev 2) e exigir ticket de compliance. |

## 4. Resposta a incidentes

| Severidade | Definicao | Acoes obrigatorias |
|----------|-----------|-------------------|
| **Sev 1** | Brecha de isolamento de data space, parada de settlement >15 min, ou corrupcao de voto de governanca. | Pagear Nexus Primary + Release Engineering + Compliance. Congelar admissao de lane, coletar metricas/logs, publicar comunicacao do incidente em 60 min, registrar RCA em <=5 dias uteis. |
| **Sev 2** | Backlog de lane excedendo SLA, ponto cego de telemetria >30 min, rollout de manifest falho. | Pagear Nexus Primary + SRE, mitigar em 4 h, registrar issues de seguimento em 2 dias uteis. |
| **Sev 3** | Regressoes nao bloqueantes (drift de docs, alerta disparada indevidamente). | Registrar no tracker, agendar correcao dentro do sprint. |

Tickets de incidentes devem incluir:

1. IDs de lane/data-space afetados e hashes de manifest.
2. Timeline (UTC) com deteccao, mitigacao, recuperacao e comunicacoes.
3. Metricas/capturas que suportem a deteccao.
4. Tarefas de acompanhamento (com owners/datas) e se automacao/runbooks precisam
   de atualizacoes.

## 5. Evidencia e trilha de auditoria

- **Arquivo de artefatos:** Armazenar bundles, manifests e exportes de telemetria em
  `artifacts/nexus/<lane>/<date>/`.
- **Snapshots de config:** `config.toml` redigido + saida `trace-config` para cada release.
- **Vinculo de governanca:** Atas do conselho e decisoes assinadas referenciadas
  no ticket de onboarding ou incidente.
- **Exportes de telemetria:** Snapshots semanais de chunks TSDB do Prometheus
  relacionados a lane, anexados ao share de auditoria por no minimo 12 meses.
- **Versionamento do runbook:** Toda mudanca significativa neste arquivo deve incluir
  uma entrada de changelog em `docs/source/project_tracker/nexus_config_deltas/README.md`
  para que auditores possam rastrear quando os requisitos mudaram.

## 6. Recursos relacionados

- `docs/source/nexus_overview.md` - arquitetura/resumo de alto nivel.
- `docs/source/nexus.md` - especificacao tecnica completa.
- `docs/source/nexus_lanes.md` - geometria de lanes.
- `docs/source/nexus_transition_notes.md` - roadmap de migracao.
- `docs/source/cbdc_lane_playbook.md` - politicas especificas de CBDC.
- `docs/source/sora_nexus_operator_onboarding.md` - fluxo de release/onboarding.
- `docs/source/nexus_telemetry_remediation_plan.md` - guardrails de telemetria.

Manter essas referencias atualizadas quando o item NX-14 avancar ou quando novas
classes de lane, regras de telemetria ou hooks de governanca forem introduzidos.
