---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/puzzle-service-operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات خدمة اللغز
العنوان: دليل عمليات خدمة الألغاز
Sidebar_label: عمليات خدمة الألغاز
الوصف: عملية البرنامج الخفي `soranet-puzzle-service` لتذاكر الدخول Argon2/ML-DSA.
---

:::ملاحظة فوينتي كانونيكا
ريفليجا `docs/source/soranet/puzzle_service_operations.md`. يتم الحفاظ على الإصدارات المتزامنة حتى يتم سحب المستندات المتوارثة.
:::

# دليل عمليات خدمة الألغاز

الخفي `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) ينبعث
تذاكر الدخول مطلوبة من قبل Argon2 والتي تحدد السياسة `pow.puzzle.*`
التتابع، عند تكوينه، رموز الوسائط المميزة للدخول ML-DSA en
حافة عدد المرحلات. كشف نقاط نهاية cinco HTTP:- `GET /healthz` - مسبار الحياة.
- `GET /v2/puzzle/config` - تحويل المعلمات الفعالة لـ PoW/puzzle
  Tomados del JSON del Relay (`handshake.descriptor_commit_hex`، `pow.*`).
- `POST /v2/puzzle/mint` - احصل على تذكرة Argon2; هيئة الأمم المتحدة JSON اختيارية
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  اطلب TTL mas corto (قم بإغلاق نافذة السياسة)، ثم قم بإدراج تذكرة واحدة
  قم بتجزئة النص وقم بإعادة تذكرة ثابتة من خلال التتابع + La huella de
  الشركة عندما تحتوي على مفاتيح ثابتة تم تكوينها.
- `GET /v2/token/config` - عندما `pow.token.enabled = true`، devuelve la
  رمز تفعيل سياسة القبول (بصمة المُصدر، حدود TTL/انحراف الساعة،
  معرف التتابع ومجموعة الإلغاء المجمعة).
- `POST /v2/token/mint` - احصل على رمز دخول ML-DSA مرتبط بالسيرة الذاتية
  شرط التجزئة؛ الجسم أسيبتا `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

يتم التحقق من التذاكر التي يتم إنتاجها من خلال الخدمة من خلال اختبار التكامل
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`، الذي يعمل أيضًا
خنق التتابع خلال سيناريوهات DoS الحجمية.

## تكوين إصدار الرموز

تكوين الحرم الجامعي JSON del Relay bajo `pow.token.*` (ver
`tools/soranet-relay/deploy/config/relay.entry.json` كمثال) للتأهيل
الرموز المميزة ML-DSA. على الأقل، أثبت الإعلان الرئيسي للمصدر وقائمة واحدة
اختياري للإلغاء:

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```تقوم خدمة الألغاز بإعادة استخدام هذه القيم وتحميل الأرشيف تلقائيًا
Norito JSON للإلغاءات في وقت التشغيل. الولايات المتحدة الأمريكية CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) للأكوانيار ه
فحص الرموز المميزة في وضع عدم الاتصال، إضافة `token_id_hex` إلى الملف
الإلغاءات ومراجعة بيانات الاعتماد الموجودة قبل نشر التحديثات أ
إنتاج.

