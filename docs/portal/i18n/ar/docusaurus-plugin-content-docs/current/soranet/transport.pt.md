---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/transport.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
الهوية: النقل
العنوان: Visao geral do Transporte SoraNet
Sidebar_label: Visao عام للنقل
الوصف: المصافحة، تدوير الأملاح ودليل القدرات لتراكب مجهول في SoraNet.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة مخصصة للنقل SNNet-1 المحدد في `docs/source/soranet/spec.md`. Mantenha ambas as copias sincronzadas.
:::

SoraNet وطبقة مجهولة تدعم نطاق جلب SoraFS، وتدفق Norito RPC وممرات البيانات المستقبلية لـ Nexus. يحدد برنامج النقل (يتضمن خارطة الطريق **SNNet-1** و**SNNet-1a** و **SNNet-1b**) حتمية المصافحة، وتداول القدرات بعد الكم (PQ) ومخطط تدوير الأملاح لكل تتابع، ويراقب العميل والبوابة نفس الوضع الآمن.

## ميتا ونموذج الأحمر- إنشاء دوائر ثلاثية القفزات (دخول -> وسط -> خروج) حول QUIC v1 حتى لا يتمكن أقرانك المسيئون من الوصول إلى Torii مباشرة.
- Sobrepor um handshake Noise XX *hybrid* (Curve25519 + Kyber768) ao QUIC/TLS لربط فصول الجلسة بنسخة TLS.
- Exigir TLVs de capacidades que anunciem supporte PQ KEM/assinatura, papel do Relay e Verso de Protocolo; تطبيق GREASE على أنواع غير معروفة للمحافظة على المستقبلات المزروعة الممتدة.
- أملاح دوارة للمحتوى اليومي وإصلاح مرحلات الحماية لمدة 30 يومًا حتى لا يتمكن الدليل من إلغاء تشفير العملاء.
- إصلاحات الخلايا في 1024 ب، وحشو الحشو/الخلايا الوهمية، وتصدير حتمية القياس عن بعد لالتقاط تجارب التخفيض السريع.

## خط أنابيب المصافحة (SNNet-1a)

1. **مغلف QUIC/TLS** - يتصل العملاء بالتتابعات عبر QUIC v1 ويكملون مصافحة TLS 1.3 المعتمدة من Ed25519 من خلال إدارة CA. يقوم مُصدر TLS (`tls-exporter("soranet handshake", 64)`) بتحريك الضوضاء حتى تكون النصوص منفصلة.
2. **Noise XX hybrid** - سلسلة بروتوكول `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` com prologue = مُصدِّر TLS. تدفق الرسائل:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   النتيجة DH do Curve25519 وتغليف Kyber مختلط مع سلاسل متماثلة نهائية. فشل في التفاوض بشأن مادة PQ التي تم إحباطها أو المصافحة بالكامل - وليس هناك بديل كلاسيكي.3. **ألغاز التذاكر والرموز المميزة** - يمكن التتابع من طلب بطاقة إثبات العمل لـ Argon2id قبل `ClientHello`. تذاكر ساو إطارات مع بادئة كومبريمنتو كيو كاريغام أ سولوكاو Argon2 hasheada e expiram داخل حدود السياسة:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   رموز القبول مع البادئة `SNTK` تطرح الألغاز عند اغتيال ML-DSA-44، صالحية الباعث ضد السياسة وقائمة الإلغاء.

4. **عرض قدرة TLV** - الحمولة النهائية لنقل الضوضاء من سعات TLV الموضحة بعيدًا. يقوم العملاء بإيقاف الاتصال إذا كانت أي قدرة على السداد (PQ KEM/التثبيت أو الورق أو العكس) تكون مباشرة أو متباينة من مدخل الدليل.

5. **تسجيل النسخ** - يقوم بترحيل التسجيل أو تجزئة النسخ، وطباعة TLS الرقمية، ومحتوى TLV للكشف عن خفض المستوى، وخطوط الأنابيب للامتثال.

## قدرة TLVs (SNNet-1c)

