---
lang: he
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ספרי הפעלה ותרגילים לחזרה

## הצעה

O פריט לעשות מפת הדרכים **DOCS-9** ספרי הפעלה אקסיג'י אקיונאווייס mais um plano de ensaio para que
מפעילי הפורטל consigam recuperar falhas de envio sem adivinhacao. Esta not cobre tres
incidentes de alto sinal - פורס falhos, degradacao de replicacao e quedas de analytics - e
documenta os drills trimestrais que provam que o rollback de alias e a validacao sintetica
continuam funcionando מקצה לקצה.

### רלאציונדו חומרי

- [`devportal/deploy-guide`](./deploy-guide) - זרימת עבודה של אריזה, חתימה וקידום כינוי.
- [`devportal/observability`](./observability) - תגי שחרור, ניתוחים ובדיקות עזר.
- `docs/source/sorafs_node_client_protocol.md`
  ה [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - Telemetria do registro e limites de escalonamento.
- `docs/portal/scripts/sorafs-pin-release.sh` e helpers `npm run probe:*`
  רשימת תיוג של רשימת תיוג.

### השוואת כלי טלמטריה

| סינאל / כלי | פרופוזיטו |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (נפגש/הוחמצה/בהמתנה) | Detecta bloqueios de replicacao e violacoes de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Quantifica profundidade do backlog e latencia de completacao para triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Mostra falhas do gateway que frequentemente seguem um פריסה רחבה. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Probes sinteticos que gateiam משחרר e validam rollbacks. |
| `npm run check:links` | שער דה קישורים quebrados; usado apos cada mitigacao. |
| `sorafs_cli manifest submit ... --alias-*` (USado por `scripts/sorafs-pin-release.sh`) | Mecanismo de promocao/reversao de alias. |
| לוח `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Agrega telemetria de refusals/כינוי/TLS/replicacao. התראות דומות ל-PagerDuty התייחסות לכאבים כמו ראיות. |

## Runbook - Deploy falho ou artefato ruim

### Condicoes de disparo

- Probes de preview/producao falham (`npm run probe:portal -- --expect-release=...`).
- Alertas Grafana em `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` אפוס um השקה.
- QA manual not rotas quebradas ou falhas do proxy נסה זאת במיידי
  a promocao do alias.

### Contencao imediata

1. **Congelar פורס:** marcar o pipeline CI com `DEPLOY_FREEZE=1` (קלט לעשות זרימת עבודה
   GitHub) ou pausar o job Jenkins para que nenhum artefato seja enviado.
2. **Capturar artefatos:** baixar `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, e a saida dos probes do build com falha para que
   o rollback referencie os digests exatos.
3. **מחזיקי עניין מודיעים:** SRE אחסון, Docs/DevRel מובילים, או קצין תורן de
   ניהול מודעות (especialmente quando `docs.sora` esta impactado).

### תהליך החזרה לאחור

1. זיהוי של טוב ידוע אחרון (LKG). O זרימת עבודה דה producao os armazena em
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Re-vincule o alias a esse manifest com o helper de shipping:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. הירשם או קורות חיים לעשות החזרה לאחור ללא כרטיס לעשות incidente junto com os digests
   manifest LKG e do manifest com falha.

### Validacao1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` ו-`sorafs_cli proof verify ...`
   (veja o guia deploy) para confirmar que o manifest repromovido continua
   batendo com o CAR arquivado.
4. `npm run probe:tryit-proxy` para garantir que o proxy Try-It staging voltou.

### לאחר התקרית

1. פריסת צינורות או צינורות עיקריים.
2. Preencha as entradas "Lessons learned" em [`devportal/deploy-guide`](./deploy-guide)
   com novos pontos, se houver.
3. פגמי אברה para a suite de testes falhada (בדיקה, בודק קישורים וכו').

## Runbook - Degradacao de replicacao

### Condicoes de disparo

- התראה: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95' ל-10 דקות.
- `torii_sorafs_replication_backlog_total > 10` ל-10 דקות (veja
  `pin-registry-ops.md`).
- Governanca reporta alias lento apos um release.

### טריאז'

1. Inspecione לוחות מחוונים של [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) para
   לאשר את הצטברות ההצטרפות esta localizado em uma classe de storage ou em um flet de providers.
2. יומני Cruz do Torii por אזהרות `sorafs_registry::submit_manifest` para determinar se
   כמו הגשות estao falhando.
3. העתקים של Amostre a saude das via `sorafs_cli manifest status --manifest ...` (ליסטה
   resultados por ספק).

### מיטיגקאו

1. Reemita o manifest com maior contagem de replicas (`--pin-min-replicas 7`) usando
   `scripts/sorafs-pin-release.sh` para que o מתזמן distribua carga em um set maior
   דה ספקים. הרשם או נובו לעכל ללא יומן לעשות אירועים.
2. ראה צבר הצטברות אסטרטגיית ספק יחיד, ביטול זמני דרך o
   מתזמן דה replicacao (מסמכים עם `pin-registry-ops.md`) e envie um novo
   מניפסט forcando os outros ספקי אטואליזר או כינוי.
3. Quando a frescura do alias for mais critica que a paridade de replicacao, re-vincule o
   כינוי a um manifest quente ja em staging (`docs-preview`), depois publique um manifest de
   acompanhamento quando o SRE limpar o backlog.

### Recuperacao e fechamento

1. Monitore `torii_sorafs_replication_sla_total{outcome="missed"}` para garantir que o
   contador לייצב.
2. לכידת a saida `sorafs_cli manifest status` como evidencia de que cada replica voltou
   קונפורמידדה.
3. Abra ou tualize o נתיחה שלאחר המוות לעשות צבר דיפליקאו com proximos passos
   (קנה מידה של ספקים, כוונון לעשות צ'אנקר וכו').

## Runbook - Queda de analytics או telemetria

### Condicoes de disparo

- `npm run probe:portal` passa, לוחות מחוונים של mas param de ingerir eventos do
  `AnalyticsTracker` ל->15 דקות.
- סקירת פרטיות aponta um aumento inesperado de eventos descartados.
- `npm run probe:tryit-proxy` falha em paths `/probe/analytics`.

### תגובה1. אימות כניסות לבנות: `DOCS_ANALYTICS_ENDPOINT` ה
   `DOCS_ANALYTICS_SAMPLE_RATE` ללא שחרור של artefato (`build/release.json`).
2. בצע מחדש את `npm run probe:portal` com `DOCS_ANALYTICS_ENDPOINT` apontando para o
   אספן דה סטייג'ינג עבור אישורים לגבי עוקבים של אינדה מטילים.
3. Se os collectors estiverem למטה, defina `DOCS_ANALYTICS_ENDPOINT=""` ובנייה מחדש
   para que o tracker faca קצר חשמלי; רשם א תקלה של ג'נלה דה לינה
   לעשות tempo do incidente.
4. Valide que `scripts/check-links.mjs` ainda faz טביעת אצבע de `checksums.sha256`
   (quedas de analytics *nao* devem bloquear a validacao do sitemap).
5. Quando o collector voltar, road `npm run test:widgets` עבור בדיקות יחידת מערכת הפעלה
   עשה helper de analytics לפני פרסום מחדש.

### לאחר התקרית

1. Atualize [`devportal/observability`](./observability) com novas limitacoes do collector
   או דרישות האהבה.
2. Abra um aviso de governanca se dados de analytics foram perdidos ou redigidos fora
   דה פוליטיקה.

## מקדחות trimestrais de resiliencia

בצע תרגילי OS Dois durante a **primeira terca-feira de cada trimestre** (ינואר/אבר/יולי/אאוט)
ou imediatamente apos qualquer mudanca maior de infraestrutura. Armazene artefatos em
`artifacts/devportal/drills/<YYYYMMDD>/`.

| מקדחה | פאסוס | Evidencia |
| ----- | ----- | -------- |
| Ensaio de rollback de alias | 1. חזור על החזרה לאחור של "Deploy falho" בשימוש או מניפסט של producao mais recente.<br/>2. Re-vincular a producao assim que os probes passarem.<br/>3. הרשם `portal.manifest.submit.summary.json` e logs de probes na פסטה לעשות תרגיל. | `rollback.submit.json`, אמרו בדיקות, תג שחרור לעשות זאת. |
| Auditoria de validacao sintetica | 1. Rodar `npm run probe:portal` e `npm run probe:tryit-proxy` contra producao e staging.<br/>2. Rodar `npm run check:links` e arquivar `build/link-report.json`.<br/>3. מאגר צילומי מסך/ייצוא של כאבים Grafana אישור או בדיקות מוצלחות. | יומני בדיקה + `link-report.json` התייחסות או טביעת אצבע. |

אסקאלון מתרגם למנהלי דוקס/DevRel ו-Revisao de governanca de SRE,
pois o מפת דרכים exige evidencia trimestral determinista de que o rollback de alias e os
בדיקות לעשות פורטל continuam saudaveis.

## Coordenacao PagerDuty e כוננות- O servico PagerDuty **Docs Portal Publishing** e dono dos alertas gerados a partir de
  `dashboards/grafana/docs_portal.json`. כרגיל `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, ו-`DocsPortal/TLSExpiry` עמודים או ראשי של Docs/DevRel
  com Storage SRE como secundario.
- Quando a page tocar, כולל `DOCS_RELEASE_TAG`, צילומי מסך מצורפים dos paineis Grafana
  afetados e linke a saida de probe/link-check nas notas do incidente antes de iniciar
  א מיטיגקאו.
- Depois da mitigacao (החזרה או פריסה מחדש), הפעל מחדש את `npm run probe:portal`,
  `npm run check:links`, e לכידת צילומי מצב Grafana עדכניות כמו מדדים
  ספי de volta aos. תוספת עדות לאירועים של PagerDuty
  antes de resolver.
- Se dois alertas dispararem ao mesmo tempo (for exemplo expiracao de TLS mais backlog),
  סירובי טריאז' פרימיירו (לפי פרסום), ביצוע o procedimento de rollback e
  depois resolva TLS/backlog com Storage SRE בפואנטה.