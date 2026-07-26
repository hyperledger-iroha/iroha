---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/transport.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: transporte
título: نظرة عامة على نقل SoraNet
sidebar_label: Nome da barra lateral
description: Sais de sal, وتوجيهات القدرات لطبقة اخفاء الهوية no SoraNet.
---

:::note المصدر القياسي
Você pode usar o SNNet-1 em `docs/source/soranet/spec.md`. Não se preocupe, você pode fazer isso sem problemas.
:::

SoraNet é capaz de obter buscas de intervalo em SoraFS e Norito RPC e Nexus المستقبلية. برنامج النقل (عناصر خارطة الطريق **SNNet-1** e **SNNet-1a** e **SNNet-1b**) بعد الكم (PQ) وخطة لتدوير sais كي يلاحظ كل relé, cliente e gateway نفس وضع الامان.

## الاهداف ونموذج الشبكة

- بناء دوائر بثلاث قفزات (entrada -> meio -> saída) QUIC v1 حتى لا يصل peers المسيئون الى Torii مباشرة.
- تكديس مصافحة Noise XX *hybrid* (Curve25519 + Kyber768) QUIC/TLS لربط مفاتيح الجلسة بنص TLS.
- Transfira TLVs para o PQ KEM/التوقيع e para o relé e para o relé. Use GREASE para limpar o óleo do motor.
- تدوير salts لمحتوى التعمية يوميا وتثبيت guard relays لمدة 30 يوما حتى لا يؤدي churn em cada cliente.
- ابقاء células ثابتة 1024 B e preenchimento/células simuladas وتصدير telemetria حتمية لالتقاط محاولات downgrade بسرعة.

## مسار المصافحة (SNNet-1a)

1. **QUIC/TLS envelope** - يتصل clientes بالـ relés عبر QUIC v1 ويكملون مصافحة TLS 1.3 باستخدام شهادات Ed25519 الموقعة من governança CA. يزود TLS exportador (`tls-exporter("soranet handshake", 64)`) طبقة Ruído بالبذور كي تكون النصوص غير قابلة للفصل.
2. **Noise XX hybrid** - سلسلة البروتوكول `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` com prólogo = exportador TLS. Descrição:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Você pode usar DH para Curve25519 e encapsulamentos no Kyber no site da empresa. فشل التفاوض على مادة PQ يجهض المصافحة بالكامل ولا يسمح باي fallback كلاسيكي فقط.

3. **Tíquetes e tokens de quebra-cabeça** - يمكن للـ relés طلب ticket اثبات عمل Argon2id قبل `ClientHello`. A configuração de frames pode ser feita por meio do Argon2 e da configuração do arquivo:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   tokens são usados para `SNTK`, o que significa que o token é ML-DSA-44. مقابل السياسة النشطة e قائمة الالغاء.

4. **capacidade de TLV** - A capacidade de ruído e o ruído afetam os TLVs do sistema. Seus clientes são responsáveis ​​por clientes e clientes (PQ KEM / التوقيع او الدور او الاصدار) مفقودة او غير متطابقة مع مدخل الدليل.

5. **تسجيل النص** - تسجل relés تجزئة النص وبصمة TLS ومحتوى TLV لتغذية كواشف downgrade ومسارات الامتثال.

## TLVs de capacidade (SNNet-1c)

Para obter mais informações sobre o TLV, use `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Mais informações:

- `snnet.pqkem` - مستوى Kyber (`kyber768` para o país).
- `snnet.pqsig` - مجموعة توقيع PQ (`ml-dsa-44`).
- `snnet.role` - Relé de corrente (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - معرف اصدار البروتوكول.
- `snnet.grease` - Você pode usar o TLVs para obter mais informações.Os clientes não estão na lista de permissões dos TLVs e dos clientes. Os relés não são usados ​​​​no microdescriptor para serem usados.

## تدوير sais e cegamento CID (SNNet-1b)

- Governança de alto nível `SaltRotationScheduleV1` ou `(epoch_id, salt, valid_after, valid_until)`. Os relés e gateways são o editor do diretório.
- يطبق clients الـ salt الجديد عند `valid_after` ويحتفظون بالـ salt السابق لفترة سماح 12 ساعة ويحفظون تاريخا Em 7 épocas, لتحمل التحديثات المتاخرة.
- المعرفات المطموسة القياسية تستخدم:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Verifique os gateways do `Sora-Req-Blinded-CID` e `Sora-Content-CID`. A unidade/rede de substituição (`CircuitBlindingKey::derive`) está localizada em `iroha_crypto::soranet::blinding`.
- اذا فات relé حقبة, يوقف الدوائر الجديدة حتى ينزل الجدول ويصدر `SaltRecoveryEventV1`, وتتعامل لوحات de plantão معه كاشارة paginação.

## بيانات الدليل وسياسة guardas

- Microdescritores importantes para relé (Ed25519 + ML-DSA-65) e PQ e TLVs, dispositivos e guarda الحالية للـ salt المعلن عنها.
- يقوم clients بتثبيت مجموعات guard لمدة 30 يوما ويخزنون كاشات `guard_set` بجانب snapshot الموقع للدليل. A CLI e o SDK podem ser usados ​​para configurar o software.

## Telemetria وقائمة تحقق الطرح

- المقاييس التي يجب تصديرها قبل الانتاج:
  -`soranet_handshake_success_total{role}`
  -`soranet_handshake_failure_total{reason}`
  -`soranet_handshake_latency_seconds`
  -`soranet_capability_mismatch_total`
  -`soranet_salt_rotation_lag_seconds`
- عتبات التنبيه موجودة بجانب مصفوفة SLO الخاصة ب SOP تدوير sais (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) ويجب عكسها في Alertmanager قبل ترقية الشبكة.
- Resultados: معدل فشل >5% خلال 5 دقائق, salt lag >15 دقيقة, او رصد incompatibilidades في القدرات بالانتاج.
- خطوات الطرح:
  1. Configure o relé/cliente para o staging e o PQ.
  2. Verifique os sais de sal SOP (`docs/source/soranet_salt_plan.md`) e os artefatos do produto.
  3. Verifique todos os relés do cliente, entre em contato conosco e envie-os para os clientes.
  4. تسجيل بصمات guard cache والجداول الخاصة بالـ salt ولوحات telemetria لكل مرحلة؛ وارفاق حزمة الادلة `status.md`.

Faça o download do software para clientes e clientes e SDK sem SoraNet para usar o SoraNet بالحتمية ومتطلبات التدقيق الملتقطة na rede SNNet.