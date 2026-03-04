---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: pq-ratchet-runbook
título: تمرين PQ Ratchet في SoraNet
sidebar_label: trinquete PQ interno
descripción: خطوات تمرين المناوبة لترقية او خفض سياسة اخفاء الهوية PQ المرحلية مع تحقق telemetría حتمي.
---

:::nota المصدر القياسي
Utilice el enlace `docs/source/soranet/pq_ratchet_runbook.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد الوثائق القديمة.
:::

## الغرض

هذا الدليل يوجه تسلسل تمرين الطوارئ لسياسة اخفاء الهوية post-quantum (PQ) المرحلية في SoraNet. يتدرب المشغلون على الترقية (Etapa A -> Etapa B -> Etapa C) y خفض منضبط الى Etapa B/A عندما تنخفض امدادات PQ. يتحقق التمرين من telemetría ganchos (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) y artefactos لسجل تدريب الحوادث.

## المتطلبات

- Utiliza el binario `sorafs_orchestrator` con ponderación de capacidad (commit عند او بعد مرجع التمرين المعروض في `docs/source/soranet/reports/pq_ratchet_validation.md`).
- الى حزمة Prometheus/Grafana التي تخدم `dashboards/grafana/soranet_pq_ratchet.json`.
- instantánea del directorio de guardia. اجلب وتحقق من نسخة قبل التمرين:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Aquí hay un directorio fuente, un archivo JSON y un archivo binario Norito y un archivo binario `soranet-directory build` para crear ayudantes.

- Metadatos y artefactos del emisor de la CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- نافذة تغيير معتمدة من فرق de guardia الخاصة بالشبكات والمراقبة.

## خطوات الترقية

1. **تدقيق المرحلة**

   سجل المرحلة الابتدائية:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Haga clic en `anon-guard-pq`.

2. **الترقية الى Etapa B (PQ mayoritaria)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```- انتظر >=5 دقائق حتى تتجدد manifiestos.
   - En Grafana (en `SoraNet PQ Ratchet Drill`) está en "Eventos de política" en `outcome=met` en `stage=anon-majority-pq`.
   - Archivos de archivos JSON y archivos JSON.

3. **الترقية الى Etapa C (PQ estricto)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Aplicación `sorafs_orchestrator_pq_ratio_*` versión 1.0.
   - تاكد ان عداد apagón يبقى ثابتا؛ خلاف ذلك اتبع خطوات الخفض.

## تمرين الخفض / apagón

1. **احداث نقص PQ اصطناعي**

   عطل retransmisiones PQ في بيئة patio de juegos بتقليم directorio de guardia الى entradas كلاسيكية فقط، ثم اعد تحميل caché الخاص بالـ orquestador:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **مراقبة telemetría الخاصة بـ apagón**

   - Panel de control: لوحة "Tasa de apagones" ترتفع فوق 0.
   -PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - Está entre `sorafs_fetch` y `anonymity_outcome="brownout"` y `anonymity_reason="missing_majority_pq"`.

3. **الخفض الى Etapa B / Etapa A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   La configuración PQ es la siguiente: `anon-guard-pq`. ينتهي التمرين عند استقرار عدادات apagones y اعادة الترقية.

4. **directorio de guardia**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetría y artefactos- **Panel de control:** `dashboards/grafana/soranet_pq_ratchet.json`
- **تنبيهات Prometheus:** تاكد من ان تنبيه brownout لـ `sorafs_orchestrator_policy_events_total` يبقى دون SLO المعتمد (&lt;5% ضمن اي نافذة 10 دقائق).
- **Registro de incidentes:** ارفق مقتطفات telemetry وملاحظات المشغل الى `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **التقاط موقع:** استخدم `cargo xtask soranet-rollout-capture` لنسخ registro de perforación ولوحة النتائج الى `artifacts/soranet_pq_rollout/<timestamp>/`, y BLAKE3 digests, y انتاج `rollout_capture.json` Aquí está.

Nombre:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

ارفق البيانات المولدة والتوقيع بحزمة gobernanza.

## Revertir

اذا كشف التمرين عن نقص PQ حقيقي، ابق على Etapa A, اخطر Networking TL, وارفق المقاييس المجمعة مع فروقات directorio de guardia الى متتبع الحوادث. استخدم تصدير directorio de guardia الذي تم التقاطه سابقا لاستعادة الخدمة الطبيعية.

:::consejo تغطية الانحدار
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` يوفر التحقق الاصطناعي الذي يدعم هذا التمرين.
:::