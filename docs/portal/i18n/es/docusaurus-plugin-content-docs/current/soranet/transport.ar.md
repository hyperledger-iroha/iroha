---
lang: es
direction: ltr
source: docs/portal/docs/soranet/transport.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: transporte
título: نظرة عامة على نقل SoraNet
sidebar_label: نظرة عامة على النقل
descripción: Sales de uso de SoraNet.
---

:::nota المصدر القياسي
Utilice el software SNNet-1 para `docs/source/soranet/spec.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد مجموعة الوثائق القديمة.
:::

SoraNet está buscando rangos de recuperación de SoraFS y Norito RPC y Nexus المستقبلية. برنامج النقل (عناصر خارطة الطريق **SNNet-1** و **SNNet-1a** و **SNNet-1b**) حدد مصافحة حتمية وتفاوض قدرات ما بعد الكم (PQ) وخطة لتدوير sales كي يلاحظ كل relé وcliente وpuerta de enlace نفس وضع الامان.

## الاهداف ونموذج الشبكة

- بناء دوائر بثلاث قفزات (entrada -> medio -> salida) فوق QUIC v1 حتى لا يصل peers المسيئون الى Torii مباشرة.
- تكديس مصافحة Ruido XX *híbrido* (Curve25519 + Kyber768) فوق QUIC/TLS لربط مفاتيح الجلسة بنص TLS.
- فرض TLV قدرات تعلن دعم PQ KEM/التوقيع ودور relé واصدار البروتوكول؛ واستخدام GREASE للانواع غير المعروفة كي تبقى الامتدادات المستقبلية قابلة للنشر.
- تدوير sales لمحتوى التعمية يوميا وتثبيت relés de guardia لمدة 30 يوما حتى لا يؤدي abandono في الدليل الى فك هوية clientes.
- ابقاء celdas ثابتة عند 1024 B وحقن relleno/celdas ficticias وتصدير telemetría حتمية لالتقاط محاولات downgrade بسرعة.

## مسار المصافحة (SNNet-1a)1. **Sobre QUIC/TLS** - Clientes de retransmisiones con QUIC v1 y TLS 1.3 basados ​​en Ed25519 para gobernanza CA. يزود Exportador TLS (`tls-exporter("soranet handshake", 64)`) طبقة Ruido بالبذور كي تكون النصوص غير قابلة للفصل.
2. **Híbrido de ruido XX** - سلسلة البروتوكول `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` مع prólogo = exportador TLS. تدفق الرسائل:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   يتم خلط مخرجات DH لـ Curve25519 y encapsulaciones الخاصة بـ Kyber في المفاتيح المتماثلة النهائية. فشل التفاوض على مادة PQ يجهض المصافحة بالكامل ولا يسمح باي fallback كلاسيكي فقط.

3. **Boletos y tokens de rompecabezas** - يمكن للـ relés طلب boleto اثبات عمل Argon2id قبل `ClientHello`. Las tramas de los cuadros que contienen Argon2 y los componentes de Argon2 son:

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

4. **تبادل capacidad TLV** - الحمولة النهائية في Ruido تنقل TLV القدرات الموضحة ادناه. يقوم clientes بانهاء الاتصال اذا كانت اي قدرة الزامية (PQ KEM/التوقيع او الدور او الاصدار) مفقودة او غير متطابقة مع مدخل الدليل.

5. **تسجيل النص** - تسجل relés تجزئة النص وبصمة TLS ومحتوى TLV لتغذية كواشف downgrade y مسارات الامتثال.

## TLV de capacidad (SNNet-1c)

تعيد القدرات استخدام غلاف TLV ثابت من `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

الانواع المعرفة حاليا:- `snnet.pqkem` - مستوى Kyber (`kyber768` للطرح الحالي).
- `snnet.pqsig` - مجموعة توقيع PQ (`ml-dsa-44`).
- `snnet.role` - Relé interno (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - معرف اصدار البروتوكول.
- `snnet.grease` - عناصر حشو عشوائية ضمن النطاق المحجوز لضمان تقبل TLVs المستقبلية.

Hay clientes en la lista de permitidos de los TLV y en la lista de permisos. تنشر relés نفس المجموعة في microdescriptor الدليل لتبقى المصادقة حتمية.

## Sales avanzadas y cegamiento CID (SNNet-1b)

- Gobernanza de تنشر سجلا `SaltRotationScheduleV1` بقيم `(epoch_id, salt, valid_after, valid_until)`. تقوم relés y puertas de enlace بجلب الجدول الموقع من editor de directorio.
- يطبق clientes الـ salt الجديد عند `valid_after` ويحتفظون بالـ salt السابق لفترة سماح 12 ساعة ويحفظون تاريخا من 7 épocas لتحمل التحديثات المتاخرة.
- Instrucciones de uso:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  تقبل gateways المفتاح المطموس عبر `Sora-Req-Blinded-CID` y تعيده في `Sora-Content-CID`. تعمية الدائرة/الطلب (`CircuitBlindingKey::derive`) موجودة في `iroha_crypto::soranet::blinding`.
- اذا فات relé حقبة, يوقف الدوائر الجديدة حتى ينزل الجدول ويصدر `SaltRecoveryEventV1`, وتتعامل لوحات de guardia معه كاشارة buscapersonas.

## guardias de بيانات الدليل وسياسة- تحتوي microdescriptores على هوية relé (Ed25519 + ML-DSA-65) ومفاتيح PQ y TLV القدرات ووسوم المناطق واهلية guardia والحقبة الحالية للـ sal المعلن عنها.
- يقوم clientes بتثبيت مجموعات guard لمدة 30 يوما ويخزنون كاشات `guard_set` بجانب instantánea الموقع للدليل. Los dispositivos CLI y SDK que están instalados en el dispositivo están conectados a un servidor.

## Telemetría وقائمة تحقق الطرح

- المقاييس التي يجب تصديرها قبل الانتاج:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Utilice SLO y SOP para agregar sales (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) y alertmanager.
- التنبيهات: معدل فشل >5% خلال 5 دقائق، salt lag >15 دقيقة، او رصد desajustes في القدرات بالانتاج.
- خطوات الطرح:
  1. تشغيل اختبارات توافق retransmisión/cliente على puesta en escena مع تفعيل المصافحة الهجينة وحزمة PQ.
  2. اجراء بروفة SOP تدوير sales (`docs/source/soranet_salt_plan.md`) وضم artefactos التمرين الى سجل التغيير.
  3. تفعيل تفاوض القدرات في الدليل، ثم النشر الى relés الدخول والوسط والخروج واخيرا clientes.
  4. تسجيل بصمات guard cache y الخاصة بالـ salt y telemetría لكل مرحلة؛ وارفاق حزمة الادلة الى `status.md`.

Configuración de clientes y SDK de SoraNet y software de SoraNet ومتطلبات التدقيق الملتقطة في خارطة طريق SNNet.