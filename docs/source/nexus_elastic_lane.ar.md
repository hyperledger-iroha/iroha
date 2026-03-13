---
lang: ar
direction: rtl
source: docs/source/nexus_elastic_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0c93bb174622874e22cbc7962759a842095aec14389d601805c2a20632c86958
source_last_modified: "2025-11-21T18:07:10.137018+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_elastic_lane.md -->

# عدة توفير المسارات المرنة (NX-7)

> **عنصر خارطة الطريق:** NX-7 — ادوات توفير lane مرنة  
> **الحالة:** اكتملت الادوات — تولد manifests و catalog snippets و Norito payloads و smoke tests،
> و helper الخاص بـ load-test bundle يوحد الان gating لزمن الـ slots مع manifests الادلة كي تنشر
> تجارب حمل المدققين دون scripts مخصصة.

يوضح هذا الدليل المساعد `scripts/nexus_lane_bootstrap.sh` الذي يؤتمت توليد manifests للـ lane،
وقطع catalog للـ lane/dataspace، وادلة rollout. الهدف هو جعل انشاء lanes Nexus الجديدة
(عامة او خاصة) سهلا دون تحرير عدة ملفات يدويا او اعادة اشتقاق هندسة الكتالوج.

## 1. المتطلبات

1. موافقة الحوكمة على alias الخاص بالـ lane، و dataspace، ومجموعة المدققين، و fault tolerance (`f`)،
   وسياسة settlement.
2. قائمة نهائية بالمدققين (account IDs) وقائمة namespaces محمية.
3. صلاحية الوصول الى مستودع تهيئة العقد لاضافة الـ snippets المولدة.
4. مسارات registry الخاصة بـ manifests للـ lane (انظر `nexus.registry.manifest_directory` و
   `cache_directory`).
5. جهات اتصال telemetria/PagerDuty للـ lane لربط التنبيهات عند التشغيل.

## 2. توليد artefacts للـ lane

شغل المساعد من جذر المستودع:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

الخيارات المهمة:

- `--lane-id` يجب ان يطابق فهرس الادخال الجديد في `nexus.lane_catalog`.
- `--dataspace-alias` و `--dataspace-id/hash` يتحكمان في ادخال catalog للـ dataspace (افتراضيا
  يستخدم lane id اذا ترك فارغا).
