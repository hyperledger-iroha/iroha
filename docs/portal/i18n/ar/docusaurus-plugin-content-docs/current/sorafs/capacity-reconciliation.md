---
id: capacity-reconciliation
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/capacity-reconciliation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

يفرض بند خارطة الطريق **SF-2c** أن يثبت فريق الخزينة أن دفتر رسوم السعة يطابق تحويلات XOR المنفذة كل ليلة. استخدم المساعد `scripts/telemetry/capacity_reconcile.py` لمقارنة لقطة `/v2/sorafs/capacity/state` مع دفعة التحويلات المنفذة وإصدار مقاييس نصية لـ Prometheus من أجل Alertmanager.

## المتطلبات المسبقة
- لقطة حالة السعة (مدخلات `fee_ledger`) مُصدّرة من Torii.
- تصدير دفتر الأستاذ لنفس النافذة (JSON أو NDJSON مع `provider_id_hex`,
  `kind` = settlement/penalty, و`amount_nano`).
- مسار textfile collector الخاص بـ node_exporter إذا كنت تريد التنبيهات.

## دليل التشغيل
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- رموز الخروج: `0` عند التطابق، `1` عند غياب settlements/penalties أو الدفع الزائد، `2` للمدخلات غير الصالحة.
- أرفق ملخص JSON + الـ hashes بحزمة الخزينة في
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- عندما يصل ملف `.prom` إلى textfile collector، يُطلق تنبيه
  `SoraFSCapacityReconciliationMismatch` (راجع
  `dashboards/alerts/sorafs_capacity_rules.yml`) عند اكتشاف تحويلات ناقصة أو مدفوعة زائدًا أو غير متوقعة.

## المخرجات
- حالات لكل مزوّد مع فروقات settlements وpenalties.
- الإجماليات المصدّرة كمقاييس gauges:
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## النطاقات المتوقعة والتفاوتات
- التسوية دقيقة: يجب أن تتطابق nanos المتوقعة مع الفعلية لـ settlement/penalty دون أي تسامح. أي فرق غير صفري يجب أن يطلق تنبيه المشغلين.
- يثبّت CI digest لنقع 30 يومًا لدفتر رسوم السعة (اختبار `capacity_fee_ledger_30_day_soak_deterministic`) على `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`. حدّث الـ digest فقط عند تغيّر تسعير أو دلالات التبريد.
- في ملف النقع (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) تبقى penalties عند الصفر؛ في الإنتاج يجب إصدار penalties فقط عند خرق حدود الاستغلال/التوفر/PoR واحترام التبريد المُضبط قبل عمليات slashing متتالية.
