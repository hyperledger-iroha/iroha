---
lang: ar
direction: rtl
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر القياسي
تعكس `docs/source/sns/address_checksum_failure_runbook.md`. حدث ملف المصدر اولا، ثم زامن هذه النسخة.
:::

تظهر اخفاقات checksum كـ `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`) عبر Torii و SDKs وعملاء المحافظ/المستكشف. تتطلب عناصر خارطة الطريق ADDR-6/ADDR-7 الان من المشغلين اتباع هذا الدليل عند اطلاق تنبيهات checksum او تذاكر الدعم.

## متى نشغل هذا الدليل

- **التنبيهات:** `AddressInvalidRatioSlo` (معرف في `dashboards/alerts/address_ingest_rules.yml`) يطلق وتعرض التعليقات `reason="ERR_CHECKSUM_MISMATCH"`.
- **انحراف fixtures:** ملف Prometheus النصي `account_address_fixture_status` او لوحة Grafana تبلغ عن checksum mismatch لاي نسخة SDK.
- **تصعيدات الدعم:** فرق المحافظ/المستكشف/SDK تشير الى اخطاء checksum او فساد IME او مسح الحافظة الذي لم يعد يفك الترميز.
- **ملاحظة يدوية:** سجلات Torii تعرض بشكل متكرر `address_parse_error=checksum_mismatch` لنقاط نهاية الانتاج.

اذا كان الحادث يتعلق تحديدا بتصادمات Local-8/Local-12، فاتبع playbooks `AddressLocal8Resurgence` او `AddressLocal12Collision` بدلا من ذلك.

## قائمة الادلة

| الدليل | الامر / الموقع | ملاحظات |
|--------|-----------------|---------|
| Snapshot Grafana | `dashboards/grafana/address_ingest.json` | التقط توزيع الاسباب غير الصالحة ونقاط النهاية المتاثرة. |
| Payload التنبيه | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | ادرج تسميات السياق والطوابع الزمنية. |
| صحة fixtures | `artifacts/account_fixture/address_fixture.prom` + Grafana | يثبت ما اذا كانت نسخ SDK انحرفت عن `fixtures/account/address_vectors.json`. |
| استعلام PromQL | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | صدر CSV لوثيقة الحادث. |
| السجلات | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (او تجميع السجلات) | نظف PII قبل المشاركة. |
| تحقق fixture | `cargo xtask address-vectors --verify` | يؤكد ان المولد القياسي و JSON الملتزم متطابقان. |
| فحص تكافؤ SDK | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | شغل لكل SDK مذكور في التنبيهات/التذاكر. |
| سلامة الحافظة/IME | `iroha tools address inspect <literal>` | يكشف المحارف المخفية او اعادة كتابة IME؛ استشهد بـ `address_display_guidelines.md`. |

## الاستجابة الفورية

1. اقر بالتنبيه، واربط لقطات Grafana + خرج PromQL في خيط الحادث، ودوّن سياقات Torii المتاثرة.
2. جمد ترقيات manifest / اصدارات SDK التي تمس تحليل العناوين.
3. احفظ لقطات لوحة التحكم وملفات Prometheus النصية الناتجة في مجلد الحادث (`docs/source/sns/incidents/YYYY-MM/<ticket>/`).
4. اجلب عينات سجلات تظهر payloads `checksum_mismatch`.
5. اخطر مالكي SDK (`#sdk-parity`) بامثلة payload ليتمكنوا من triage.

## عزل السبب الجذري

### انحراف fixture او المولد

- اعد تشغيل `cargo xtask address-vectors --verify`; اعد التوليد اذا فشل.
- نفذ `ci/account_fixture_metrics.sh` (او `scripts/account_fixture_helper.py check` بشكل فردي) لكل SDK واكد ان fixtures المضمنة تطابق JSON القياسي.

### تراجعات ترميز العميل / IME

- افحص القيم التي قدمها المستخدمون عبر `iroha tools address inspect` للعثور على joins بعرض صفري او تحويلات kana او payloads مبتورة.
- طابق تدفقات wallet/explorer مع `docs/source/sns/address_display_guidelines.md` (اهداف النسخ المزدوج، التحذيرات، مساعدات QR) للتاكد من انها تتبع UX المعتمدة.

### مشاكل manifest او السجل

- اتبع `address_manifest_ops.md` لاعادة التحقق من اخر manifest bundle وللتاكد من عدم عودة محددات Local-8.
- استخدم `scripts/address_local_toolkit.sh audit ...` عندما تظهر محددات Local القديمة في payloads.

### حركة مرور خبيثة او تالفة

- حلل IPs/app IDs المخالفة عبر سجلات Torii و `torii_http_requests_total`.
- احتفظ بما لا يقل عن 24 ساعة من السجلات لمتابعة Security/Governance.

## التخفيف والتعافي

| السيناريو | الاجراءات |
|-----------|-----------|
| انحراف fixture | اعد توليد `fixtures/account/address_vectors.json`, اعد تشغيل `cargo xtask address-vectors --verify`, حدث حزم SDK, وارفق لقطات `address_fixture.prom` بالتذكرة. |
| تراجع SDK/عميل | افتح قضايا تشير الى fixture القياسي + خرج `iroha tools address inspect`, وامنع الاصدارات عبر CI تكافؤ SDK (مثل `ci/check_address_normalize.sh`). |
| ارساليات خبيثة | طبق rate-limit او احظر principals المخالفين، وصعد الى Governance اذا تطلب الامر tombstone للمحددات. |

بعد تطبيق التخفيفات، اعد تشغيل استعلام PromQL اعلاه لتاكيد ان `ERR_CHECKSUM_MISMATCH` يبقى عند الصفر (باستثناء `/tests/*`) لمدة 30 دقيقة على الاقل قبل خفض الحادث.

## الاغلاق

1. ارشف لقطات Grafana و CSV الخاص بـ PromQL ومقتطفات السجلات و `address_fixture.prom`.
2. حدث `status.md` (قسم ADDR) وصف السطر في roadmap اذا تغيرت الادوات/الوثائق.
3. سجل ملاحظات ما بعد الحادث تحت `docs/source/sns/incidents/` عند ظهور دروس جديدة.
4. تاكد ان ملاحظات اصدار SDK تذكر اصلاحات checksum عند الحاجة.
5. تحقق من بقاء التنبيه اخضر لمدة 24 ساعة وان فحوصات fixture تبقى خضراء قبل الاغلاق.
