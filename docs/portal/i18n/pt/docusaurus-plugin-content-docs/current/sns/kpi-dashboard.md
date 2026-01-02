---
id: kpi-dashboard
lang: pt
direction: ltr
source: docs/portal/docs/sns/kpi-dashboard.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---
# Painel KPI do Servico de nomes Sora

O painel de KPI oferece a stewards, guardians e reguladores um unico lugar para revisar sinais de adocao, erro e receita antes da cadencia mensal do anexo (SN-8a). A definicao do Grafana esta no repositorio em `dashboards/grafana/sns_suffix_analytics.json` e o portal espelha os mesmos paineis via um iframe embutido para que a experiencia corresponda a instancia interna do Grafana.

## Filtros e fontes de dados

- **Filtro de sufixo** - dirige as consultas `sns_registrar_status_total{suffix}` para que `.sora`, `.nexus` e `.dao` sejam inspecionados de forma independente.
- **Filtro de liberacao em massa** - delimita as metricas `sns_bulk_release_payment_*` para que financas possa conciliar um manifesto de registrador especifico.
- **Metricas** - puxa de Torii (`sns_registrar_status_total`, `torii_request_duration_seconds`), da CLI guardian (`guardian_freeze_active`), `sns_governance_activation_total`, e das metricas do helper de onboarding em massa.

## Paineis

1. **Registros (ultimas 24h)** - numero de eventos de registrador bem-sucedidos para o sufixo selecionado.
2. **Ativacoes de governanca (30d)** - mocoes de carta/adendo registradas pela CLI.
3. **Throughput do registrador** - taxa por sufixo de acoes bem-sucedidas do registrador.
4. **Modos de erro do registrador** - taxa de 5 minutos dos contadores `sns_registrar_status_total` rotulados como erro.
5. **Janelas de freeze do guardian** - seletores ativos em que `guardian_freeze_active` reporta um ticket de freeze aberto.
6. **Unidades liquidas de pagamento por ativo** - totais reportados por `sns_bulk_release_payment_net_units` por ativo.
7. **Requisicoes em massa por sufixo** - volumes de manifesto por id de sufixo.
8. **Unidades liquidas por requisicao** - calculo estilo ARPU derivado das metricas de release.

## Checklist mensal de revisao de KPI

O lider financeiro conduz uma revisao recorrente na primeira terca-feira de cada mes:

1. Abra a pagina do portal **Analytics -> SNS KPI** (ou o dashboard Grafana `sns-kpis`).
2. Capture uma exportacao PDF/CSV das tabelas de throughput do registrador e de receita.
3. Compare sufixos para violacoes de SLA (picos de taxa de erro, seletores congelados >72 h, deltas de ARPU >10 %).
4. Registre resumos + itens de acao na entrada de anexo correspondente em `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
5. Anexe os artefatos exportados do dashboard ao commit do anexo e vincule-os na agenda do conselho.

Se a revisao encontrar violacoes de SLA, abra um incidente PagerDuty para o responsavel afetado (registrar duty manager, guardian on-call ou steward program lead) e acompanhe a remediacao no log do anexo.
