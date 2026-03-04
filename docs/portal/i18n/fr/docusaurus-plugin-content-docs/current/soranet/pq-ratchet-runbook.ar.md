---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-ratchet-runbook
titre : par PQ Ratchet sur SoraNet
sidebar_label : par PQ Ratchet
description: خطوات تمرين المناوبة لترقية او خفض سياسة اخفاء الهوية PQ المرحلية مع تحقق télémétrie حتمي.
---

:::note المصدر القياسي
Il s'agit de la référence `docs/source/soranet/pq_ratchet_runbook.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد الوثائق القديمة.
:::

## الغرض

Il s'agit d'une approche post-quantique (PQ) de SoraNet. يتدرب المشغلون على الترقية (Stage A -> Stage B -> Stage C) et وعلى خفض منضبط الى Stage B/A عندما تنخفض امدادات PQ. Il s'agit de crochets de télémétrie (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) et d'artefacts pour la navigation.

## المتطلبات

- Utiliser le binaire `sorafs_orchestrator` pour la pondération des capacités (commit est également utilisé pour `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Utilisez Prometheus/Grafana pour `dashboards/grafana/soranet_pq_ratchet.json`.
- instantané dans le répertoire de garde. اجلب وتحقق من نسخة قبل التمرين:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Il s'agit du répertoire source JSON qui contient le binaire Norito et `soranet-directory build` pour les helpers.

- Les métadonnées et les artefacts de l'émetteur de la CLI :

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- نافذة تغيير معتمدة من فرق on call الخاصة بالشبكات والمراقبة.

## خطوات الترقية

1. **تدقيق المرحلة**

   سجل المرحلة الابتدائية:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Utilisez `anon-guard-pq` pour la connexion.

2. **الترقية الى Étape B (PQ majoritaire)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - انتظر >=5 دقائق حتى تتجدد manifestes.
   - Pour Grafana (pour `SoraNet PQ Ratchet Drill`) et pour "Événements de stratégie" comme `outcome=met` pour `stage=anon-majority-pq`.
   - La compatibilité avec JSON et la compatibilité avec la fonction JSON.

3. **الترقية الى Stade C (PQ strict)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Utilisez le `sorafs_orchestrator_pq_ratio_*` pour la version 1.0.
   - تاكد ان عداد brownout يبقى ثابتا؛ خلاف ذلك اتبع خطوات الخفض.

## تمرين الخفض / baisse de tension

1. **احداث نقص PQ اصطناعي**

   Pour les relais PQ et le terrain de jeu, pour le répertoire de garde, les entrées et les entrées, vous pouvez également utiliser le cache pour l'orchestrateur :

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Utilisation de la télémétrie en cas de baisse de tension**

   - Tableau de bord : "Taux de baisse de tension" à 0.
   - PromQL : `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - Il s'agit de `sorafs_fetch` ou `anonymity_outcome="brownout"` ou `anonymity_reason="missing_majority_pq"`.

3. **الخفض الى Étape B / Étape A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   La clé PQ est la suivante : `anon-guard-pq`. Il y a une baisse de tension et une baisse de tension.

4. **Répertoire de garde**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Télémétrie et artefacts

- **Tableau de bord :** `dashboards/grafana/soranet_pq_ratchet.json`
- **تنبيهات Prometheus :** تاكد من ان تنبيه baisse de tension pour `sorafs_orchestrator_policy_events_total` يبقى دون SLO المعتمد (<5% ضمن اي نافذة 10 دقائق).
- **Journal des incidents :** ارفق مقتطفات télémétrie et وملاحظات المشغل الى `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **التقاط موقع:** استخدم `cargo xtask soranet-rollout-capture` pour le journal de forage et les résumés BLAKE3, وانتاج `rollout_capture.json` ici.

مثال:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

ارفق البيانات المولدة والتوقيع بحزمة gouvernance.

## RestaurationVous avez besoin de PQ pour Stage A, de Networking TL et de Guard Directory. متتبع الحوادث. استخدم تصدير guard directory الذي تم التقاطه سابقا لاستعادة الخدمة الطبيعية.

:::tip تغطية الانحدار
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` يوفر التحقق الاصطناعي الذي يدعم هذا التمرين.
:::