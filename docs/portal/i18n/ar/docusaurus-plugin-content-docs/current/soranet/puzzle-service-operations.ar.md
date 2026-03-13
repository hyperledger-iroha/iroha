---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/puzzle-service-operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات خدمة اللغز
العنوان: دليل عمليات خدمة الالغاز
Sidebar_label: عمليات خدمة الالغاز
الوصف: تشغيل daemon `soranet-puzzle-service` لتذاكر الادميشن Argon2/ML-DSA.
---

:::ملاحظة المصدر القياسي
تعكس `docs/source/soranet/puzzle_service_operations.md`. حافظ على النسختين متزامنتين حتى يتم تقاعد الوثائق القديمة.
:::

# دليل عمليات خدمة الالغاز

يقوم البرنامج الخفي `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) باصدار
تذاكر الدخول المساهمة بـ Argon2 والتي تدعم السياسة الخاصة بالـ Relay `pow.puzzle.*`
وأنت تقوم بتجهيزها، وتقوم بوساطة رموز قبول ML-DSA نيابة عن مرحلات الحافة.
المستخدمة في توزيع نقاط نهاية HTTP:

- `GET /healthz` - فحص الحيوية.
- `GET /v2/puzzle/config` - يعيد معلمات PoW/puzzle إلى المسح
  من JSON الخاص بالـ Relay (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v2/puzzle/mint` - يصدر تذكرة Argon2; الجسم JSON اختياري
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  يطلب TTL اقصر (يتم ضبطه ضمن سياسة النافذة)، ويربط التذكرة بـ Transcript hash،
  ويعيد تذكرة موقعة من التتابع + بصمة التوقيع عندما تكون لوحة المفاتيح التوقيع هيئة.
- `GET /v2/token/config` - عندما `pow.token.enabled = true` يعيد
  رمز الدخول المميز (بصمة المُصدر، نطاق TTL/انحراف الساعة، معرف التتابع،
  إلغاء NRF الصلبة).
- `POST /v2/token/mint` - يصدر رمز قبول ML-DSA مرتبطا بـ تجزئة السيرة الذاتية مقدم؛
  الجسم يقبل `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.عملية التحقق من الشركة المصنعة عبر خدمة تكاملية
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`، والتي تختبرها أيضًا
throttles الخاصة بالـ Relay خلال سيناريوهات DoS متوسطة.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## تهيئة حرية التوكنات

اضبط استخدام JSON الخاص بالـ Relay ضمن `pow.token.*` (انظر
`tools/soranet-relay/deploy/config/relay.entry.json` كمثال) توكنات ML-DSA.
على ما يؤكده المفتاح العام للـ المُصدر وقائمة الإبطال الاختيارية:

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

خدمة الالغاز القائمة على استخدام هذه القيم تستحق عادة تحميل ملف الإلغاء بصيغة
Norito JSON في وقت التشغيل باللون الأسود. استخدم CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) لاإصدار التوكنات
واستعراضها دون اتصال، إضافة `token_id_hex` الى ملف الإلغاء، ودقيق
الاعتمادات قبل دفع التحديثات الى الانتاج.

مرر المفتاح السري للـ المُصدر الى خدمة الالغاز عبر flags CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

ومن ثم `--token-secret-hex` عندما يكون السرا عبر خط أنابيب خارجي. هو
مراقب لإلغاء الملف بالحفاظ على `/v2/token/config` محدثا؛ نسق التحديثات مع
أمرر `soranet-admission-token revoke` باستثناء تاخر حالة الإلغاء.اضبط `pow.signed_ticket_public_key_hex` في JSON الخاص بالـ Relay للاعلان عن المفتاح
العام ML-DSA-44 المستخدم من تذاكر PoW الموقعة; تعكس `/v2/puzzle/config`
المفتاح وبصمته BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) كي أسرع للعملاء
من تثبيت المدقق. يتم التحقق من الموقع من خلال معرف الترحيل وربط النص
وتشارك نفس المخزن الإلغاء؛ تظل تذاكر PoW الخام بحجم 74 قرصًا صالحًا عند تهيئة
التحقق من التذكرة الموقعة. مرر سر خاص بالموقع عبر `--signed-ticket-secret-hex`
او `--signed-ticket-secret-path` عند تشغيل خدمة الالغاز; رفض أزواج المفاتيح
غير المتطابقة اذا لم يتحقق السر من `pow.signed_ticket_public_key_hex`.
`POST /v2/puzzle/mint` يقبل `"signed": true` (واختياريا `"transcript_hash_hex"`) لارجاع
تذكرة موقعة مشفر بـ Norito الى جانب البايتات التذكرة الخام; تتضمن الردود
`signed_ticket_b64` و`signed_ticket_fingerprint_hex` لتتبع بصمات الأصابع. يتم
رفض الطلبات مع `signed = true` اذا لم يتم تهيئة الموقع السري.

