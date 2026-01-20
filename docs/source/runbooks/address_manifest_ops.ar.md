---
lang: ar
direction: rtl
source: docs/source/runbooks/address_manifest_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb5d84c6939c186ebb4cd1b622e5ab66872349f5c177191c940a9e9fd63d1a17
source_last_modified: "2025-12-14T09:53:36.233782+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# دليل تشغيل عمليات بيان العناوين (ADDR-7c)

يُحوِّل هذا الدليل بند خارطة الطريق **ADDR-7c** إلى خطوات تشغيلية توضح كيفية
التحقق والنشر وإخراج الإدخالات من بيان حسابات/أسماء Sora Nexus. يُكمل العقد
التقني في [`docs/account_structure.md`](../../account_structure.md) §4 وتوقعات
القياس المسجلة في `dashboards/grafana/address_ingest.json`.

## 1. النطاق والمدخلات

| المُدخل | المصدر | ملاحظات |
|-------|--------|-------|
| حزمة البيان الموقعة (`manifest.json`, `manifest.sigstore`, `checksums.sha256`, `notes.md`) | Pin على SoraFS (`sorafs://address-manifests/<CID>/`) ومرآة HTTPS | تصدر الحزم عبر أتمتة الإصدارات؛ حافظ على هيكل الدليل عند النسخ. |
| Digest + تسلسل البيان السابق | الحزمة السابقة (نمط المسار نفسه) | مطلوب لإثبات التدرج/الثبات. |
| الوصول إلى القياس | لوحة Grafana `address_ingest` + Alertmanager | مطلوب لمراقبة تقاعد Local‑8 وارتفاع العناوين غير الصالحة. |
| الأدوات | `cosign`, `shasum`, `b3sum` (أو `python3 -m blake3`), `jq`, `iroha` CLI, `scripts/account_fixture_helper.py` | ثبّت الأدوات قبل تنفيذ قائمة التحقق. |

## 2. هيكلية المخرجات

كل حزمة تتبع البنية التالية؛ لا تعِد تسمية الملفات عند النسخ بين البيئات.

```
address-manifest-<REVISION>/
├── manifest.json              # canonical JSON (UTF-8, newline-terminated)
├── manifest.sigstore          # Sigstore bundle from `cosign sign-blob`
├── checksums.sha256           # one-line SHA-256 sum for each artifact
└── notes.md                   # change log (reason codes, tickets, owners)
```

حقول الرأس في `manifest.json`:

| الحقل | الوصف |
|-------|-------------|
| `version` | نسخة المخطط (حالياً `1`). |
| `sequence` | رقم مراجعة متزايد بشكل أحادي؛ يجب أن يزيد بواحد بالضبط. |
| `generated_ms` | الطابع الزمني UTC للنشر (ميلي ثانية منذ epoch). |
| `ttl_hours` | العمر الأقصى للذاكرة المؤقتة التي قد تحترمها Torii/SDK (الافتراضي 24). |
| `previous_digest` | BLAKE3 لبدن البيان السابق (hex). |
| `entries` | مصفوفة مرتبة من السجلات (`global_domain`, `local_alias`, `tombstone`). |

## 3. إجراء التحقق

1. **تنزيل الحزمة.**

   ```bash
   export REV=2025-04-12
   sorafs_cli fetch --id sorafs://address-manifests/${REV} --out artifacts/address_manifest_${REV}
   cd artifacts/address_manifest_${REV}
   ```

2. **حاجز checksum.**

   ```bash
   shasum -a 256 -c checksums.sha256
   ```

   يجب أن تُظهر جميع الملفات `OK`؛ اعتبر أي اختلاف محاولة عبث.

3. **تحقق Sigstore.**

   ```bash
   cosign verify-blob \
     --bundle manifest.sigstore \
     --certificate-identity-regexp 'governance\.sora\.nexus/addr-manifest' \
     --certificate-oidc-issuer https://accounts.google.com \
     manifest.json
   ```

4. **إثبات الثبات.** قارن `sequence` و`previous_digest` مع البيان المؤرشف:

   ```bash
   jq '.sequence, .previous_digest' manifest.json
   b3sum -l 256 ../address-manifest_<prev>/manifest.json
   ```

   يجب أن يطابق الـ digest المعروض قيمة `previous_digest`. لا يُسمح بقفزات
   التسلسل؛ أعد إصدار البيان إذا حدثت مخالفة.

5. **التأكد من TTL.** تأكد أن `generated_ms + ttl_hours` يغطي نوافذ النشر
   المتوقعة؛ وإلا فعلى الحوكمة إعادة النشر قبل انتهاء ذاكرة التخزين المؤقت.

6. **سلامة الإدخالات.**
   - يجب أن تتضمن إدخالات `global_domain` `{ "domain": "example", "chain": "sora:nexus:global", "selector": "global" }`.
   - يجب أن تتضمن إدخالات `local_alias` digest بطول 12 بايت ينتجه Norm v1
     (استخدم `iroha address convert <address-or-account_id> --format json --expect-prefix 753`
     للتحقق؛ يعكس ملخص JSON المجال عبر `input_domain` ويعيد `--append-domain` الترميز كـ `<ih58>@<domain>` للبيانات).
   - يجب أن تشير إدخالات `tombstone` إلى selector المتقاعد بدقة وتضمين
     `reason_code` و`ticket` و`replaces_sequence`.

