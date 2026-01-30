---
lang: es
direction: ltr
source: docs/portal/docs/soranet/transport.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: transport
title: نظرة عامة على نقل SoraNet
sidebar_label: نظرة عامة على النقل
description: المصافحة، تدوير salts، وتوجيهات القدرات لطبقة اخفاء الهوية في SoraNet.
---

:::note المصدر القياسي
تعكس هذه الصفحة مواصفات نقل SNNet-1 في `docs/source/soranet/spec.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد مجموعة الوثائق القديمة.
:::

SoraNet هي طبقة اخفاء الهوية التي تدعم range fetches في SoraFS وبث Norito RPC وقنوات بيانات Nexus المستقبلية. برنامج النقل (عناصر خارطة الطريق **SNNet-1** و **SNNet-1a** و **SNNet-1b**) حدد مصافحة حتمية وتفاوض قدرات ما بعد الكم (PQ) وخطة لتدوير salts كي يلاحظ كل relay وclient وgateway نفس وضع الامان.

## الاهداف ونموذج الشبكة

- بناء دوائر بثلاث قفزات (entry -> middle -> exit) فوق QUIC v1 حتى لا يصل peers المسيئون الى Torii مباشرة.
- تكديس مصافحة Noise XX *hybrid* (Curve25519 + Kyber768) فوق QUIC/TLS لربط مفاتيح الجلسة بنص TLS.
- فرض TLVs قدرات تعلن دعم PQ KEM/التوقيع ودور relay واصدار البروتوكول؛ واستخدام GREASE للانواع غير المعروفة كي تبقى الامتدادات المستقبلية قابلة للنشر.
- تدوير salts لمحتوى التعمية يوميا وتثبيت guard relays لمدة 30 يوما حتى لا يؤدي churn في الدليل الى فك هوية clients.
- ابقاء cells ثابتة عند 1024 B وحقن padding/cells dummy وتصدير telemetry حتمية لالتقاط محاولات downgrade بسرعة.

## مسار المصافحة (SNNet-1a)

1. **QUIC/TLS envelope** - يتصل clients بالـ relays عبر QUIC v1 ويكملون مصافحة TLS 1.3 باستخدام شهادات Ed25519 الموقعة من governance CA. يزود TLS exporter (`tls-exporter("soranet handshake", 64)`) طبقة Noise بالبذور كي تكون النصوص غير قابلة للفصل.
2. **Noise XX hybrid** - سلسلة البروتوكول `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` مع prologue = TLS exporter. تدفق الرسائل:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   يتم خلط مخرجات DH لـ Curve25519 وكلا encapsulations الخاصة بـ Kyber في المفاتيح المتماثلة النهائية. فشل التفاوض على مادة PQ يجهض المصافحة بالكامل ولا يسمح باي fallback كلاسيكي فقط.

3. **Puzzle tickets و tokens** - يمكن للـ relays طلب ticket اثبات عمل Argon2id قبل `ClientHello`. التذاكر هي frames مع بادئة طول تحمل حل Argon2 المجزأ وتنتهي صلاحيتها ضمن حدود السياسة:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   tokens القبول المسبوقة بـ `SNTK` تتجاوز الالغاز عندما تتحقق صحة توقيع ML-DSA-44 من المُصدر مقابل السياسة النشطة وقائمة الالغاء.

4. **تبادل capability TLV** - الحمولة النهائية في Noise تنقل TLVs القدرات الموضحة ادناه. يقوم clients بانهاء الاتصال اذا كانت اي قدرة الزامية (PQ KEM/التوقيع او الدور او الاصدار) مفقودة او غير متطابقة مع مدخل الدليل.

5. **تسجيل النص** - تسجل relays تجزئة النص وبصمة TLS ومحتوى TLV لتغذية كواشف downgrade ومسارات الامتثال.

## Capability TLVs (SNNet-1c)

تعيد القدرات استخدام غلاف TLV ثابت من `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

الانواع المعرفة حاليا:

- `snnet.pqkem` - مستوى Kyber (`kyber768` للطرح الحالي).
- `snnet.pqsig` - مجموعة توقيع PQ (`ml-dsa-44`).
- `snnet.role` - دور relay (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - معرف اصدار البروتوكول.
- `snnet.grease` - عناصر حشو عشوائية ضمن النطاق المحجوز لضمان تقبل TLVs المستقبلية.

يحافظ clients على allow-list ل TLVs المطلوبة ويفشلون المصافحة عند الاغفال او التخفيض. تنشر relays نفس المجموعة في microdescriptor الدليل لتبقى المصادقة حتمية.

## تدوير salts و CID blinding (SNNet-1b)

- تنشر governance سجلا `SaltRotationScheduleV1` بقيم `(epoch_id, salt, valid_after, valid_until)`. تقوم relays و gateways بجلب الجدول الموقع من directory publisher.
- يطبق clients الـ salt الجديد عند `valid_after` ويحتفظون بالـ salt السابق لفترة سماح 12 ساعة ويحفظون تاريخا من 7 epochs لتحمل التحديثات المتاخرة.
- المعرفات المطموسة القياسية تستخدم:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  تقبل gateways المفتاح المطموس عبر `Sora-Req-Blinded-CID` وتعيده في `Sora-Content-CID`. تعمية الدائرة/الطلب (`CircuitBlindingKey::derive`) موجودة في `iroha_crypto::soranet::blinding`.
- اذا فات relay حقبة، يوقف الدوائر الجديدة حتى ينزل الجدول ويصدر `SaltRecoveryEventV1`، وتتعامل لوحات on-call معه كاشارة paging.

## بيانات الدليل وسياسة guards

- تحتوي microdescriptors على هوية relay (Ed25519 + ML-DSA-65) ومفاتيح PQ و TLVs القدرات ووسوم المناطق واهلية guard والحقبة الحالية للـ salt المعلن عنها.
- يقوم clients بتثبيت مجموعات guard لمدة 30 يوما ويخزنون كاشات `guard_set` بجانب snapshot الموقع للدليل. تقوم واجهات CLI و SDK باظهار بصمة الكاش لتمكين ارفاق ادلة الطرح بسجلات التغيير.

## Telemetry وقائمة تحقق الطرح

- المقاييس التي يجب تصديرها قبل الانتاج:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- عتبات التنبيه موجودة بجانب مصفوفة SLO الخاصة ب SOP تدوير salts (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) ويجب عكسها في Alertmanager قبل ترقية الشبكة.
- التنبيهات: معدل فشل >5% خلال 5 دقائق، salt lag >15 دقيقة، او رصد mismatches في القدرات بالانتاج.
- خطوات الطرح:
  1. تشغيل اختبارات توافق relay/client على staging مع تفعيل المصافحة الهجينة وحزمة PQ.
  2. اجراء بروفة SOP تدوير salts (`docs/source/soranet_salt_plan.md`) وضم artefacts التمرين الى سجل التغيير.
  3. تفعيل تفاوض القدرات في الدليل، ثم النشر الى relays الدخول والوسط والخروج واخيرا clients.
  4. تسجيل بصمات guard cache والجداول الخاصة بالـ salt ولوحات telemetry لكل مرحلة؛ وارفاق حزمة الادلة الى `status.md`.

اتباع قائمة التحقق هذه يسمح لفرق المشغلين والـ clients والـ SDK بتبني نقل SoraNet بتناغم مع الالتزام بالحتمية ومتطلبات التدقيق الملتقطة في خارطة طريق SNNet.
