---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: pq-ratchet-runbook
כותרת: Simulation PQ Ratchet SoraNet
sidebar_label: Runbook PQ Ratchet
תיאור: חזרות ב-call לקידום או retrograder la politique d'anonymat PQ et valider la telemetrie deterministe.
---

:::הערה מקור קנוניק
Cette page reflete `docs/source/soranet/pq_ratchet_runbook.md`. Gardez les deux copies alignees jusqu'a la retraite de l'ancien set de documentation.
:::

## Objectif

זה מדריך הפעלה לרצף של תרגיל כיבוי אש לפוליטיקה של פוסט-קוונטי (PQ) etagee de SoraNet. המבצעים שחוזרים על עצמם לקידום (שלב א' -> שלב ב' -> שלב ג') הם רק שלב ב' ו-ב'. Le drill valide les hooks de telemetrie (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) ואספו את חפצי האמנות עבור החזרה לאירוע.

## תנאי מוקדם

- Dernier binaire `sorafs_orchestrator` עם שקלול יכולת (התחייבות שוויונית או אחריה א לה התייחסות למקדחה ב-`docs/source/soranet/reports/pq_ratchet_validation.md`).
- Acces au stack Prometheus/Grafana qui sert `dashboards/grafana/soranet_pq_ratchet.json`.
- ספריית Snapshot נומינלית du guard. Recuperez et verifiez une copie avant le drill:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

ספריית המקור אינה מפרסמת que du JSON, re-encodez-le en binaire Norito avec `soranet-directory build` avant d'executer les helpers de rotation.

- Capturez les metadata et pre-stagez les artefacts de rotation de l'suer with le CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- אישור השינויים מצייד רשתות בשיחות וצפייה.

## אטיפס דה קידום

1. **ביקורת שלב**

   הרשמה לשלב היציאה:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   קידום אוונט של Attendez `anon-guard-pq`.

2. **קידום לעומת שלב ב' (רוב PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Attendez >=5 דקות pour que les manifests se rafraichissent.
   - Dans Grafana (לוח המחוונים `SoraNet PQ Ratchet Drill`) confirmez que le panneau "אירועי מדיניות" affiche `outcome=met` pour `stage=anon-majority-pq`.
   - Capturez un צילום מסך או JSON du panau et attachez-le au log d'incident.

3. **קידום לעומת שלב ג' (PQ קפדני)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Verifiez que les histogrammes `sorafs_orchestrator_pq_ratio_*` tendent vers 1.0.
   - Confirmez que le compteur brownout reste plat; sinon suivez les etapes de retrogradation.

## מקדחה לאחור / חום

1. **Induire une penurie PQ synthetique**

   Desactivez les relays PQ dans l'environnement playground en taillant le guard directory aux seules entrees classiques, puis rechargez le cache orchestrator:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **מתבונן ב-la telemetrie brownout**

   - לוח מחוונים: le panneau "Brownout Rate" monte au-dessus de 0.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` doit reporter `anonymity_outcome="brownout"` avec `anonymity_reason="missing_majority_pq"`.

3. **רטרוגרדר לעומת שלב ב' / שלב א'**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Si l'offre PQ reste insuffisante, retrogradez vers `anon-guard-pq`. Le drill se termine quand les compteurs brownout se stabilisent and que les promotions peuvent etre reappliquees.

4. **מדריך המסעדה לשומר**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetrie et artefacts- **לוח מחוונים:** `dashboards/grafana/soranet_pq_ratchet.json`
- **התראות Prometheus:** אבטחה לגבי התראה ל-`sorafs_orchestrator_policy_events_total` reste sous le SLO configure (<5% sur toute fenetre de de 10 דקות).
- **יומן תקריות:** ajoutez les snippets de telemetrie et les notes operationur a `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **חתם לכידת:** השתמש ב-`cargo xtask soranet-rollout-capture` לשפוך מכונת צילום ל-Drill log et le scoreboard in `artifacts/soranet_pq_rollout/<timestamp>/`, calculer les digests BLAKE3 et produire un `rollout_capture.json` sign.

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

Joignez les metadata generees et la signature au dossier governance.

## חזרה לאחור

אם אתה תרגיל להפגין את PQ אמיתי, נשאר על שלב א', הודע ל-Networking TL ואספי המדדים אספו את כל ההבדלים במדריך השומר או המעקב אחר האירועים. לנצל את היצוא של שומר ספרייה לכידת פלוס כדי לשפוך שירות רגיל.

:::tip Couverture de regression
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` fournit la validation synthetic qui sotient ce drill.
:::