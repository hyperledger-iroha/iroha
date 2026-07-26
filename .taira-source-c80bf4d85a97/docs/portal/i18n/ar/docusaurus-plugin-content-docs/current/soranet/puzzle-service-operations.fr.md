---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/puzzle-service-operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات خدمة اللغز
العنوان: دليل عمليات خدمة الألغاز
Sidebar_label: خدمة الألغاز
الوصف: استغلال البرنامج الخفي `soranet-puzzle-service` لتذاكر الدخول Argon2/ML-DSA.
---

:::ملاحظة المصدر الكنسي
:::

# دليل عمليات خدمة الألغاز

البرنامج الخفي `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) تم حذفه
قواعد تذاكر الدخول sur Argon2 التي تعكس سياسة `pow.puzzle.*` du
قم بالتتابع ثم بعد التكوين، قم بضبط رموز القبول ML-DSA من أجلها
حافة التبديلات. Il فضح نقاط نهاية cinq HTTP:

- `GET /healthz` - مسبار الحياة.
- `GET /v1/puzzle/config` - إعادة إصدار معلمات PoW/puzzle Effectifs
  تتابع du JSON (`handshake.descriptor_commit_hex`، `pow.*`).
- `POST /v1/puzzle/mint` - emet un تذكرة Argon2؛ خيار الأمم المتحدة JSON
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  طالب بـ TTL plus Court (مشبك نافذة السياسة)، ضع تذكرة واحدة
  نسخة تجزئة و renvoie un تذكرة Signe Par le Relay + l'empreinte de
  التوقيع عندما يتم تكوين عناصر التوقيع.
- `GET /v1/token/config` - عندما `pow.token.enabled = true`، إعادة السياسة
  رمز الدخول نشط (بصمة المُصدر، يحد من TTL/انحراف الساعة، معرف التتابع،
  et l'ensemble de revocation fusionne).
- `POST /v1/token/mint` - إصدار رمز مميز للدخول ML-DSA لاستئناف التجزئة
  فورني. يقبل الجسم `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.يتم التحقق من منتجات التذاكر المقدمة من خلال الخدمة من خلال اختبار التكامل
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`، الذي يمارس هذا أيضًا
خنق التتابع عند سيناريوهات DoS Volumetriques.[tools/soranet-relay/tests/adaptive_and_puzzle.rs:337]

## تكوين انبعاث الرموز المميزة

تعريف الأبطال JSON du Relay sous `pow.token.*` (عرض)
`tools/soranet-relay/deploy/config/relay.entry.json` pour un exemple) afin
تنشيط الرموز المميزة ML-DSA. بحد أدنى، قم بتزويد المفتاح العام لجهة الإصدار
وقائمة خيارات الإلغاء:

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

تقوم خدمة الألغاز بإعادة استخدام هذه القيم وإعادة شحن الملف تلقائيًا
Norito JSON للإلغاء في وقت التشغيل. استخدم CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) من أجل emettre et
مفتش الرموز المميزة في وضع عدم الاتصال، إضافة الإدخالات `token_id_hex` إلى الملف
الإلغاء، ومراجعة بيانات الاعتماد الموجودة قبل إجراء التحديثات
أون الإنتاج.

قم بتمرير سر خدمة إصدار اللغز عبر les flags CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` متاح أيضًا عندما يكون السر موجودًا على قدم المساواة
خط الأنابيب خارج النطاق. مراقب ملف الإلغاء يحفظ `/v1/token/config`
يوم؛ قم بتنسيق كل يوم باستخدام الأمر `soranet-admission-token revoke`
لتجنب حالة الإلغاء والتأخير.قم بتعريف `pow.signed_ticket_public_key_hex` في مرحل JSON للمعلن
يُستخدم المفتاح العام ML-DSA-44 للتحقق من علامات تذاكر PoW؛ `/v1/puzzle/config`
كرر la cle et son empreinte BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) afin
يمكن للعملاء تثبيت أداة التحقق. علامات التذاكر صالحة
contre le Relay ID et les Transcript Bindings et Partagent Le Meme Store de
الإلغاء؛ تبقى تذاكر إثبات العمل لمدة تصل إلى 74 ثمانيًا صالحة عند التحقق
تم تكوين التذكرة الموقعة. قم بتمرير سر التوقيع عبر `--signed-ticket-secret-hex`
ou `--signed-ticket-secret-path` au lancement du puzzle Service؛ لو رسم الزواج
قم بإعادة تعيين أزواج المفاتيح غير المتماسكة إذا كان السر غير صالح للمقاومة
`pow.signed_ticket_public_key_hex`. `POST /v1/puzzle/mint` قبول `"signed": true`
(والخيار `"transcript_hash_hex"`) لإرسال علامة تذكرة Norito ar
بالإضافة إلى البايتات من تذكرة بروت؛ تتضمن الردود `signed_ticket_b64` وآخرون
`signed_ticket_fingerprint_hex` لمتابعة إعادة تشغيل بصمات الأصابع. ليه
تم رفض الطلبات مع `signed = true` إذا كان سر التوقيع غير صحيح
تكوين.