- `--validator` يمكن تكراره او اخذه من `--validators-file`.
- `--route-instruction` / `--route-account` يولدان قواعد توجيه جاهزة للصق.
- `--metadata key=value` (او `--telemetry-contact/channel/runbook`) تلتقط جهات اتصال runbook لكي
  تعرض لوحات القياس المالكين الصحيحين.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` تضيف hook للترقية في manifest عندما تحتاج lane
  لضوابط تشغيل موسعة.
- `--encode-space-directory` يشغل `cargo xtask space-directory encode` تلقائيا. استخدم
  `--space-directory-out` اذا اردت ملف `.to` في مسار مختلف.

ينتج السكربت ثلاثة artefacts داخل `--output-dir` (افتراضيا المجلد الحالي)، مع رابع اختياري عند
تفعيل encoding:

1. `<slug>.manifest.json` — manifest للـ lane يحتوي quorum المدققين، namespaces محمية، و metadata
   اختيارية لـ hook runtime-upgrade.
2. `<slug>.catalog.toml` — snippet TOML مع `[[nexus.lane_catalog]]` و `[[nexus.dataspace_catalog]]`
   وقواعد التوجيه المطلوبة. تاكد من ضبط `fault_tolerance` على ادخال dataspace لحجم لجنة lane-relay
   (`3f+1`).
3. `<slug>.summary.json` — ملخص تدقيق يصف الهندسة (slug، segments، metadata) وخطوات rollout
   والامر الدقيق لـ `cargo xtask space-directory encode` (ضمن `space_directory_encode.command`).
   ارفق هذا JSON بتذكرة onboarding كدليل.
4. `<slug>.manifest.to` — ينتج عند تفعيل `--encode-space-directory`؛ جاهز لامر
   `iroha app space-directory manifest publish` عبر Torii.

استخدم `--dry-run` لمعاينة JSON/snippets دون كتابة ملفات، و `--force` لاستبدال artefacts الموجودة.

## 3. تطبيق التغييرات

1. انسخ manifest JSON الى `nexus.registry.manifest_directory` المهيأ (وايضا الى cache directory اذا
   كان registry يعكس bundles بعيدة). التزم بالملف اذا كانت manifests مُنسخة في مستودع التهيئة.
2. الصق snippet الكتالوج في `config/config.toml` (او `config.d/*.toml`). تاكد من ان
   `nexus.lane_count` لا يقل عن `lane_id + 1`، وحدّث اي `nexus.routing_policy.rules` يجب ان تشير
   الى الـ lane الجديدة.
3. قم بعمل encode (اذا لم تستخدم `--encode-space-directory`) وانشر manifest في Space Directory عبر
   الامر المخزن في summary (`space_directory_encode.command`). هذا يولد payload `.manifest.to`
   المتوقع من Torii ويسجل الادلة للمراجعين؛ ارسل عبر `iroha app space-directory manifest publish`.
4. شغل `irohad --sora --config path/to/config.toml --trace-config` واحفظ مخرجات trace في تذكرة
   rollout. هذا يثبت ان الهندسة الجديدة تطابق slug/segments المولدة.
5. اعد تشغيل المدققين المعينين للـ lane بعد نشر تغييرات manifest/catalog. احتفظ بـ summary JSON
   في التذكرة لاستخدامه في التدقيق مستقبلا.

## 4. بناء حزمة توزيع registry

عند جاهزية manifest و catalog snippet و summary، قم بتغليفها للتوزيع على المدققين. يقوم bundler
الحديث بنسخ manifests الى التخطيط المتوقع بواسطة `nexus.registry.manifest_directory` /
`cache_directory`، وينتج governance catalog overlay لتبديل الوحدات دون تعديل config الرئيسي، ويؤرشف
الحزمة اختياريا:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

النتائج:

1. `manifests/<slug>.manifest.json` — انسخ الى `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` — ضع في `nexus.registry.cache_directory` لتجاوز او تبديل وحدات
   الحوكمة (`--module ...` يتجاوز catalog المؤقت). هذا هو مسار الوحدات القابلة للاستبدال في NX-2:
   استبدل تعريف وحدة، اعد تشغيل bundler، ووزع overlay cache دون لمس `config.toml`.
3. `summary.json` — يتضمن digests SHA-256 / Blake2b لكل manifest مع metadata للـ overlay.
4. `registry_bundle.tar.*` اختياري — جاهز لـ Secure Copy / تخزين artefacts.

اذا كان نشرُك يعكس bundles الى hosts air-gapped، فقم بمزامنة مجلد المخرجات كاملا (او tarball).
يمكن للعقد المتصلة ان تركب مجلد manifests مباشرة، بينما العقد غير المتصلة تستهلك tarball، وتستخرج
وتنسخ manifests + overlay cache الى المسارات المهيأة.

## 5. اختبارات smoke للمدققين

بعد اعادة تشغيل Torii، شغل helper smoke للتحقق من ان lane تبلغ `manifest_ready=true`، وان المقاييس
تعرض عدد lanes المتوقع، وان gauge المختوم واضح. يجب ان تعرض lanes التي تتطلب manifest قيمة
`manifest_path` غير فارغة — يفشل helper بسرعة اذا غاب الدليل حتى تشمل ضوابط NX-7 اشارات الحزم
الموقعة تلقائيا:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v2/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

اضف `--insecure` عند اختبار بيئات self-signed. يخرج السكربت بقيمة غير صفرية اذا كانت lane مفقودة،
او sealed، او اذا انحرفت المقاييس/التلِمتري عن القيم المتوقعة. استخدم knobs الجديدة
`--min-block-height` و `--max-finality-lag` و `--max-settlement-backlog` و `--max-headroom-events`
لابقاء telemetria الارتفاع/النهائية/backlog/headroom ضمن الحدود التشغيلية. اجمع مع
`--max-slot-p95/--max-slot-p99` (ومع `--min-slot-samples`) لتطبيق SLO مدة slots NX-18 مباشرة داخل
helper، ومرر `--allow-missing-lane-metrics` فقط عند غياب gauges في staging (في الانتاج اترك
الافتراضيات).

الـ helper نفسه يطبق الان telemetria لاختبارات حمل scheduler. استخدم `--min-teu-capacity` لاثبات
ان كل lane تبلغ `nexus_scheduler_lane_teu_capacity`، واضبط استغلال slot عبر `--max-teu-slot-commit-ratio`
(يقارن `nexus_scheduler_lane_teu_slot_committed` مع السعة)، وحافظ على عدادات deferral/truncation
صفر عبر `--max-teu-deferrals` و `--max-must-serve-truncations`. هذه knobs تحول شرط NX-7 لـ
"اختبارات حمل اعمق" الى تحقق CLI قابل للتكرار: يفشل helper عندما تؤجل lane عمل PQ/TEU او عندما
يتجاوز TEU الملتزم لكل slot حدود headroom، ويطبع CLI ملخصا لكل lane لكي تلتقط حزم الادلة نفس
الارقام التي تحققت منها CI.

للتدقيق air-gapped (او CI) يمكنك اعادة تشغيل استجابة Torii محفوظة بدلا من الاتصال بعقدة حية:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

الـ fixtures تحت `fixtures/nexus/lanes/` تعكس artefacts التي ينتجها helper الخاص بالbootstrap حتى يمكن
lint manifests جديدة دون scripting خاص. تنفذ CI نفس التدفق عبر `ci/check_nexus_lane_smoke.sh`
وتشغل ايضا `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) لضمان توافق
helper smoke NX-7 مع صيغة payload المنشورة ولضمان قابلية اعادة انتاج digests/overlays.

عند اعادة تسمية lane، التقط احداث telemetria `nexus.lane.topology` (مثلا باستخدام
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) ومررها الى helper smoke.
الخيارات الجديدة `--telemetry-file/--from-telemetry` تقبل log newline-delimited و
`--require-alias-migration old:new` تتحقق من ان حدث `alias_migrated` سجل عملية rename:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

يضم fixture `telemetry_alias_migrated.ndjson` عينة rename معيارية كي تتحقق CI من parsing telemetria
دون الوصول لعقدة حية.

## 6. اختبارات حمل المدققين (دليل NX-7)

يتطلب roadmap **NX-7** من مشغلي lane التقاط تجربة حمل قابلة لاعادة الانتاج قبل اعتبار lane جاهزة
للانتاج. الهدف هو الضغط على lane بما يكفي لاختبار مدة slot، backlog التسوية، quorum DA، الاوراكل،
headroom الجدولة، ومقاييس TEU، ثم ارشفة النتائج بحيث يمكن للمدققين اعادة تشغيلها دون tooling مخصص.
المساعد `scripts/nexus_lane_load_test.py` يجمع smoke checks و slot-duration gating و slot bundle
manifest في حزمة واحدة لرفع نتائج الحمل مباشرة في تذاكر الحوكمة.

### 6.1 تحضير workload

1. انشئ مجلد run والتقط fixtures معيارية للـ lane تحت الاختبار:

   ```bash
   mkdir -p artifacts/nexus/load/payments-2026q2
   cargo xtask nexus-fixtures --output artifacts/nexus/load/payments-2026q2/fixtures
   ```

   هذه fixtures تعكس `fixtures/nexus/lane_commitments/*.json` وتوفر seed حتميا لمولد الحمل
   (سجل seed في `artifacts/.../README.md`).
2. نفذ baseline للـ lane قبل التجربة:

   ```bash
   scripts/nexus_lane_smoke.py \
     --status-url https://torii.example.com/v2/sumeragi/status \
     --metrics-url https://torii.example.com/metrics \
     --lane-alias payments \
     --expected-lane-count 3 \
     --min-block-height 50000 \
     --max-finality-lag 4 \
     --max-settlement-backlog 0.5 \
     --min-settlement-buffer 0.25 \
     --max-slot-p95 1000 \
     --max-slot-p99 1100 \
     --min-slot-samples 50 \
     --insecure \
     > artifacts/nexus/load/payments-2026q2/smoke_before.log
   ```

   احتفظ بمخرجات stdout/stderr في مجلد run حتى تكون عتبات smoke قابلة للتدقيق.
3. التقط سجل telemetria الذي سيغذي `--telemetry-file` (دليل alias migration) و
   `validate_nexus_telemetry_pack.py`:

   ```bash
   journalctl -u irohad -o json \
     --since "2026-05-10T09:00:00Z" \
     --until "2026-05-10T11:00:00Z" \
     > artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson
   ```

4. ابدأ workload للـ lane (ملف k6، replay harness، او اختبارات ingestion الاتحادية) واحتفظ بـ seed
   ورقم slot range، فهذه metadata تستخدم في مدقق telemetry manifest ضمن القسم 6.3.

5. قم بتجميع دليل التجربة عبر helper الجديد. قدم payloads الملتقطة لـ status/metrics/telemetry و
   aliases للـ lanes واي احداث alias migration مطلوبة. يكتب helper ملفات `smoke.log` و
   `slot_summary.json` و slot bundle manifest و `load_test_manifest.json` لربط كل شيء من اجل مراجعة
   الحوكمة:

   ```bash
   scripts/nexus_lane_load_test.py \
     --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
     --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
     --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
     --lane-alias payments \
     --lane-alias core \
     --expected-lane-count 3 \
     --slot-range 81200-81600 \
     --workload-seed NX7-PAYMENTS-2026Q2 \
     --require-alias-migration core:payments \
     --out-dir artifacts/nexus/load/payments-2026q2
   ```

   يطبق الامر نفس قيود quorum DA و oracle و settlement buffer و TEU و slot-duration المذكورة في
   الدليل ويولد manifest جاهزا للارفاق دون scripting مخصص.

### 6.2 تشغيل مُقاس

اثناء تشبع lane بالعمل:

1. التقط snapshots لِـ Torii status و metrics:

   ```bash
   curl -sS https://torii.example.com/v2/sumeragi/status \
     > artifacts/nexus/load/payments-2026q2/torii_status.json
   curl -sS https://torii.example.com/metrics \
     > artifacts/nexus/load/payments-2026q2/metrics.prom
   ```

2. احسب quantiles لزمن slot واحفظ summary:

   ```bash
   scripts/telemetry/check_slot_duration.py \
     artifacts/nexus/load/payments-2026q2/metrics.prom \
     --max-p95-ms 1000 \
     --max-p99-ms 1100 \
     --min-samples 200 \
     --json-out artifacts/nexus/load/payments-2026q2/slot_summary.json
   scripts/telemetry/bundle_slot_artifacts.py \
     --metrics artifacts/nexus/load/payments-2026q2/metrics.prom \
     --summary artifacts/nexus/load/payments-2026q2/slot_summary.json \
     --out-dir artifacts/nexus/load/payments-2026q2/slot_bundle \
     --metadata lane=payments \
     --metadata workload_seed=NX7-PAYMENTS-2026Q2
   ```

3. صدّر snapshot lane-governance كـ JSON + Parquet للتدقيق طويل المدى:

   ```bash
   cargo xtask nexus-lane-audit \
     --status artifacts/nexus/load/payments-2026q2/torii_status.json \
     --json-out artifacts/nexus/load/payments-2026q2/lane_audit.json \
     --parquet-out artifacts/nexus/load/payments-2026q2/lane_audit.parquet \
     --captured-at 2026-05-10T10:15:00Z
   ```

   يسجل snapshot JSON/Parquet الان استخدام TEU، ومستويات trigger للجدولة، وعدادات RBC chunk/byte،
   واحصاءات مخطط المعاملات لكل lane كي تظهر ادلة rollout كلا من backlog وضغط التنفيذ.

4. اعد تشغيل helper smoke عند ذروة الحمل لتقييم العتبات تحت الضغط (اكتب الى `smoke_during.log`)
   ثم اعد التشغيل بعد انتهاء workload (`smoke_after.log`).

### 6.3 Telemetry pack و manifest الحوكمة

يجب ان يحتوي مجلد run على telemetry pack (`prometheus.tgz`، تدفق OTLP، سجلات منظمة، ومخرجات
الحمل). تحقق من pack ودوّن metadata المطلوبة للحوكمة:

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus/load/payments-2026q2 \
  --manifest-out artifacts/nexus/load/payments-2026q2/telemetry_manifest.json \
  --expected prometheus.tgz --expected otlp.ndjson \
  --expected torii_structured_logs.jsonl --expected B4-RB-2026Q1.log \
  --slot-range 81200-81600 --require-slot-range \
  --workload-seed NX7-PAYMENTS-2026Q2 --require-workload-seed \
  --metadata lane=payments --metadata run=2026q2-rollout
```

اخيرا ارفق سجل telemetria الملتقط واطلب دليل alias migration عند اعادة تسمية lane خلال الاختبار:

```bash
scripts/nexus_lane_smoke.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --require-alias-migration core:payments \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-block-height 50000 \
  --max-finality-lag 4 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 200
```

ارشِف القطع التالية لتذكرة الحوكمة:

- `smoke_before.log`, `smoke_during.log`, `smoke_after.log`
- `metrics.prom`, `slot_summary.json`, `slot_bundle_manifest.json`
- `lane_audit.{json,parquet}`
- `telemetry_manifest.json` + محتويات pack (`prometheus.tgz`, `otlp.ndjson`, وغيرها)
- `nexus.lane.topology.ndjson` (او slice telemetria المناسب)

اصبحت التجربة قابلة للارجاع ضمن manifests في Space Directory و trackers الحوكمة كاختبار NX-7
المعياري لهذه الـ lane.

## 7. متابعات telemetria والحوكمة

- حدّث لوحات lane (`dashboards/grafana/nexus_lanes.json` والـ overlays ذات الصلة) بالـ lane id
  الجديد و metadata. المفاتيح المولدة (`contact`, `channel`, `runbook`, وغيرها) تسهل تعبئة labels.
- اربط قواعد PagerDuty/Alertmanager للـ lane الجديدة قبل تفعيل admission. يعكس `summary.json`
  checklist الموجودة في `docs/source/nexus_operations.md`.
- سجل manifest bundle في Space Directory حالما تصبح مجموعة المدققين live. استخدم نفس manifest JSON
  الذي يولده helper، موقعا حسب runbook الحوكمة.
- اتبع `docs/source/sora_nexus_operator_onboarding.md` لاختبارات smoke (FindNetworkStatus,
  reachability لـ Torii) واحتفظ بالادلة ضمن مجموعة artefacts الموضحة اعلاه.

## 8. مثال dry-run

لمعاينة artefacts دون كتابة ملفات:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --dry-run
```

يطبع الامر JSON summary و TOML snippet الى stdout، ما يسمح بتكرار سريع اثناء التخطيط.

---

للمزيد من السياق:

- `docs/source/nexus_operations.md` — checklist تشغيلية ومتطلبات telemetria.
- `docs/source/sora_nexus_operator_onboarding.md` — تدفق onboarding مفصل يشير الى helper الجديد.
- `docs/source/nexus_lanes.md` — هندسة lanes و slugs و layout التخزين المستخدم بواسطة الاداة.

</div>
