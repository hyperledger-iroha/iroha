---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/puzzle-service-operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: puzzle-service-operations
title: دليل عمليات خدمة الالغاز
sidebar_label: عمليات خدمة الالغاز
description: تشغيل daemon `soranet-puzzle-service` لتذاكر الادmission Argon2/ML-DSA.
---

:::note المصدر القياسي
يعكس `docs/source/soranet/puzzle_service_operations.md`. حافظ على النسختين متزامنتين حتى يتم تقاعد الوثائق القديمة.
:::

# دليل عمليات خدمة الالغاز

يقوم daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) باصدار
admission tickets المدعومة بـ Argon2 والتي تعكس policy الخاصة بالـ relay `pow.puzzle.*`
وعند تهيئتها، يقوم بوساطة ML-DSA admission tokens نيابة عن edge relays.
يعرض خمس نقاط نهاية HTTP:

- `GET /healthz` - فحص liveness.
- `GET /v1/puzzle/config` - يعيد معلمات PoW/puzzle الفعلية المسحوبة
  من JSON الخاص بالـ relay (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - يصدر تذكرة Argon2; body JSON اختياري
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  يطلب TTL اقصر (يتم ضبطه ضمن نافذة policy)، ويربط التذكرة بـ transcript hash،
  ويعيد تذكرة موقعة من relay + بصمة التوقيع عندما تكون مفاتيح التوقيع مهيئة.
- `GET /v1/token/config` - عندما `pow.token.enabled = true` يعيد سياسة
  admission-token النشطة (issuer fingerprint, حدود TTL/clock-skew, relay ID,
  ومجموعة revocation المدمجة).
- `POST /v1/token/mint` - يصدر ML-DSA admission token مرتبطا بـ resume hash المقدم؛
  body الطلب يقبل `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

تتم عملية التحقق من التذاكر المنتجة عبر خدمة الاختبار التكاملية
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`، والتي تختبر ايضا
throttles الخاصة بالـ relay خلال سيناريوهات DoS الحجمية.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## تهيئة اصدار التوكنات

اضبط حقول JSON الخاصة بالـ relay ضمن `pow.token.*` (انظر
`tools/soranet-relay/deploy/config/relay.entry.json` كمثال) لتمكين توكنات ML-DSA.
على الاقل قدم المفتاح العام للـ issuer وقائمة revocation اختيارية:

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

خدمة الالغاز تعيد استخدام هذه القيم وتقوم باعادة تحميل ملف revocation بصيغة
Norito JSON في runtime تلقائيا. استخدم CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) لاصدار التوكنات
واستعراضها offline، واضافة `token_id_hex` الى ملف revocation، وتدقيق
الاعتمادات قبل دفع updates الى الانتاج.

مرر المفتاح السري للـ issuer الى خدمة الالغاز عبر flags CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

يتوفر ايضا `--token-secret-hex` عندما يكون السر مدارا عبر pipeline خارجية. يقوم
watcher لملف revocation بالحفاظ على `/v1/token/config` محدثا؛ نسق التحديثات مع
امر `soranet-admission-token revoke` لتجنب تاخر حالة revocation.

اضبط `pow.signed_ticket_public_key_hex` في JSON الخاص بالـ relay للاعلان عن المفتاح
العام ML-DSA-44 المستخدم للتحقق من PoW tickets الموقعة; يعكس `/v1/puzzle/config`
المفتاح وبصمته BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) كي يتمكن clients
من تثبيت verifier. يتم التحقق من التذاكر الموقعة مقابل relay ID وtranscript bindings
وتشارك نفس مخزن revocation؛ تظل PoW tickets الخام بحجم 74 بايت صالحة عند تهيئة
signed-ticket verifier. مرر secret الخاص بالموقع عبر `--signed-ticket-secret-hex`
او `--signed-ticket-secret-path` عند تشغيل خدمة الالغاز; يرفض التشغيل keypairs
غير المتطابقة اذا لم يتحقق السر من `pow.signed_ticket_public_key_hex`.
`POST /v1/puzzle/mint` يقبل `"signed": true` (واختياريا `"transcript_hash_hex"`) لارجاع
signed ticket مشفر بـ Norito الى جانب bytes التذكرة الخام; تتضمن الردود
`signed_ticket_b64` و`signed_ticket_fingerprint_hex` لتتبع replay fingerprints. يتم
رفض الطلبات مع `signed = true` اذا لم يتم تهيئة secret الموقع.