7. **تكافؤ fixtures.** أعد توليد المتجهات القياسية وتأكد أن جدول digest المحلي
   لم يتغير بشكل غير متوقع:

   ```bash
   cargo xtask address-vectors
   python3 scripts/account_fixture_helper.py check --quiet
   ```

8. **حاجز الأتمتة.** شغّل مُحقِّق البيان لإعادة فحص الحزمة طرفاً لطرف
   (مخطط الرأس، شكل الإدخالات، checksums، وربط previous‑digest):

   ```bash
   cargo xtask address-manifest verify \
     --bundle artifacts/address-manifest_2025-05-12 \
     --previous artifacts/address-manifest_2025-04-30
   ```

   خيار `--previous` يشير إلى الحزمة السابقة مباشرة ليتحقق الأداة من أحادية
   `sequence` ويعيد حساب إثبات BLAKE3 لـ `previous_digest`. يفشل الأمر سريعاً عند
   انحراف checksum أو غياب حقول مطلوبة في `tombstone`، لذا أرفق المخرجات
   بتذكرة التغيير قبل طلب التواقيع.

## 4. تدفق تغييرات alias و tombstone

1. **اقتراح التغيير.** أنشئ تذكرة حوكمة تتضمن رمز السبب
   (`LOCAL8_RETIREMENT`, `DOMAIN_REASSIGNED`, إلخ) والمحددات المتأثرة.
2. **اشتقاق حمولة قياسية.** لكل alias يتم تحديثه نفّذ:

   ```bash
   iroha address convert snx1...@wonderland --expect-prefix 753 --format json > /tmp/alias.json
   jq '.canonical_hex, .input_domain' /tmp/alias.json
   ```

3. **صياغة إدخال البيان.** أضف سجل JSON مثل:

   ```json
   {
     "type": "tombstone",
     "selector": { "kind": "local", "digest_hex": "b18fe9c1abbac45b3e38fc5d" },
     "reason_code": "LOCAL8_RETIREMENT",
     "ticket": "ADDR-7c-2025-04-12",
     "replaces_sequence": 36
   }
   ```

   عند استبدال alias محلي بآخر عالمي، أضف سجل `tombstone` وسجل `global_domain`
   اللاحق الذي يحمل مُميِّز Nexus.

4. **تحقق من الحزمة.** أعد تشغيل خطوات التحقق أعلاه على البيان المبدئي قبل
   طلب التوقيعات.
   بعد التأكد من أن استخدام Local‑8 أصبح صفراً. لا تغيّره إلى `false` إلا في بيئات dev/test
   عندما تحتاج فترة soak إضافية.

## 5. المراقبة والرجوع

- لوحات المراقبة: `dashboards/grafana/address_ingest.json` (لوحات لـ
  `torii_address_invalid_total{endpoint,reason}` و
  `torii_address_local8_total{endpoint}` و
  `torii_address_collision_total{endpoint,kind="local12_digest"}` و
  `torii_address_collision_domain_total{endpoint,domain}`) يجب أن تبقى خضراء
  لمدة 30 يوماً قبل إغلاق حركة Local‑8/Local‑12 بشكل دائم.
- دليل الإغلاق: صدّر استعلام نطاق 30 يوم من Prometheus لـ
  `torii_address_local8_total` و`torii_address_collision_total` (مثلاً
  `promtool query range --output=json ...`) ونفّذ
  `cargo xtask address-local8-gate --input <file> --json-out artifacts/address_gate.json`؛
  أرفق JSON + مخرجات CLI بتذاكر الإطلاق حتى تتمكن الحوكمة من رؤية نافذة التغطية
  وتأكيد ثبات العدادات.
- التنبيهات (انظر `dashboards/alerts/address_ingest_rules.yml`):
  - `AddressLocal8Resurgence` — يرسل إنذاراً عند أي زيادة جديدة في Local‑8. تعامل معها
    (تجاوزاً للقيمة الافتراضية) حتى تُعالج العميل. أعد العلم إلى `true` عندما تصبح
    التليمترية نظيفة.
  - `AddressLocal12Collision` — يطلق لحظة تصادم labelين Local‑12 في نفس digest. أوقف
    ترقيات البيان، نفّذ `scripts/address_local_toolkit.sh` لتأكيد الربط، ونسّق مع
    حوكمة Nexus قبل إعادة إصدار السجل المتأثر.
  - `AddressInvalidRatioSlo` — يحذر عندما تتجاوز الإرسالات غير الصحيحة IH58/المضغوطة
    (مع استثناء رفض Local‑8/strict‑mode) نسبة 0.1 % لمدة 10 دقائق. تحقق من
    `torii_address_invalid_total` حسب السياق/السبب ونسّق مع فريق SDK المالك قبل
    إعادة تفعيل الوضع الصارم.
- السجلات: احتفظ بخطوط `manifest_refresh` من Torii ورقم تذكرة الحوكمة في `notes.md`.
- الرجوع: أعد نشر الحزمة السابقة (نفس الملفات مع تذكرة تشير للرجوع) واضبط مؤقتاً

## 6. المراجع

- [`docs/account_structure.md`](../../account_structure.md) §§4–4.1 (العقد).
- [`scripts/account_fixture_helper.py`](../../../scripts/account_fixture_helper.py) (مزامنة fixtures).
- [`fixtures/account/address_vectors.json`](../../../fixtures/account/address_vectors.json) (digests معيارية).
- [`dashboards/grafana/address_ingest.json`](../../../dashboards/grafana/address_ingest.json) (التليمترية).

</div>