## دليل روماتيزر1. **جمع واصف الالتزام الجديد.** نشر واصف تتابع الحكم الالتزام في
   حزمة الدليل. انسخ السلسلة السداسية الى `handshake.descriptor_commit_hex` داخل
   الصارمة JSON الخاص بالـ Relay مع خدمة الالغاز.
2. **مراجعة نطاق حافلات الالغاز.** متأكد ان قيم
   `pow.puzzle.{memory_kib,time_cost,lanes}` المحدثة تماشى مع خطة الاصدار. يجب
   على تشغيلين ابقاء تهيئة Argon2 حتمية عبر المرحلات (حد ادنى 4 MiB ذاكرة،
   1 <= ممرات <= 16).
3. **تجهيز اعادة التشغيل.** اعد تحميل وحدة systemd او لاحظ عند اعلان الحكم
   عن القطع الخاص بالتدوير. خدمة لا تدعم hot-reload؛ إعادة التشغيل مطلوبة
   كامل واصف ارتكاب الجديد.
4. **التحقق.** اصدار تذكرة عبر `POST /v2/puzzle/mint` وتأكد ان `difficulty` و`expires_at`
   حسب السياسة الجديدة. تقرير نقع (`docs/source/soranet/reports/pow_resilience.md`)
   يختار حدود الكمون للمرجعية. عندما تكون التوكنات مفعلة، اجلب
   `/v2/token/config` للتأكد من أن بصمة المُصدر لعدد وعدد الإلغاء
   حسبان القيم.

##عمل تعطيل الطارئ

1. اضبط `pow.puzzle.enabled = false` في مرحلتي الصلبة. تستخدم بـ `pow.required = true`
   اذا كان يجب ان تبقى تذاكر hashcash الاحتياطية الزامية.
2. اختياريا، تنفيذ ادخالات `pow.emergency` لرفض الواصفات القديمة بينما بوابة Argon2
   غير متصل.
3. اعد تشغيل كل من تتابع الالغاز بعد الغد.
4. راقب `soranet_handshake_pow_difficulty` وكذلك ان يتم ضبطها الى قيمة hashcash
   لاحظ، وتحقق من ان `/v2/puzzle/config` لذلك `puzzle = null`.

##الدفاع والتنبيهات- **زمن الوصول SLO:** تتبع `soranet_handshake_latency_seconds` وابق P95 أقل من 300 مللي ثانية.
  قوة الإزاحات الخاصة باختبار نقع بيانات تجديد لخناقات الـ Guard.
  【docs/source/soranet/reports/pow_resilience.md:1】
- **ضغط الحصص:** استخدم `soranet_guard_capacity_report.py` مع مقاييس الترحيل
  لضبط فترات التبريد لـ `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).[docs/source/soranet/relay_audit_pipeline.md:68]
- **محاذاة اللغز:** يجب ان يطابق `soranet_handshake_pow_difficulty`
  المعادة من `/v2/puzzle/config`. يشير الفرق الى التتابع المناسب قديم او فشل اعادة
  تشغيل.
- **جاهزية الرمز المميز:** تنبيه اذا متعدد `/v2/token/config` الى `enabled = false`
  بشكل غير متوقع او اذا ابلغ `revocation_source` عن الطوابع الزمنية القديمة. يجب على
  مشغلات روماني ملفوف بنمط Norito عبر CLI عند سحب توكن على
  دقة نقطة النهاية هذه.
- **صحة الخدمة:** افحص `/healthz` حسب إيقاع الحياة المنتظم ونبه اذا
  عاد `/v2/puzzle/mint` استجابات HTTP 500 (يدل على عدم تطابق معلمات Argon2 او
  فشل RNG). حدوث أخطاء في سك الرموز عبر استجابات HTTP 4xx/5xx على `/v2/token/mint`؛
  تتفاعل مع الخفاقات الأساسية كحالة الترحيل.

##قسم وتسجيل التدقيقتقوم ببث احداث `handshake` منظمة تتضمن اسباب throttle ومدد Cooldown.
تأكد من أن خط الأنابيب متكامل في `docs/source/soranet/relay_audit_pipeline.md`
تستوعب هذه السجلات حتى تبقى ولابد من الالغاز غير قابل للدقيق. عندما تكون
بوابة الالغاز مفعلة، ارشف تلخيص المخرج وsnapshot الصارمة Norito مع
تذكرة الطرح للمقررين المستقبليين. يجب تتبع علامات القبول للمدير السابق
نوافذ الصيانة عبر قيم `token_id_hex` وادراجها في ملف الإلغاء عند انتهاائها او
الغايات.