Pasa la clave سر مُصدر خدمة الألغاز عبر الأعلام CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` متاح أيضًا عند إدارة السر
خط أنابيب للأدوات خارج النطاق. مراقب ملف الإلغاء
تم تحديث mantiene `/v2/token/config`; تحديث التنسيق مع القائد
`soranet-admission-token revoke` لتجنب الإلغاء في حالة الإلغاء.تكوين `pow.signed_ticket_public_key_hex` وتتابع JSON للإعلان
تم استخدام المفتاح الرئيسي ML-DSA-44 للتحقق من تذاكر شركات PoW؛ `/v2/puzzle/config`
نسخة طبق الأصل من المفتاح y su huella BLAKE3 (`signed_ticket_public_key_fingerprint_hex`)
حتى يتمكن العملاء من العثور على أداة التحقق. التذاكر المؤكدة صالحة
مقابل معرف التتابع وروابط النسخ ومشاركة نفس مخزن الإلغاء؛
تذاكر PoW الخام من 74 بايت تبقى صالحة عند التحقق
تم تكوين التذكرة الموقعة. قم بتمرير سر الشركة عبر `--signed-ticket-secret-hex`
o `--signed-ticket-secret-path` لبدء خدمة اللغز؛ El arranque rechaza
Pares de claves que noصدفة إذا كان السر غير صالح ضد `pow.signed_ticket_public_key_hex`.
`POST /v2/puzzle/mint` يقبل `"signed": true` (واختياري `"transcript_hash_hex"`) للفقرة
تحويل تذكرة ثابتة Norito جنبًا إلى جنب مع بايتات التذكرة بشكل مباشر؛ لاس
تتضمن الردود `signed_ticket_b64` و`signed_ticket_fingerprint_hex` للمساعدة
إعادة تشغيل بصمات الأصابع النقطية. تم تقديم الطلبات مع `signed = true`
إذا لم يتم تكوين سر الشركة.

## كتاب قواعد اللعبة لتدوير المفاتيح1. **تذكر التزام الواصف الجديد.** نشر الحوكمة
   يلتزم الواصف في حزمة الدليل. نسخ السلسلة السداسية en
   `handshake.descriptor_commit_hex` في تكوين التتابع JSON
   خدمة compartido con el puzzle.
2. **مراجعة حدود سياسة اللغز.** تأكيد القيم
   تم تحديث `pow.puzzle.{memory_kib,time_cost,lanes}` وفقًا للخطة
   دي الافراج. يجب على المشغلين الحفاظ على تكوين Argon2 المحدد
   بين المرحلات (الحد الأدنى 4 ميجابايت من الذاكرة، 1 <= ممرات <= 16).
3. **تحضير التجديد.** إعادة شحن وحدة النظام أو الحاوية مرة واحدة
   أعلنت الحكومة عن قطع التناوب. الخدمة لا تدعم إعادة التحميل السريع؛
   إذا كانت هناك حاجة إلى تجديد لارتكاب الواصف الجديد.
4. **صالح.** قم بإصدار تذكرة عبر `POST /v2/puzzle/mint` وقم بتأكيد ذلك
   تتزامن القيم `difficulty` و`expires_at` مع السياسة الجديدة. المعلومات
   نقع (`docs/source/soranet/reports/pow_resilience.md`) يلتقط الحدود
   دي لاتنسيا إسبيرادوس كمرجع. عندما تكون الرموز المميزة مؤهلة،
   راجع `/v2/token/config` للتأكد من إعلان مُصدر بصمة الإصبع
   ويتزامن حساب الإلغاءات مع القيم المستهلكة.

## إجراءات إزالة التأهيل في حالات الطوارئ1. قم بتكوين `pow.puzzle.enabled = false` في قسم تكوين المرحل.
   Mantiene `pow.required = true` si los Tickets hashcash الاحتياطي deben seguir
   التزام بالسلامة.
2. يتم تطبيق الإدخالات الاختيارية `pow.emergency` للبحث عن الواصفات
   عفا عليها الزمن منذ أن تم إغلاق بوابة Argon2 دون اتصال بالإنترنت.
3. قم بتحديث التتابع وخدمة اللغز لتطبيق التغيير.
4. قم بمراقبة `soranet_handshake_pow_difficulty` للتأكد من الصعوبة
   في انتظار التحقق من قيمة hashcash، والتحقق من الإبلاغ `/v2/puzzle/config`
   `puzzle = null`.

## مراقبة وتنبيهات- **زمن الوصول SLO:** عنوان `soranet_handshake_latency_seconds` والحفاظ على P95
  لمدة 300 مللي ثانية. أثبتت إزاحات اختبار النقع بيانات المعايرة
  لخانق الحرس.[docs/source/soranet/reports/pow_resilience.md:1]
- **ضغط الحصة:** الولايات المتحدة الأمريكية `soranet_guard_capacity_report.py` مع مقاييس التتابع
  لضبط فترات التهدئة `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).[docs/source/soranet/relay_audit_pipeline.md:68]
- **محاذاة اللغز:** `soranet_handshake_pow_difficulty` يجب أن تتزامن مع ذلك
  تم تطويره بصعوبة بواسطة `/v2/puzzle/config`. يشير الاختلاف إلى التكوين
  تتابع التتابع أو فشل إعادة التشغيل.
- **جاهزية الرمز المميز:** تنبيه si `/v2/token/config` cae a `enabled = false`
  بشكل منفصل إذا كان `revocation_source` يُبلغ عن الطوابع الزمنية التي لا معنى لها. المشغلون
  قم بتدوير ملف الإلغاءات Norito عبر CLI عند سحب رمز مميز
  للحفاظ على نقطة النهاية هذه دقيقة.
- **صحة الخدمة:** Sondea `/healthz` في الإيقاع المعتاد للحياة والتنبيه
  si `/v2/puzzle/mint` إعادة الرد على HTTP 500 (يشير إلى عدم تطابق المعلمات
  Argon2 أو فشل RNG). تظهر أخطاء سك العملات الرقمية كإجابة
  HTTP 4xx/5xx en `/v2/token/mint`; القيام بخطوات متكررة مثل حالة الترحيل.

## تسجيل الامتثال والتدقيقتصدر المرحلات أحداثًا مبنية على `handshake` تتضمن أسبابًا
دواسة الوقود ومدة التهدئة. تأكد من وصف خط أنابيب الامتثال
en `docs/source/soranet/relay_audit_pipeline.md` يستوعب هذه السجلات لذلك
تغيير سياسة اللغز سيغان سييندو قابل للتدقيق. عندما يكون باب اللغز
هذه الكفاءة، أرشفة تذاكر التذاكر الصادرة ولقطة التكوين
Norito مع تذكرة بدء تشغيل القاعات المستقبلية. رموز القبول
يجب أن تكون المخلفات المنبعثة قبل صيانة النوافذ أقل أهمية من قيمها
`token_id_hex` وقم بإدراجه في ملف الإلغاءات بمجرد انتهاء صلاحيته
شون إبطالي.