## دليل تدوير المفاتيح

1. **جمع descriptor commit الجديد.** تنشر governance relay descriptor commit في
   directory bundle. انسخ سلسلة hex الى `handshake.descriptor_commit_hex` داخل
   تكوين JSON الخاص بالـ relay المشترك مع خدمة الالغاز.
2. **مراجعة حدود سياسة الالغاز.** تاكد ان قيم
   `pow.puzzle.{memory_kib,time_cost,lanes}` المحدثة تتماشى مع خطة الاصدار. يجب
   على المشغلين ابقاء تهيئة Argon2 حتمية عبر relays (حد ادنى 4 MiB ذاكرة،
   1 <= lanes <= 16).
3. **تجهيز اعادة التشغيل.** اعد تحميل وحدة systemd او الحاوية عند اعلان governance
   عن cutover الخاص بالتدوير. الخدمة لا تدعم hot-reload؛ اعادة التشغيل مطلوبة
   لالتقاط descriptor commit الجديد.
4. **التحقق.** اصدر تذكرة عبر `POST /v1/puzzle/mint` وتاكد ان `difficulty` و`expires_at`
   يطابقان السياسة الجديدة. تقرير soak (`docs/source/soranet/reports/pow_resilience.md`)
   يلتقط حدود الكمون المتوقعة للمرجعية. عندما تكون التوكنات مفعلة، اجلب
   `/v1/token/config` للتأكد من ان issuer fingerprint المعلن وعدد revocation
   يطابقان القيم المتوقعة.

## اجراء التعطيل الطارئ

1. اضبط `pow.puzzle.enabled = false` في تكوين relay المشترك. احتفظ بـ `pow.required = true`
   اذا كان يجب ان تبقى تذاكر hashcash fallback الزامية.
2. اختياريا، نفذ ادخالات `pow.emergency` لرفض descriptors القديمة بينما بوابة Argon2
   offline.
3. اعد تشغيل كل من relay وخدمة الالغاز لتطبيق التغيير.
4. راقب `soranet_handshake_pow_difficulty` لضمان ان الصعوبة تنخفض الى قيمة hashcash
   المتوقعة، وتحقق من ان `/v1/puzzle/config` يبلغ `puzzle = null`.

## المراقبة والتنبيهات

- **Latency SLO:** تتبع `soranet_handshake_latency_seconds` وابق P95 اقل من 300 ms.
  توفر offsets الخاصة باختبار soak بيانات معايرة لthrottles الـ guard.
  【docs/source/soranet/reports/pow_resilience.md:1】
- **Quota pressure:** استخدم `soranet_guard_capacity_report.py` مع relay metrics
  لضبط cooldowns لـ `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Puzzle alignment:** يجب ان يطابق `soranet_handshake_pow_difficulty` الصعوبة
  المعادة من `/v1/puzzle/config`. يشير الاختلاف الى تكوين relay قديم او فشل اعادة
  التشغيل.
- **Token readiness:** نبه اذا انخفض `/v1/token/config` الى `enabled = false`
  بشكل غير متوقع او اذا ابلغ `revocation_source` عن timestamps قديمة. يجب على
  المشغلين تدوير ملف revocation بنمط Norito عبر CLI عند سحب توكن للحفاظ على
  دقة هذا endpoint.
- **Service health:** افحص `/healthz` وفق cadence liveness المعتادة ونبه اذا
  اعاد `/v1/puzzle/mint` استجابات HTTP 500 (يدل على عدم تطابق معلمات Argon2 او
  فشل RNG). تظهر اخطاء token minting عبر استجابات HTTP 4xx/5xx على `/v1/token/mint`؛
  تعامل مع الاخفاقات المتكررة كحالة paging.

## الامتثال وتسجيل التدقيق

تقوم relays ببث احداث `handshake` منظمة تتضمن اسباب throttle ومدد cooldown.
تاكد من ان pipeline الامتثال الموصوفة في `docs/source/soranet/relay_audit_pipeline.md`
تستوعب هذه السجلات حتى تبقى تغييرات سياسة الالغاز قابلة للتدقيق. عندما تكون
بوابة الالغاز مفعلة، ارشف عينات التذاكر الصادرة وsnapshot تكوين Norito مع
تذكرة rollout للمدققين المستقبليين. يجب تتبع admission tokens الصادرة قبل
نوافذ الصيانة عبر قيم `token_id_hex` وادراجها في ملف revocation عند انتهائها او
الغائها.
