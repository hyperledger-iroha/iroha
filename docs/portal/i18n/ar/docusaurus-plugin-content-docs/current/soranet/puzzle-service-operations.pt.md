---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/puzzle-service-operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات خدمة اللغز
العنوان: Guia de Operacoes do Puzzle Service
Sidebar_label: العمليات تقوم بخدمة الألغاز
الوصف: Operacao do daemon `soranet-puzzle-service` لتذاكر القبول Argon2/ML-DSA.
---

:::ملاحظة فونتي كانونيكا
إسبيلها `docs/source/soranet/puzzle_service_operations.md`. Mantenha ambas as copias sincronzadas.
:::

# تقوم أداة التشغيل بخدمة الألغاز

يا شيطان `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) ينبعث
تذاكر الدخول respaldados por Argon2 التي تعيد صياغة السياسة `pow.puzzle.*`
قم بترحيل الرموز المميزة لقبول ML-DSA عند تكوينها،
مرحلات حافة دوس. يعرض خمس نقاط نهاية HTTP:

- `GET /healthz` - مسبار الحياة.
- `GET /v1/puzzle/config` - استعادة معلمات إثبات العمل/الألغاز الإضافية
قم بإجراء ترحيل JSON (`handshake.descriptor_commit_hex`، `pow.*`).
- `POST /v1/puzzle/mint` - إصدار تذكرة Argon2؛ أم الجسم JSON اختياري
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  قم بإدخال TTL أقل (مشبك في نافذة السياسة)، قم بتذكرة نسخة من TTL
  تجزئة التذكرة وإرجاعها إلى تتابع التتابع + بصمة الإصبع عند المحاولة
  كما تم تكوين رؤساء القتلة.
- `GET /v1/token/config` - عندما `pow.token.enabled = true`، قم بإعادة السياسة
  رمز الدخول المميز (بصمة المُصدر، حدود TTL/انحراف الساعة، معرف التتابع
  e o مجموعة الإلغاء mesclado).
- `POST /v1/token/mint` - قم بإصدار رمز القبول ML-DSA vinculado لاستئناف التجزئة
  جريمة قتل؛ o الجسم أسيتا `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.التذاكر التي يتم إنتاجها من خلال الخدمة التي تم التحقق منها لا تختبر التكامل
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`، الذي يمارس نظام التشغيل أيضًا
تقوم الخانقات بالترحيل خلال سيناريوهات DoS الحجمية.
(أدوات/soranet-relay/tests/adaptive_and_puzzle.rs:337)

## تكوين إرسال الرموز المميزة

تعريف الحرم الجامعي JSON do Relay em `pow.token.*` (veja
`tools/soranet-relay/deploy/config/relay.entry.json` كمثال) للتأهيل
رموز ML-DSA. لا يوجد حد أدنى، يجب عليك نشر جهة الإصدار وقائمة الإلغاءات
اختياري:

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

تقوم خدمة الألغاز بإعادة استخدام القيم وإعادة تخزينها تلقائيًا
تمت إعادة تحديث Norito JSON في وقت التشغيل. استخدم سطر الأوامر `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) للمصدر e
فحص الرموز المميزة دون الاتصال بالإنترنت، إضافة `token_id_hex` إلى ملف التجديد
قم بمراجعة الاعتمادات الموجودة قبل نشر التحديثات على المنتج.

