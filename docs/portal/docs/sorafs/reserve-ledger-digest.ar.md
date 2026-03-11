---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 72f24d0456a52deb36ed5801fbd0c2c0a97c0daee4419be8d4a49546d7cf758c
source_last_modified: "2025-11-20T08:27:52.686493+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: ملخص دفتر أستاذ الاحتياطي ولوحات المراقبة
description: كيفية تحويل مخرجات `sorafs reserve ledger` إلى تليمترية ولوحات وتنبيهات لسياسة Reserve+Rent.
---

سياسة Reserve+Rent (بند خارطة الطريق **SFM-6**) توفر الآن مساعدات CLI `sorafs reserve` إضافة إلى
المترجم `scripts/telemetry/reserve_ledger_digest.py` لكي تتمكن تشغيلات الخزينة من إصدار تحويلات
إيجار/احتياطي حتمية. تعكس هذه الصفحة سير العمل المحدد في
`docs/source/sorafs_reserve_rent_plan.md` وتشرح كيفية توصيل موجز التحويلات الجديد إلى
Grafana + Alertmanager كي يتمكن مراجعو الاقتصاد والحوكمة من تدقيق كل دورة فواتير.

## سير العمل من البداية للنهاية

1. **عرض السعر + إسقاط الدفتر**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

   sorafs reserve ledger \
     --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
     --provider-account i105... \
     --treasury-account i105... \
     --reserve-account i105... \
     --asset-definition xor#sora \
     --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   يقوم مساعد الدفتر بإرفاق كتلة `ledger_projection` (الإيجار المستحق، عجز الاحتياطي،
   فرق top-up، قيم underwriting المنطقية) إضافة إلى تعليمات Norito من نوع `Transfer`
   اللازمة لنقل XOR بين حسابي الخزينة والاحتياطي.

2. **إنشاء الملخص + مخرجات Prometheus/NDJSON**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   يقوم مساعد الملخص بتطبيع إجماليات micro-XOR إلى XOR، ويسجل ما إذا كان الإسقاط يستوفي
   underwriting، ويُصدر مقاييس موجز التحويلات `sorafs_reserve_ledger_transfer_xor`
   و`sorafs_reserve_ledger_instruction_total`. عندما يلزم معالجة عدة دفاتر (مثلا دفعة
   من المزودين)، كرر أزواج `--ledger`/`--label` وسيكتب المساعد ملف NDJSON/Prometheus
   واحدا يحتوي كل الملخصات لكي تلتقط لوحات المراقبة الدورة كاملة دون glue مخصص.
   يستهدف ملف `--out-prom` جامع textfile لـ node-exporter - ضع ملف `.prom` في الدليل
   الذي يراقبه الـ exporter أو ارفعه إلى دلو التليمترية الذي يستهلكه job لوحة Reserve -
   بينما يقوم `--ndjson-out` بتغذية نفس الـ payloads في خطوط البيانات.

3. **نشر artefacts + الأدلة**
   - خزّن الملخصات تحت `artifacts/sorafs_reserve/ledger/<provider>/` واربط ملخص Markdown
     من تقرير الاقتصاد الأسبوعي.
   - أرفق ملخص JSON مع burn-down الإيجار (كي يتمكن المدققون من إعادة حساب المعادلة) وضع
     checksum ضمن حزمة أدلة الحوكمة.
   - إذا أشار الملخص إلى top-up أو خرق underwriting، فاذكر معرفات التنبيه
     (`SoraFSReserveLedgerTopUpRequired`, `SoraFSReserveLedgerUnderwritingBreach`) وسجّل
     أي ISIs تحويل تم تطبيقها.

## المقاييس → لوحات المراقبة → التنبيهات

| المقياس المصدر | لوحة Grafana | التنبيه / ربط السياسة | الملاحظات |
|---------------|-------------|------------------------|-----------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | “DA Rent Distribution (XOR/hour)” في `dashboards/grafana/sorafs_capacity_health.json` | غذِّ ملخص الخزينة الأسبوعي؛ قمم تدفق الاحتياطي تنعكس إلى `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`). |
| `torii_da_rent_gib_months_total` | “Capacity Usage (GiB-months)” (نفس اللوحة) | اقرنها بملخص الدفتر لإثبات أن التخزين المفوتر يطابق تحويلات XOR. |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | “Reserve Snapshot (XOR)” + بطاقات الحالة في `dashboards/grafana/sorafs_reserve_economics.json` | التنبيه `SoraFSReserveLedgerTopUpRequired` يعمل عندما `requires_top_up=1`؛ والتنبيه `SoraFSReserveLedgerUnderwritingBreach` يعمل عندما `meets_underwriting=0`. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | “Transfers by Kind”, “Latest Transfer Breakdown” وبطاقات التغطية في `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerInstructionMissing` و`SoraFSReserveLedgerRentTransferMissing` و`SoraFSReserveLedgerTopUpTransferMissing` تحذر عندما يكون موجز التحويلات مفقودا أو صفريا رغم الحاجة إلى rent/top-up؛ بطاقات التغطية تهبط إلى 0% في الحالات نفسها. |

عند اكتمال دورة الإيجار، حدّث لقطات Prometheus/NDJSON، وتأكد من أن لوحات Grafana
تلتقط `label` الجديد، وأرفق لقطات الشاشة + معرفات Alertmanager مع حزمة حوكمة الإيجار.
هذا يثبت أن إسقاط CLI والتليمترية وartefacts الحوكمة جميعها تنطلق من **نفس** موجز
التحويلات ويحافظ على محاذاة لوحات اقتصاد خارطة الطريق مع أتمتة Reserve+Rent. يجب أن
تُظهر بطاقات التغطية 100% (أو 1.0) وأن تنطفئ التنبيهات الجديدة بمجرد وجود تحويلات
الإيجار وtop-up الاحتياطي داخل الملخص.
