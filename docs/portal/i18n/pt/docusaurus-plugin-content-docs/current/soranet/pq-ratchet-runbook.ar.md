---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-ratchet-runbook
título: O PQ Ratchet da SoraNet
sidebar_label: Nome PQ Ratchet
description: خطوات تمرين المناوبة لترقية او خفض سياسة اخفاء الهوية PQ المرحلية مع تحقق telemetria حتمي.
---

:::note المصدر القياسي
Verifique o valor `docs/source/soranet/pq_ratchet_runbook.md`. Não se preocupe, o problema pode ser alterado.
:::

## الغرض

Você pode usar o software pós-quântico (PQ) no SoraNet. يتدرب المشغلون على الترقية (Estágio A -> Estágio B -> Estágio C) وعلى خفض منضبط Estágio B/A عندما تنخفض امدادات PQ. Use ganchos de telemetria (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) e artefatos que você pode usar.

## المتطلبات

- O binário para `sorafs_orchestrator` é baseado na ponderação de capacidade (commit e o commit no `docs/source/soranet/reports/pq_ratchet_validation.md`).
- O código Prometheus/Grafana é o mesmo que `dashboards/grafana/soranet_pq_ratchet.json`.
- snapshot do diretório guard. اجلب وتحقق من نسخة قبل التمرين:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Esse diretório de origem é JSON, mas o binário Norito é `soranet-directory build` para ajudar os helpers.

- Metadados de metadados وجهز مسبقا artefatos تدوير emissor عبر CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- نافذة تغيير معتمدة من فرق on-call الخاصة بالشبكات والمراقبة.

## خطوات الترقية

1. **تدقيق المرحلة**

   سجل المرحلة الابتدائية:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   O `anon-guard-pq` é um problema.

2. **الترقية الى Estágio B (Maioria PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - انتظر >=5 دقائق حتى تتجدد manifests.
   - Em Grafana (para `SoraNet PQ Ratchet Drill`) está no local "Eventos de Política" que está em `outcome=met` para `stage=anon-majority-pq`.
   - O valor do arquivo JSON e o JSON são válidos.

3. **الترقية الى Estágio C (PQ estrito)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - تحقق ان مخططات `sorafs_orchestrator_pq_ratio_*` تتجه الى 1.0.
   - تاكد ان عداد brownout يبقى ثابتا؛ Não use nenhum produto.

## تمرين الخفض / brownout

1. **Como usar PQ**

   عطل relés PQ no playground بتقليم guarda diretório الى entradas كلاسيكية فقط, ثم اعد تحميل cache الخاص بالـ orquestrador:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **مراقبة telemetria الخاصة بـ brownout**

   - Dashboard: A opção "Brownout Rate" é igual a 0.
   -PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - Não use `sorafs_fetch` ou `anonymity_outcome="brownout"` com `anonymity_reason="missing_majority_pq"`.

3. **الخفض الى Estágio B / Estágio A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Você pode usar PQ para obter informações sobre `anon-guard-pq`. Não há queda de energia e queda de energia.

4. ** Diretório de proteção **

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetria e artefatos

- **Painel:** `dashboards/grafana/soranet_pq_ratchet.json`
- **تنبيهات Prometheus:** تاكد من ان تنبيه brownout لـ `sorafs_orchestrator_policy_events_total` يبقى دون SLO المعتمد (&lt;5% ضمن اي 10 dias).
- **Registro de incidentes:** ارفق مقتطفات telemetria وملاحظات المشغل الى `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **التقاط موقع:** استخدم `cargo xtask soranet-rollout-capture` para o log de perfuração e o BLAKE3 digests, وانتاج `rollout_capture.json` Modelo.

Exemplo:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

ارفق البيانات المولدة والتوقيع بحزمة governança.

## Reversãoاذا كشف التمرين عن نقص PQ حقيقي, ابق على Stage A, اخطر Networking TL, وارفق المقاييس المجمعة مع فروقات guard directory الى متتبع الحوادث. استخدم تصدير guard directory الذي تم التقاطه سابقا لاستعادة الخدمة الطبيعية.

:::dica
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` não é compatível com o código `cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics`.
:::