قم بتمرير مفتاح سري لجهة الإصدار لخدمة اللغز عبر العلامات CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` يتم توفيره أيضًا عندما يكون الأمر سريًا ويتم إدارته من أجلك
أدوات خط أنابيب للنطاقات. أيها المراقب، قم بعمل ملف التجديد
تم تحديث `/v1/token/config`; تحديثات coordene كوماندو
`soranet-admission-token revoke` لتجنب حالة المراجعة الفاشلة.Defina `pow.signed_ticket_public_key_hex` no JSON do Relay للإعلان عن المهمة
نشر ML-DSA-44 يستخدم للتحقق من سرقة تذاكر PoW؛ `/v1/puzzle/config`
قم بإعادة إنتاج بصمة الإصبع الخاصة بك BLAKE3 (`signed_ticket_public_key_fingerprint_hex`)
لكي يقوم العملاء بإصلاح أو التحقق. تذاكر Assinados Sao Validados Contra
o ترحيل معرف e نسخة الارتباطات ومشاركة o مخزن الإلغاء نفسه؛ إثبات العمل
إجمالي التذاكر 74 بايت صالحة دائمًا عند التحقق من التذكرة الموقعة
هذا هو التكوين. قم بتمرير سر التوقيع عبر `--signed-ticket-secret-hex` ou
`--signed-ticket-secret-path` لبدء خدمة اللغز؛ o بدء التشغيل rejeita
أزواج المفاتيح متباينة إذا كانت سرية nao valida contra `pow.signed_ticket_public_key_hex`.
`POST /v1/puzzle/mint` لهذا `"signed": true` (و`"transcript_hash_hex"` اختياري) للفقرة
إعادة تذكرة موقعة Norito junto com os bytes do Ticket bruto؛ كردود
يشمل `signed_ticket_b64` و`signed_ticket_fingerprint_hex` للمساعدة في التنقيط
بصمات الأصابع دي اعادتها. طلبات com `signed = true` sao rejeitadas se o Signer Secret
لا يمكن تكوينه.

## Playbook de rotacao de chaves1. ** التزام واصف Coletar o novo. ** نشرة الحوكمة أو التتابع
   لا يلتزم الواصف بحزمة الدليل. انسخ الفقرة السداسية السداسية
   `handshake.descriptor_commit_hex` في تكوين JSON ومشاركة التتابع
   كوم يا خدمة اللغز.
2. **مراجعة حدود سياسة اللغز.** تأكيد القيم التي تم تحديثها
   `pow.puzzle.{memory_kib,time_cost,lanes}` هو موجود في خطة الإصدار.
   يقوم المشغلون بإدارة إعدادات Argon2 الحتمية بين المرحلات
   (الحد الأدنى 4 ميجابايت من الذاكرة، 1 <= ممرات <= 16).
3. **التحضير لإعادة التشغيل.** أعد تشغيل وحدة النظام أو الحاوية عندما يتم ذلك
   إعلان الحكم أو قطع الدوران. خدمة لا تدعم إعادة التحميل السريع؛
   إعادة التشغيل ضرورية لربط الواصف الجديد.
4. **صالح.** قم بإرسال تذكرة عبر `POST /v1/puzzle/mint` وقم بتأكيد ذلك
   `difficulty` e `expires_at` تتوافق مع سياسة nova. يا تقرير نقع
   (`docs/source/soranet/reports/pow_resilience.md`) يلتقط حدود زمن الوصول
   تطلعات للمرجعية. عندما تكون الرموز المميزة مؤهلة، بحثًا
   `/v1/token/config` لضمان إعلان بصمة إصبع المُصدر
   عدوى التمرد تتوافق مع القيم المنتظرة.

## إجراءات الطوارئ1. قم بتعريف `pow.puzzle.enabled = false` لتكوين مشاركة التتابع.
   Mantenha `pow.required = true` se os تذاكر التجزئة الاحتياطية المحددة
   دائم obrigatorios.
2. اختياريًا، فرض الإدخالات `pow.emergency` لإعادة استخدام الواصفات
   Antigos enquanto o gate Argon2 estiver دون اتصال بالإنترنت.
3. تحديث خدمة التتابع والألغاز لتطبيق التعديل.
4. شاشة `soranet_handshake_pow_difficulty` لضمان الصعوبة
   كيفية إعداد تقرير قيمة hashcash والتحقق من `/v1/puzzle/config`
   `puzzle = null`.

## المراقبة والتنبيهات- **زمن الوصول SLO:** يرافق `soranet_handshake_latency_seconds` ويحافظ على P95
  أباكسو دي 300 مللي ثانية. يتم إجراء اختبار النقع للإزاحات من أجل بيانات المعايرة
  خنق الحرس. (docs/source/soranet/reports/pow_resilience.md:1)
- **ضغط الحصص:** استخدم مقاييس الترحيل `soranet_guard_capacity_report.py` com
  لضبط فترات التهدئة `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`). (docs/source/soranet/relay_audit_pipeline.md:68)
- **محاذاة اللغز:** `soranet_handshake_pow_difficulty` يطور المراسلة أ
  تم إرجاع الصعوبة بواسطة `/v1/puzzle/config`. Divergencia indica تكوين التتابع
  قديمة أو إعادة تشغيل falho.
- **جاهزية الرمز المميز:** تنبيه بحد ذاته `/v1/token/config` cair para `enabled = false`
  بشكل منفصل أو إذا كان `revocation_source` يبلغ عن الطوابع الزمنية التي لا معنى لها. المشغلين
  يجب أن تقوم بتدوير الملف Norito من خلال CLI عند الرمز المميز لـ
  يتم التراجع عن ذلك لنقطة النهاية المحددة.
- **سلامة الخدمة:** اختبار `/healthz` للإيقاع المعتاد للحيوية والتنبيه
  إذا كان `/v1/puzzle/mint` يعيد HTTP 500 (يشير إلى عدم تطابق معلمات Argon2 ou
  فالهاس دي RNG). تظهر أخطاء سك الرمز المميز مثل استجابة HTTP 4xx/5xx
  `/v1/token/mint`; قم بتكرار الأخطاء مثل حالة الترحيل.

## تسجيل التدقيق الإلكتروني للامتثالتقوم المرحلات بإصدار أحداث `handshake` التي تتضمن دوافع الخانق
فترات التهدئة. ضمان أن يتم وصف خط أنابيب الامتثال
`docs/source/soranet/relay_audit_pipeline.md` يحتوي على سجلات لتتمكن من تعديلها
سياسة اللغز هي التدقيق. عندما تكون بوابة اللغز مؤهلة,
احصل على جميع التذاكر الصادرة ولقطة التكوين Norito مع
تذكرة بدء التشغيل لقاعات المستقبل. رموز القبول الصادرة قبل janelas
يجب أن يكون التحديث ذو قيمة قليلة `token_id_hex` ويتم إدخاله
ملف التجديد عند انتهاء الصلاحية أو التجديد.