---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/puzzle-service-operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات خدمة اللغز
العنوان: Руководство по експлуатации خدمة الألغاز
Sidebar_label: عمليات خدمة الألغاز
الوصف: برنامج Эксплуатация daemon `soranet-puzzle-service` لتذاكر القبول Argon2/ML-DSA.
---

:::note Канонический источник
:::

# Руководство по ксплуатации خدمة الألغاز

تم تسجيل البرنامج الخفي `soranet-puzzle-service` (`tools/soranet-puzzle-service/`)
تذاكر الدخول المدعومة من Argon2، والتي تتبع سياسة `pow.puzzle.*` في التتابع
وعندما يكون ذلك ممكنًا، قم بتوسط الرموز المميزة لقبول ML-DSA من مرحلات الحافة الخاصة.
نقترح قراءة نقاط نهاية HTTP:

- `GET /healthz` - مسبار الحياة.
- `GET /v2/puzzle/config` - تفعيل معلمات PoW/puzzle الفعالة،
  считанные из Relay JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v2/puzzle/mint` - شراء تذكرة Argon2؛ هيئة JSON اختيارية
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  إغلاق TTL (مثبت بنافذة السياسة)، تذكرة خاصة
  من خلال تجزئة النص وإصدار تذكرة موقعة من التتابع + بصمة التوقيع
  عند الحصول على مفاتيح التوقيع.
- `GET /v2/token/config` - عندما `pow.token.enabled = true`، قم بالتنشيط
  سياسة رمز القبول (بصمة المُصدر، حدود انحراف TTL/الساعة، معرف الترحيل،
  ومجموعة الإلغاء المدمجة).
- `POST /v2/token/mint` - رمز قبول ML-DSA، متصل بالمورد
  استئناف التجزئة؛ نموذج الجسم `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.التذاكر، خدمة متميزة، تحقق في اختبار التكامل
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`، وهذا أيضًا
قم بتشغيل صمامات التتابع في سيناريو DoS الحجمي في الوقت المناسب.

## Настройка выпуска tokenov

قم بإضافة مرحل JSON تحت `pow.token.*` (cm.
`tools/soranet-relay/deploy/config/relay.entry.json` كمثال)، لذلك
قم بتضمين رموز ML-DSA. الحد الأدنى من السماح للمصدر بالمفتاح العام والاختياري
قائمة الإلغاء:

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

يتم الاستفادة من خدمة الألغاز مرة أخرى ويتم إعادة تصميمها تلقائيًا
Norito JSON ملف الإلغاء في وقت التشغيل. استخدم CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) للتنزيل و
التحقق من الرموز المميزة في وضع عدم الاتصال، إدخالات `token_id_hex` في ملف الإلغاء файл и
ضمان تدقيق بيانات الاعتماد الخاصة بنشر التحديثات في الإنتاج.

قم بإدخال المفتاح السري لجهة الإصدار في خدمة الألغاز من خلال علامات CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` جاهز للاستخدام عندما يستخدم أدوات خارج النطاق بشكل سري
خط أنابيب. مراقب ملف الإلغاء дератит `/v2/token/config` актуальным;
قم بالتنسيق مع الأمر `soranet-admission-token revoke` لذلك
قم بإلغاء تحديد حالة الإلغاء.قم بتثبيت `pow.signed_ticket_public_key_hex` في تتابع JSON للامتثال
ML-DSA-44 المفتاح العام للتحقق من تذاكر إثبات العمل الموقعة؛ `/v2/puzzle/config`
مفتاح القفل وبصمة الإصبع BLAKE3 (`signed_ticket_public_key_fingerprint_hex`)،
يمكن للعملاء التحقق من رقم التعريف الشخصي. يتم التحقق من التذاكر الموقعة من خلال معرف التتابع و
روابط النسخ واستخدامها كمتجر إبطال؛ تذاكر إثبات العمل (PoW) الخام ذات 74 بايت
يتم التحقق من صحة التذكرة الموقعة من خلال أداة التحقق من التذكرة الموقعة. اسبقية سر التوقيع
من خلال `--signed-ticket-secret-hex` أو `--signed-ticket-secret-path` عند الإغلاق
خدمة اللغز؛ ابدأ بإلغاء تحديد أزواج المفاتيح غير الضرورية، إذا لم يتم التحقق من صحة السر
بروتيف `pow.signed_ticket_public_key_hex`. تم تنفيذ `POST /v2/puzzle/mint`
`"signed": true` (والاختياري `"transcript_hash_hex"`) للرجوع إليه
التذكرة الموقعة بترميز Norito تحتوي على بايت تذكرة أولية؛ تتضمن الإجابات
`signed_ticket_b64` و `signed_ticket_fingerprint_hex` لمسح بصمات الأصابع.
يتم إلغاء الاتصال مع `signed = true`، إذا لم يتم تثبيت سر الموقع.

