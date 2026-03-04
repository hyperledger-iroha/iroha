---
lang: pt
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Metricas SNS e kit de onboarding

O item de roadmap **SN-8** agrupa duas promessas:

1. Publicar dashboards que exponham registros, renovacoes, ARPU, disputas e janelas de freeze para `.sora`, `.nexus`, e `.dao`.
2. Entregar um kit de onboarding para que registrars e stewards conectem DNS, pricing e APIs de forma consistente antes que qualquer sufixo entre no ar.

Esta pagina espelha a versao fonte
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
para que revisores externos sigam o mesmo procedimento.

## 1. Pacote de metricas

### Dashboard Grafana e embed do portal

- Importe `dashboards/grafana/sns_suffix_analytics.json` no Grafana (ou outro host de analitica)
  via a API padrao:

```bash
curl -H "Content-Type: application/json"          -H "Authorization: Bearer ${GRAFANA_TOKEN}"          -X POST https://grafana.sora.net/api/dashboards/db          --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- O mesmo JSON alimenta o iframe desta pagina do portal (ver **SNS KPI Dashboard**).
  Sempre que atualizar o dashboard, rode
  `npm run build && npm run serve-verified-preview` dentro de `docs/portal` para
  confirmar que Grafana e o embed permanecem sincronizados.

### Paineis e evidencia

| Painel | Metricas | Evidencia de governanca |
|-------|---------|-------------------------|
| Registros e renovacoes | `sns_registrar_status_total` (success + renewal resolver labels) | Throughput por sufixo + acompanhamento de SLA. |
| ARPU / unidades liquidas | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | Finance consegue casar manifests do registrar com receitas. |
| Disputas e freezes | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | Mostra freezes ativos, cadencia de arbitragem e carga do guardian. |
| SLA / taxas de erro | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | Destaca regressoes de API antes de impactar clientes. |
| Tracker de manifest bulk | `sns_bulk_release_manifest_total`, metricas de pagamento com labels `manifest_id` | Conecta drops CSV a tickets de settlement. |

Exporte um PDF/CSV do Grafana (ou do iframe embutido) durante a revisao mensal de KPI
e anexe a entrada de anexo correspondente em
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Os stewards tambem capturam o SHA-256
do bundle exportado em `docs/source/sns/reports/` (por exemplo,
`steward_scorecard_2026q1.md`) para que auditorias possam reproduzir a trilha de evidencia.

### Automacao de anexos

Gere arquivos de anexo diretamente da exportacao do dashboard para que os
revisores recebam um resumo consistente:

```bash
cargo xtask sns-annex       --suffix .sora       --cycle 2026-03       --dashboard dashboards/grafana/sns_suffix_analytics.json       --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json       --output docs/source/sns/reports/.sora/2026-03.md       --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md       --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- O helper hasheia a exportacao, captura UID/tags/numero de paineis e escreve
  um anexo Markdown em `docs/source/sns/reports/.<suffix>/<cycle>.md` (veja o exemplo
  `.sora/2026-03` junto deste doc).
- `--dashboard-artifact` copia a exportacao para
  `artifacts/sns/regulatory/<suffix>/<cycle>/` para que o anexo aponte para o caminho
  de evidencia canonica; use `--dashboard-label` apenas se precisar apontar para
  um arquivo fora de banda.
- `--regulatory-entry` aponta para o memo regulatorio. O helper insere (ou substitui)
  um bloco `KPI Dashboard Annex` que registra o caminho do anexo, o artefato do dashboard,
  o digest e o timestamp para manter a evidencia sincronizada.
- `--portal-entry` mantem alinhada a copia Docusaurus
  (`docs/portal/docs/sns/regulatory/*.md`) para que revisores nao precisem
  comparar resumos de anexo separados manualmente.
- Se pular `--regulatory-entry`/`--portal-entry`, anexe o arquivo gerado aos
  memos manualmente e ainda assim envie os snapshots PDF/CSV capturados no Grafana.