إعادة استخدام القدرات في مغلف TLV Fixo de `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

نصائح محددة لهذا:- `snnet.pqkem` - مستوى Kyber (`kyber768` للطرح الفعلي).
- `snnet.pqsig` - جناح القتل PQ (`ml-dsa-44`).
- `snnet.role` - ورق التتابع (`entry`، `middle`، `exit`، `gateway`).
- `snnet.version` - معرف البروتوكول العكسي.
- `snnet.grease` - إدخالات التخفيضات المسبقة في الحجز لضمان التسامح مع TLVs المستقبلية.

يحتفظ العملاء بقائمة مسموحة من TLVs المطلوبة والمصافحة عند حذفها أو تخفيضها. Relays publicam o mesmo set em your microdescriptor do Directory للتأكد من صحة الحتمية.

## Rotacao de salts e CID المسببة للعمى (SNNet-1b)

- نشر الحوكمة في السجل `SaltRotationScheduleV1` بقيم `(epoch_id, salt, valid_after, valid_until)`. لا يتم تدمير المرحلات والبوابات وكاميرات التقويم أو ناشر الدليل.
- العملاء يطبقون الملح الجديد على `valid_after`، ويحافظون على الملح السابق لمدة 12 ساعة من الرحمة ويحتفظون بتاريخ من 7 حقب لتحمل الحداثة المتأخرة.
- Identificadores cegos canonicos usam:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" || salt || cid)
  ```

  يتم استخدام البوابات عبر `Sora-Req-Blinded-CID` وصدى الصدى في `Sora-Content-CID`. دائرة التعمية/الطلب (`CircuitBlindingKey::derive`) موجودة في `iroha_crypto::soranet::blinding`.
- في حالة التتابع بعد مرور عصر ما، يتم تداخل الدوائر الجديدة لتخفيض التقويم وإصدار `SaltRecoveryEventV1`، حيث تعمل لوحات المعلومات عند الطلب كوظيفة ترحيل منفصلة.## Dados do Directory e Politica de Guard

- الواصفات الدقيقة تحمل هوية التتابع (Ed25519 + ML-DSA-65)، ورؤوس PQ، وTLVs للسعة، وعلامات المنطقة، وأهلية الحماية، وعصر الملح المعلن.
- مجموعات حراسة الإصلاح للعملاء لمدة 30 يومًا وذاكرة التخزين المؤقت المستمرة `guard_set` جنبًا إلى جنب مع لقطة الدليل. تعرض أغلفة CLI وSDK بصمة الإصبع في ذاكرة التخزين المؤقت حتى تتم إضافة أدلة الطرح إلى تعديلات التعديل.

## القياس عن بعد وقائمة التحقق من الطرح- مقاييس التصدير قبل الإنتاج:
  -`soranet_handshake_success_total{role}`
  -`soranet_handshake_failure_total{reason}`
  -`soranet_handshake_latency_seconds`
  -`soranet_capability_mismatch_total`
  -`soranet_salt_rotation_lag_seconds`
- تستمر حدود التنبيه عند إغلاق مصفوفة SLO لتدوير الأملاح SOP (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) ويجب أن يتم تحديدها بواسطة Alertmanager قبل الترويج لها.
- التنبيهات: تصنيف الخطأ > 5% لمدة 5 دقائق، تأخر الملح > 15 دقيقة، أو عدم تطابق القدرات المرصودة في المنتج.
- خطوات الطرح:
  1. تنفيذ اختبارات قابلية التشغيل البيني للترحيل/العميل عبر التدريج عبر المصافحة الهجينة والمكدس PQ الماهر.
  2. قم بإجراء SOP لتدوير الأملاح (`docs/source/soranet_salt_plan.md`) وقم بحفر القطع الأثرية لتغيير السجل.
  3. التمكن من التعامل مع القدرات بدون دليل، بعد الدوران لمرحلات الدخول، والمرحلات المتوسطة، ومرحلات الخروج وعملاء الشركات.
  4. بصمات الأصابع لحراسة المسجل، والجداول الزمنية الملحية، ولوحات القياس عن بعد لكل حالة؛ anexar o حزمة الأدلة `status.md`.

تسمح لك متابعة قائمة التحقق هذه بدمج أوقات المشغلين والعملاء وSDK مع وسائل نقل SoraNet أثناء انتظار متطلبات التحديد والاستماع الملتقط بدون خريطة طريق SNNet.