## تدوير المفاتيح1. **التعرف على التزام الواصف الجديد.** نشر واصف التتابع للحوكمة
   الالتزام بحزمة الدليل. انسخ السلسلة السداسية في `handshake.descriptor_commit_hex`
   تكوينات ترحيل JSON التي تستخدم خدمة الألغاز.
2. **التحقق من لغز سياسة الحدود.** تأكد من أن هذه فكرة عامة
   `pow.puzzle.{memory_kib,time_cost,lanes}` خطة الإصدار المرغوبة. المشغلين
   يجب مراعاة تكوين Argon2 بين المرحلات المحددة (الحد الأدنى 4 MiB
   памяти, 1 <= الممرات <= 16).
3. ** قم بإعادة التشغيل. ** قم بإعادة شحن وحدة systemd أو الحاوية بعد ذلك، كيف
   الحكم يلتزم بالتناوب. الخدمة لا تدعم إعادة التحميل السريع؛ لل
   يتطلب تطبيق الواصف الجديد إعادة التشغيل.
4. **التحقق.** قم بشراء التذكرة من خلال `POST /v2/puzzle/mint` ثم قم بالتحقق،
   что `difficulty` و `expires_at` يتوافقان مع السياسة الجديدة. تقرير نقع
   (`docs/source/soranet/reports/pow_resilience.md`) الالتزام بحدود زمن الوصول
   للصحافيين. عندما تكون الرموز المميزة متضمنة، اضغط على `/v2/token/config` للتنزيل،
   أن بصمة المُصدر المُعلن عنها وعدد مرات الإلغاء يزيد من أهمية الأمر.

## إجراء تعطيل الطوارئ1. قم بتثبيت `pow.puzzle.enabled = false` في تكوينات التتابع العامة. توقف
   `pow.required = true`، إذا تم إلغاء التذاكر الاحتياطية لـ hashcash.
2. قم بشكل اختياري بتثبيت إدخالات `pow.emergency` لإلغاء تثبيت الإدخالات
   واصفات пока بوابة Argon2 حاليا.
3. قم بإعادة تشغيل خدمة التتابع والألغاز من أجل تحسين التحسين.
4. مراقبة `soranet_handshake_pow_difficulty` للتأكد من صعوبة ذلك
   قم بالدفع إلى آخر علامة hashcash، وتحقق من `/v2/puzzle/config`
   أرسل `puzzle = null`.

## المراقبة والتنبيه- **زمن الوصول SLO:** راجع `soranet_handshake_latency_seconds` ثم انتقل إلى P95
  أقل من 300 مللي ثانية. نقع إزاحة الاختبار في بيانات المعايرة لخانق الحرس.
  【docs/source/soranet/reports/pow_resilience.md:1】
- **ضغط الحصص:** استخدم `soranet_guard_capacity_report.py` مع مقاييس التتابع
  للأجهزة `pow.quotas` Cooldowns (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).[docs/source/soranet/relay_audit_pipeline.md:68]
- **محاذاة اللغز:** `soranet_handshake_pow_difficulty` مطلوب منك
  الصعوبة من `/v2/puzzle/config`. يشير الاختلاف إلى تكوين التتابع القديم
  أو إعادة تشغيل غير ضرورية.
- **جاهزية الرمز المميز:** تنبيه، إذا تم إرسال `/v2/token/config` بشكل غير ضروري إلى
  `enabled = false` أو `revocation_source` تنشئ طوابع زمنية قديمة. المشغلين
  يجب عليك إعادة تدوير ملف الإلغاء Norito من خلال CLI عند إنشاء الرمز المميز، لذلك
  تم تعليق نقطة النهاية تمامًا.
- **سلامة الخدمة:** قم بتثبيت `/healthz` في إيقاع وتنبيه الحياة بشكل عام،
  إذا تم استخدام `/v2/puzzle/mint` لـ HTTP 500 (يشير إلى عدم تطابق معلمة Argon2
  أو فشل RNG). يتم إنشاء سك الرموز المميزة باستخدام HTTP 4xx/5xx
  `/v2/token/mint`; قم بقراءة حالة الترحيل بعد ذلك.

## تسجيل الامتثال والتدقيقترحيل الأحداث الهيكلية العامة `handshake`، بما في ذلك أسباب الخانق
وفترات التهدئة. كن متأكدا من أن خط أنابيب الامتثال من
`docs/source/soranet/relay_audit_pipeline.md` يقوم بتسجيل هذه السجلات من أجل التحسين
سياسة اللغز оставались Auditables. عندما تكون بوابة اللغز شاملة, أرشفة
التذاكر المسكوكة ولقطة التكوين Norito مع تذكرة التشغيل
لتدقيق الحسابات. رموز القبول، المتاحة قبل نوافذ الصيانة،
يجب عليك المتابعة إلى `token_id_hex` والإضافة في ملف الإلغاء بعد ذلك
الكتابة أو الملاحظة.