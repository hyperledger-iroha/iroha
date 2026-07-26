---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: pq-ratchet-runbook
כותרת: Simulacro de PQ Ratchet de SoraNet
sidebar_label: Runbook de PQ Ratchet
תיאור: Pasos de ensayo para guardia al promotor o degradar la politica de anonimato PQ escalonada con validacion de telemetria determinista.
---

:::שימו לב פואנטה קנוניקה
Esta pagina refleja `docs/source/soranet/pq_ratchet_runbook.md`. Manten ambas copias sincronizadas.
:::

## הצעה

Este runbook guia la secuencia del simulacro para la politica de anonimato post-quantum (PQ) escalonada de SoraNet. Los operadores ensayan tanto la promocion (שלב A -> שלב B -> שלב C) como la degradacion controlada de regreso a Stage B/A cuando cae el supply PQ. El simulacro valida los hooks de telemetria (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) y recolecta artefactos para el log de rehearsal de incidentes.

## דרישות מוקדמות

- Ultimo binario `sorafs_orchestrator` עם שקלול יכולות (התחייבות ב-`docs/source/soranet/reports/pq_ratchet_validation.md`).
- גישה למחסנית Prometheus/Grafana עבור `dashboards/grafana/soranet_pq_ratchet.json`.
- מדריך השמירה של תמונת מצב נומינלית. Trae y verifica una copia antes del simulacro:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

אם ספריית המקור בודדת מפרסמת את JSON, קודדת מחדש את Norito בינאריו עם `soranet-directory build` לפני הוצאת העוזרים לסיבוב.

- Captura metadata y pre-stagea artefactos de rotacion של המנפיק עם CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Ventana de cambio aprobada por los equipos on-call de networking y observability.

## Pasos de promocion

1. **ביקורת שלב**

   הרשמה לשלב הראשון:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Espera `anon-guard-pq` לפני קידום מכירות.

2. **פרומוציונה לשלב ב' (רוב PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Espera >=5 minutos para que los manifests reresquen.
   - En Grafana (לוח המחוונים `SoraNet PQ Ratchet Drill`) מאשר את הפאנל "אירועי מדיניות" מוestre `outcome=met` עבור `stage=anon-majority-pq`.
   - צילום מסך של פאנל JSON ותוסף יומן אירועים.

3. **קידום שלב ג' (PQ קפדני)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Verifica que los histogramas `sorafs_orchestrator_pq_ratio_*` tiendan a 1.0.
   - Confirma que el contador de brownout permanezca plano; לא, סיג לוס pasos degradacion.

## Simulacro degradacion / חום

1. **לעורר una escasez sintetica de PQ**

   Deshabilita relays PQ en el entorno de playground recortando el guard directory a entradas clasicas solamente, luego recarga el cache del orchestrator:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **תצפית על טלמטריה דה בראון**

   - לוח מחוונים: הפאנל "Brownout Rate" sube por encima de 0.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` debe reportar `anonymity_outcome="brownout"` con `anonymity_reason="missing_majority_pq"`.

3. **דרגה א' שלב ב' / שלב א'**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   אם אספקת ה-PQ אינה מספיקה, הורד ל-`anon-guard-pq`. El simulacro termina cuando los contadores de brownout se estabilizan y las promociones pueden reaplicarse.

4. **מדריך מסעדות אל גארד**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## טלמטריה ואמנות- **לוח מחוונים:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertas Prometheus:** אבטחה דה que la alerta de brownout de `sorafs_orchestrator_policy_events_total` תגרום להגדרות SLO (<5% בטווח של 10 דקות).
- **יומן תקריות:** תוספת של קטעי טלמטריה והוראות מפעיל א `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Captura firmada:** ארה"ב `cargo xtask soranet-rollout-capture` עבור עותק של יומן מקדחה ולוח תוצאות ב-`artifacts/soranet_pq_rollout/<timestamp>/`, תקצירים חישוביים BLAKE3 y מפיק ו-`rollout_capture.json` firmado.

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

Adjunta los metadatos generados y la firma al paquete de governance.

## חזרה לאחור

על פי סימולציה של PQ, קבוע בשלב א', הודעה על Networking TL y adjunta las metricas recolectadas Junto con los diffs of guard directory on incident tracker. מדריך היצוא של ארה"ב, מדריך שרתים רגיל.

:::טיפ קוברטורה דה רגרסיה
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` proporciona la validacion sintetica que respalda este simulacro.
:::