## Playbook de Rotation des cles1. **Collecter le nouveau descriptor Commit.** الحوكمة Publie le Relay
   يلتزم الواصف في حزمة الدليل. انسخ السلسلة السداسية في
   `handshake.descriptor_commit_hex` في تكوين JSON Relay Partagee
   خدمة اللغز avec le.
2. **التحقق من إجراءات لغز السياسة.** تأكيد القيم
   `pow.puzzle.{memory_kib,time_cost,lanes}` يخطئ في محاذاة الخطة اليومية
   دي الافراج. يتعين على المشغلين التأكد من تحديد تكوين Argon2
   بين المرحلات (الحد الأدنى 4 ميجابايت من الذاكرة، 1 <= ممرات <= 16).
3. **قم بتحضير إعادة التشغيل.** أعد شحن وحدة النظام أو الحاوية
   حتى تعلن الحكومة عن انتقال التناوب. الخدمة غير مدعومة
   لا يتم إعادة التحميل الساخن؛ إعادة التخطيط مطلوبة للحصول على واصف جديد
   ارتكاب.
4. **التحقق.** قم بإرسال تذكرة عبر `POST /v1/puzzle/mint` وأكد ذلك
   `difficulty` و`expires_at` يتوافقان مع السياسة الجديدة. لو علاقة
   نقع (`docs/source/soranet/reports/pow_resilience.md`) التقاط des Bornes de
   الحضور الكمون صب المرجع. عندما تكون الرموز نشطة، يتم حذفها
   `/v1/token/config` للتحقق من إعلان مُصدر بصمة الإصبع
   مراسلة حساب الإلغاء لقيم الحضور.

## إجراء إلغاء التنشيط العاجل1. قم بتعريف `pow.puzzle.enabled = false` أثناء مشاركة تتابع التكوين.
   Gardez `pow.required = true` si les Tickets hashcash الاحتياطي doivent Rester
   إلزامية.
2. الخيار، فرض الإدخالات `pow.emergency` لرفضها
   الواصفات قديمة نظرًا لأن Porte Argon2 غير متصل بالإنترنت.
3. أعد تشغيل خدمة Relay and Puzzle لتطبيقها
   التغيير.
4. قم بمراقبة `soranet_handshake_pow_difficulty` للتحقق من الصعوبة
   ضع قيمة hashcash في الحضور، وتأكد من `/v1/puzzle/config`
   تقرير `puzzle = null`.

## المراقبة والتنبيه- **زمن الوصول SLO:** Suivez `soranet_handshake_latency_seconds` وgardez le P95
  سو 300 مللي ثانية. توفر إزاحات النقع اختبارًا لبيانات المعايرة
  من أجل حماية الخانق.[docs/source/soranet/reports/pow_resilience.md:1]
- **ضغط الحصة:** استخدم `soranet_guard_capacity_report.py` مع les
  تتابع المقاييس لضبط فترات التهدئة `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).[docs/source/soranet/relay_audit_pipeline.md:68]
- **محاذاة اللغز:** `soranet_handshake_pow_difficulty` تتوافق مع ذلك
  صعوبة العائدين على قدم المساواة `/v1/puzzle/config`. يشير الاختلاف إلى التكوين
  قم بترحيل معدل إعادة الزواج الذي لا معنى له.
- **جاهزية الرمز المميز:** تنبيه إذا `/v1/token/config` مزلق إلى `enabled = false`
  طريقة عدم الانتباه أو إذا كان `revocation_source` يشير إلى الطوابع الزمنية التي لا معنى لها.
  يقوم المشغلون بتدوير ملف الإلغاء Norito عبر CLI
  تم إيقاف رمز qu'un token لحماية نقطة النهاية هذه بشكل دقيق.
- **صحة الخدمة:** Probez `/healthz` مع إيقاع الحياة المعتاد وآخرون
  تنبيه إذا `/v1/puzzle/mint` قم بإعادة الرد على HTTP 500 (يشير إلى عدم تطابق
  من معلمات Argon2 أو من echecs RNG). تقع أخطاء سك العملات الرقمية في حد ذاتها
  واضح عبر الرد على HTTP 4xx/5xx على `/v1/token/mint`؛ سميت ليه
  يتكرر echecs كشرط للترحيل.

## تسجيل الامتثال والتدقيقتنطلق المرحلات من الأحداث `handshake` التي تشمل الأسباب
خنق وأوقات التهدئة. تأكد من امتثال خط الأنابيب
وصف في `docs/source/soranet/relay_audit_pipeline.md` إدخال سجلات ces afin
ما هي التغييرات في السياسة التي تظل قابلة للتدقيق. لغز Quand la Porte
نشط، قم بأرشفة خيارات التذاكر الدقيقة ولقطة الشاشة
التكوين Norito مع تذكرة البدء لعمليات التدقيق المستقبلية. ليه
رموز القبول mintes avant les fenetres de Maintenance doivent etre suivis
مع القيم `token_id_hex` وإدراجها في ملف الإلغاء
تنتهي صلاحية fois أو يتم إلغاءها.