- Para exportacoes recorrentes, liste os pares sufixo/ciclo em
  `docs/source/sns/regulatory/annex_jobs.json` e execute
  `python3 scripts/run_sns_annex_jobs.py --verbose`. O helper percorre cada entrada,
  copia a exportacao do dashboard (por padrao `dashboards/grafana/sns_suffix_analytics.json`
  quando nao especificado) e atualiza o bloco de anexo dentro de cada memo regulatorio
  (e, quando disponivel, memo do portal) em uma unica passagem.
- Execute `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (ou `make check-sns-annex`) para provar que a lista de jobs permanece ordenada/sem duplicatas, que cada memo contem o marcador `sns-annex` e que o stub de anexo existe. O helper grava `artifacts/sns/annex_schedule_summary.json` ao lado dos resumos de locale/hash usados em pacotes de governanca.
Isso remove passos manuais de copiar/colar e mantem a evidencia do anexo SN-8 consistente enquanto protege contra drift de agenda, marcador e localizacao no CI.

## 2. Componentes do kit de onboarding

### Wiring de sufixo

- Esquema de registro + regras de selector:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  e [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md).
- Helper de esqueleto DNS:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  com o fluxo de ensaio documentado no
  [runbook de gateway/DNS](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md).
- Para cada lancamento de registrar, arquive uma nota curta em
  `docs/source/sns/reports/` resumindo amostras de selector, provas GAR e hashes DNS.

### Cheatsheet de precos

| Tamanho do label | Tarifa base (USD equiv) |
|--------------|---------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6-9 | $12 |
| 10+ | $8 |

Coeficientes de sufixo: `.sora` = 1.0x, `.nexus` = 0.8x, `.dao` = 1.3x.  
Multiplicadores de termo: 2-year -5%, 5-year -12%; grace window = 30 days, redemption
= 60 days (20% fee, min $5, max $200). Registre desvios negociados no
ticket do registrar.

### Leiloes premium vs renovacoes

1. **Pool premium** -- commit/reveal com lance selado (SN-3). Acompanhe os bids com
   `sns_premium_commit_total` e publique o manifest em
   `docs/source/sns/reports/`.
2. **Dutch reopen** -- apos expirar grace + redemption, inicie uma venda Dutch de 7-day
   a 10x que decai 15% ao dia. Etiquete manifests com `manifest_id` para que o
   dashboard mostre o progresso.
3. **Renovacoes** -- monitore `sns_registrar_status_total{resolver="renewal"}` e
   capture o checklist de autorenew (notificacoes, SLA, rails de pagamento de fallback)
   dentro do ticket do registrar.

### APIs de desenvolvedor e automacao

- Contratos de API: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- Helper bulk e esquema CSV:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- Comando de exemplo:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv       --ndjson artifacts/sns/releases/2026q2/requests.ndjson       --submission-log artifacts/sns/releases/2026q2/submissions.log       --submit-torii-url https://torii.sora.net       --submit-token-file ~/.config/sora/tokens/registrar.token
```

Inclua o manifest ID (saida de `--submission-log`) no filtro do dashboard KPI
para que finance possa reconciliar os paineis de receita por release.

### Bundle de evidencia

1. Ticket do registrar com contatos, escopo do sufixo e rails de pagamento.
2. Evidencia DNS/resolver (zonefile skeletons + provas GAR).
3. Planilha de precos + overrides aprovados por governanca.
4. Artefatos de smoke-test de API/CLI (exemplos `curl`, transcripts de CLI).
5. Screenshot do dashboard KPI + exportacao CSV, anexada ao anexo mensal.

## 3. Checklist de lancamento

| Passo | Owner | Artefato |
|------|-------|----------|
| Dashboard importado | Product Analytics | Resposta da API Grafana + UID do dashboard |
| Embed do portal validado | Docs/DevRel | logs `npm run build` + screenshot de preview |
| Ensaio DNS completo | Networking/Ops | saidas de `sns_zonefile_skeleton.py` + log do runbook |
| Dry run da automacao do registrar | Registrar Eng | log de submissions `sns_bulk_onboard.py` |
| Evidencia de governanca arquivada | Governance Council | link do anexo + SHA-256 do dashboard exportado |

Complete a checklist antes de ativar um registrar ou sufixo. O bundle assinado
libera o gate do roadmap SN-8 e oferece aos auditores uma referencia unica ao
revisar lancamentos do marketplace.
