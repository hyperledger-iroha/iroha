---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/puzzle-service-operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات خدمة اللغز
العنوان: دليل عمليات خدمة الألغاز
Sidebar_label: عمليات خدمة الألغاز
الوصف: تذاكر دخول Argon2/ML-DSA لـ `soranet-puzzle-service` daemon کی آبریشنز.
---

:::ملاحظة المصدر الكنسي
`docs/source/soranet/puzzle_service_operations.md` هذا هو العنوان. إذا لم يتم تعيين وثائق برانا للتقاعد، فلا داعي للقلق بشأن مزامنة وارنز.
:::

# دليل عمليات خدمة اللغز

`tools/soranet-puzzle-service/` أو `soranet-puzzle-service` البرنامج الخفي
تذاكر الدخول المدعومة من Argon2 تتابع سياسة `pow.puzzle.*`
قم بنسخ البطاقة، وبعد تكوين مرحلات الحافة من جانب ML-DSA
وسيط رموز القبول کرتا ہے۔ تعرض نقاط نهاية HTTP ما يلي:

- `GET /healthz` - مسبار الحياة.
- `GET /v1/puzzle/config` - تتابع JSON (`handshake.descriptor_commit_hex`, `pow.*`) سے
  المزيد من معلمات إثبات العمل/الألغاز أكثر دقة.
- `POST /v1/puzzle/mint` - تذكرة Argon2 بالنعناع کرتا ہے؛ هيئة JSON اختيارية
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  TTL TTL TTL (مشبك نافذة السياسة)، تجزئة التذكرة والنص
  سے ربط كرتا ہے، ومفاتيح التوقيع التي تم تكوينها ہوں لترحيل التذكرة الموقعة +
  التوقيع بصمة الإصبع واپس کرتا ہے۔
- `GET /v1/token/config` - جب `pow.token.enabled = true` ہو تو رمز القبول النشط
  سياسة واپس کرتا ہے (بصمة المُصدر، حدود انحراف TTL/الساعة، معرف التتابع، و
  مجموعة الإلغاء المدمجة).
- `POST /v1/token/mint` - رمز قبول ML-DSA بالنعناع کرتا ہے جو تجزئة السيرة الذاتية المقدمة
  سے ملزمة ہوتا ہے؛ نص الطلب `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`
  أقبل كرتا ہے۔خدمة کے بنئے گئے تذاكر کو اختبار التكامل
`volumetric_dos_soak_preserves_puzzle_and_latency_slo` يمكن التحقق من صحة ما قمت به
سيناريوهات DoS الحجمية التي يتم تشغيلها أثناء تتابع الخانق هي ممارسة كرتا.
【أدوات/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## تكوين إصدار الرمز کرنا

`pow.token.*` کے تحت تتابع مجموعة حقول JSON کریں (مثال کے لئے
`tools/soranet-relay/deploy/config/relay.entry.json` (رقم) إلى الرموز المميزة ML-DSA
تمكين ہوں۔ كيفية إصدار المفتاح العام لجهة الإصدار وقائمة الإبطال الاختيارية:

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

خدمة الألغاز هي قيم إعادة استخدام الكرتا ووقت التشغيل مع إبطال Norito JSON
ما عليك فعله هو إعادة تحميل الكرتا. سطر الأوامر `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) يستخدم مرة أخرى
الرموز المميزة غير المتصلة بالنعناع/الفحص ہوں، الإبطال فائل میں إدخالات `token_id_hex` إلحاق ہوں،
وتحديثات الإنتاج متاحة حاليًا ومراجعة بيانات الاعتماد.

المفتاح السري للمصدر وأعلام CLI وخدمة الألغاز التي يمكن اكتشافها:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` هو جهاز يعمل على إدارة خط أنابيب الأدوات السري خارج النطاق.
مراقب ملف الإلغاء `/v1/token/config` رکھتا ہے؛ التحديثات کو
`soranet-admission-token revoke` إدارة تنسيق حالة الإلغاء
لا يوجد تأخير.Relay JSON `pow.signed_ticket_public_key_hex` مجموعة تذاكر PoW الموقعة
تحقق من إعلان المفتاح العام ML-DSA-44؛ `/v1/puzzle/config` هو مفتاح و
إنها بصمة BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) وتردد صدى
دبوس التحقق من العملاء. معرف ترحيل التذاكر الموقعة وروابط النص كانت مختلفة
التحقق من صحة التفويض ومتجر الإلغاء هذا ومشاركة البطاقة؛ تذاكر إثبات العمل (PoW) الخام ذات 74 بايت
تم تكوين أداة التحقق من التذكرة الموقعة مرة واحدة لتكون صالحة. سر الموقع کو
`--signed-ticket-secret-hex` أو `--signed-ticket-secret-path` إطلاق الخدمة
پر تمرير کریں؛ أزواج المفاتيح غير المتطابقة عند بدء التشغيل ترفض المفتاح السري
`pow.signed_ticket_public_key_hex` لا يوجد خطأ في التحقق من الصحة. `POST /v1/puzzle/mint`
`"signed": true` (و `"transcript_hash_hex"` اختياري) قبول کرتا ہے تاکہ Norito مشفر
بايتات التذكرة الأولية الموقعة کے ساتھ واپس ہو؛ الردود موجودة `signed_ticket_b64`
و`signed_ticket_fingerprint_hex` يتضمن ميزة إعادة تشغيل مسار بصمات الأصابع.
` signed = true` رفض الطلبات إذا لم يتم تكوين سر الموقّع.

