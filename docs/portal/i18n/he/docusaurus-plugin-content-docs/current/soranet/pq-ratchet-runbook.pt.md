---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: pq-ratchet-runbook
כותרת: Simulacro PQ Ratchet do SoraNet
sidebar_label: Runbook de PQ Ratchet
תיאור: Passos de ensaio on-call para promotor ou rebaixar a politica de anonimato PQ em estagios com validacao deterministica de telemetria.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/soranet/pq_ratchet_runbook.md`. Mantenha ambas as copias sincronizadas.
:::

## הצעה

Este runbook guia a sequencia do simulacro para a politica de anonimato post-quantum (PQ) em estagios do SoraNet. Operadores ensaiam promocao (שלב A -> שלב ב' -> שלב ג') e a despromocao controlada de volta a Stage B/A quando a oferta de PQ cai. O simulacro valida hooks de telemetria (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) e coleta artefatos para o log de rehearsal de incidentes.

## דרישות מוקדמות

- Ultimo binario `sorafs_orchestrator` com שקלול יכולת (התחייב igual או אחורי אאו הפניה לעשות מקדחה רוב עם `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Acesso ao stack Prometheus/Grafana que serve `dashboards/grafana/soranet_pq_ratchet.json`.
- ספריית Snapshot נומינלית do guard. Busque e valide uma copia antes do simulacro:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

ראה את ספריית המקור הפומבית ל-JSON, מקודד מחדש עבור Norito בינארי com `soranet-directory build` לפני העזרה של ה-rotacao.

- לכידת מטא-נתונים ו-pre-stage artefatos de rotacao עבור מנפיק com o CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Janela de mudanca aprovada pelos times on call de networking e צפייה.

## Passos de promocao

1. **ביקורת שלב**

   הרשמה לשלב ראשוני:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Espere `anon-guard-pq` antes da promocao.

2. **Promova para Stage B (PQ רוב)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Aguarde >=5 minutos para manifests serem atualizados.
   - No Grafana (לוח המחוונים `SoraNet PQ Ratchet Drill`) לאשר את "אירועי המדיניות" עבור `outcome=met` עבור `stage=anon-majority-pq`.
   - צלם צילום מסך או JSON עשה כאב ותוספת או יומן אירועים.

3. **Promova para Stage C (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - בדוק את ההיסטוגרמות של `sorafs_orchestrator_pq_ratio_*` בגרסה 1.0.
   - אשר que o contador de brownout permanece plano; caso contrario, siga os passos de despromocao.

## תרגיל Despromocao / Brownout

1. **Induza uma escassez sintetica de PQ**

   ממסרים עזים PQ אין מגרש משחקים סביבה או ספריית שומרים אנטרדיס קלאסיים אפנים, קבצים חוזרים או מטמון לתזמר:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **התבוננו ב-telmetria de brownout**

   - לוח מחוונים: o Painel "Brownout Rate" sobe acima de 0.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` deve reportar `anonymity_outcome="brownout"` com `anonymity_reason="missing_majority_pq"`.

3. **Despromova para Stage B / Stage A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   ראה הצעה של PQ ainda for insficiente, despromova para `anon-guard-pq`. O simulacro termina quando os contadores de brownout estabilizam e as promocoes podem ser reaplicadas.

4. **מדריך המסעדה או השומרים**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetria e artefatos- **לוח מחוונים:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertas Prometheus:** garanta que o alerta de brownout de `sorafs_orchestrator_policy_events_total` fique abaixo do SLO configurado (<5% em qualquer janela de 10 minutes).
- **יומן תקריות:** אנקס טרכוס טלמטריה e notas do operator em `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Captura assinada:** השתמשו ב-`cargo xtask soranet-rollout-capture` להעתקה או ביומן מקדחה או בלוח תוצאות עבור `artifacts/soranet_pq_rollout/<timestamp>/`, תקצירים חישוביים של BLAKE3 ותוצרת על `rollout_capture.json` assinado.

דוגמה:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Anexe os metadata gerados e a assinatura ao pacote de governance.

## חזרה לאחור

ראה או סימולציה חושפנית escassez Real de PQ, להתמיד בשלב א', הודעה o Networking TL e anex as metricas coletadas Junto com os diffs do guard directory ao tracker incident. השתמש או ייצוא לעשות ספריית שומר capturado anteriormente למסעדה או שירות רגיל.

:::tip Cobertura de regressao
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` fornece a validacao sintetica que sustenta este simulacro.
:::