## كتاب اللعب لتدوير المفاتيح1. **التزام واصف جديد.** تتابع حزمة دليل الحوكمة
   يلتزم الواصف بنشر کرتی ہے۔ يتم استخدام سلسلة سداسية عشرية لترحيل تكوين JSON
   `handshake.descriptor_commit_hex` لدي نسخة من خدمة الألغاز المشتركة التي تمت مشاركتها.
2. **مراجعة حدود سياسة اللغز.** تم تحديث بطاقة التحقق
   `pow.puzzle.{memory_kib,time_cost,lanes}` خطة إصدار القيم متوافقة مع ذلك. المشغلين کو
   يعمل تكوين Argon2 على ترحيل حتمية أكبر (أقل من 4 MiB من الذاكرة،
   1 <= ممرات <= 16).
3. **مرحلة إعادة التشغيل.** يعلن تغيير تناوب الإدارة عن وحدة systemd أو
   إعادة تحميل الحاوية. لا توجد خدمة إعادة التحميل السريع؛ لا يوجد واصف يرتكب لینے کے لئے
   إعادة التشغيل ضروري ہے۔
4. **التحقق من صحة البطاقة.** `POST /v1/puzzle/mint` إصدار التذكرة وفحصها
   `difficulty` و`expires_at` سياسة جديدة متطابقة. تقرير نقع
   (`docs/source/soranet/reports/pow_resilience.md`) مرجع کے لئے حدود زمن الوصول المتوقعة
   القبض على كرتا ہے۔ الرموز المميزة تمكنك من جلب `/v1/token/config` إلى جهة الإصدار المعلن عنها
   بصمة الإصبع وعدد الإبطال القيم المتوقعة سے تتطابق مع ہوں۔

## إجراء تعطيل الطوارئ1. تعيين تكوين التتابع المشترك `pow.puzzle.enabled = false`.
   `pow.required = true` يجب إعادة النظر في تذاكر التجزئة الاحتياطية.
2. يتم فرض إدخالات `pow.emergency` الاختيارية على بوابة Argon2 دون اتصال بالإنترنت
   الواصفات التي لا معنى لها ترفض ہوں۔
3. خدمة التتابع والألغاز دون إعادة تشغيل التطبيق.
4. `soranet_handshake_pow_difficulty` مراقبة صعوبة قيمة التجزئة المتوقعة
   قم بإسقاط السجل وتحقق من السجل `/v1/puzzle/config` `puzzle = null`.

## المراقبة والتنبيه- **زمن الوصول SLO:** `soranet_handshake_latency_seconds` تتبع المسار وP95 لا يزيد عن 300 مللي ثانية.
  إزاحة اختبار النقع صمامات الحماية لبيانات المعايرة.
  【docs/source/soranet/reports/pow_resilience.md:1】
- **ضغط الحصص:** `soranet_guard_capacity_report.py` مقاييس التتابع المستخدمة في الاستخدام
  `pow.quotas` Cooldowns (`soranet_abuse_remote_cooldowns`, `soranet_handshake_throttled_remote_quota_total`) لحن ہوں۔
  【docs/source/soranet/relay_audit_pipeline.md:68】
- **محاذاة اللغز:** `soranet_handshake_pow_difficulty` إلى `/v1/puzzle/config` متواصلة
  صعوبة المباراة هانا چائے. تكوين ترحيل التباعد الذي لا معنى له أو فشل في إعادة التشغيل.
- **جاهزية الرمز المميز:** إذا حدث `/v1/token/config` بعد المتوقع `enabled = false` أو حدث ذلك
  `revocation_source` تقرير الطوابع الزمنية القديمة لتنبيهك. مشغلي واجهة CLI Norito
  قم بتدوير ملف الإلغاء مرة أخرى عند تقاعد الرمز المميز أو نقطة النهاية الصحيحة.
- **سلامة الخدمة:** `/healthz` هو الإيقاع المعتاد للحيوية عند التحقيق والتنبيه إذا حدث ذلك
  `/v1/puzzle/mint` استجابات HTTP 500 د (عدم تطابق معلمة Argon2 أو فشل RNG).
  أخطاء سك الرمز المميز `/v1/token/mint` لاستجابات HTTP 4xx/5xx التي تبدو رائعة؛ الإخفاقات المتكررة
  حالة الترحيل مسموح بها.

##الامتثال وتسجيل التدقيقتنطلق أحداث `handshake` المنظمة من أسباب الاختناق وفترات التهدئة.
يقوم هذا الدليل `docs/source/soranet/relay_audit_pipeline.md` بتتبع خط أنابيب الامتثال الذي يستوعب السجلات
تغيير سياسة اللغز قابل للتدقيق. بوابة اللغز تمكن ہو تو سك عينات التذاكر وتكوين Norito
لقطة وتذكرة طرح وأرشيف ثابت لعمليات التدقيق المستقبلية للأجهزة. صيانة نوافذ سے پہلے
النعناع رموز القبول المميزة لقيم `token_id_hex` التي تتبع ما هو جديد وتنتهي أو تلغي
قبل ملف الإلغاء، قم بإدراج ما هو جديد.