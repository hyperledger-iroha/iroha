---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/transport.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : transport
titre : نظرة عامة على نقل SoraNet
sidebar_label : نظرة عامة على النقل
description: Les sels de sels et les sels de SoraNet.
---

:::note المصدر القياسي
Vous avez besoin de SNNet-1 pour `docs/source/soranet/spec.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد مجموعة الوثائق القديمة.
:::

SoraNet est un outil de récupération de plage pour SoraFS et Norito RPC et Nexus. المستقبلية. La version (**SNNet-1** et **SNNet-1a** et **SNNet-1b**) est en cours de mise à jour. (PQ) وخطة لتدوير salts كي يلاحظ كل relay وclient وgateway نفس وضع الامان.

## الاهداف ونموذج الشبكة

- بناء دوائر بثلاث قفزات (entrée -> milieu -> sortie) pour QUIC v1 حتى لا يصل peers المسيئون الى Torii مباشرة.
- Fonctionne avec Noise XX *hybrid* (Curve25519 + Kyber768) et QUIC/TLS comme compatible avec TLS.
- TLVs قدرات تعلن دعم PQ KEM/التوقيع ودور relay واصدار البروتوكول؛ واستخدام GREASE للانواع غير المعروفة كي تبقى الامتدادات المستقبلية قابلة للنشر.
- تدوير salts لمحتوى التعمية يوميا وتثبيت guard relays لمدة 30 يوما حتى لا يؤدي churn في الدليل الى فك هوية clients.
- Les cellules sont utilisées comme 1024 B et le rembourrage/cellules factices et la télémétrie sont également utilisées pour rétrograder.

## مسار المصافحة (SNNet-1a)

1. **Enveloppe QUIC/TLS** - Clients et relais pour QUIC v1 et TLS 1.3 pour la gouvernance CA Ed25519. Utilisez l'exportateur TLS (`tls-exporter("soranet handshake", 64)`) pour Noise à l'aide de l'exportateur TLS.
2. **Noise XX hybrid** - سلسلة البروتوكول `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` avec prologue = exportateur TLS. تدفق الرسائل:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Il s'agit d'une application DH pour Curve25519 et d'encapsulations pour Kyber dans l'environnement de travail. فشل التفاوض على مادة PQ يجهض المصافحة بالكامل ولا يسمح باي fallback كلاسيكي فقط.

3. **Billets de puzzle et jetons** - يمكن للـ relays طلب ticket اثبات عمل Argon2id قبل `ClientHello`. Les frames sont également compatibles avec Argon2 et les paramètres suivants :

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   tokens القبول المسبوقة بـ `SNTK` تتجاوز الغاز عندما تتحقق صحة توقيع ML-DSA-44 من المُصدر مقابل السياسة النشطة وقائمة الالغاء.

4. **Capacité d'exploitation TLV** - TLV pour le bruit et les TLV pour l'environnement. Clients en ligne مع مدخل الدليل.

5. **Transmission** - Relais de connexion pour connexion TLS et TLV pour rétrogradation et déclassement.

## TLV de capacité (SNNet-1c)

Utilisez le code TLV pour `typ/length/value` :

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

الانواع المعرفة حاليا:

- `snnet.pqkem` - pour Kyber (`kyber768` pour Kyber).
- `snnet.pqsig` - مجموعة توقيع PQ (`ml-dsa-44`).
- `snnet.role` - relais (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - معرف اصدار البروتوكول.
- `snnet.grease` - عناصر حشو عشوائية ضمن النطاق المحجوز لضمان تقبل TLV المستقبلية.Les clients sont autorisés sur la liste d'autorisation pour les TLV et les clients. تنشر relais نفس المجموعة في microdescriptor الدليل لتبقى المصادقة حتمية.

## Sels de culture et aveuglement CID (SNNet-1b)

- تنشر gouvernance سجلا `SaltRotationScheduleV1` par `(epoch_id, salt, valid_after, valid_until)`. Il s'agit de relais et de passerelles pour l'éditeur d'annuaire.
- يطبق clients الـ salt الجديد عند `valid_after` ويحتفظون بالـ salt السابق لفترة سماح 12 ساعة ويحفظون تاريخا من 7 époques لتحمل التحديثات المتاخرة.
- المعرفات المطموسة القياسية تستخدم:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Les passerelles sont compatibles avec `Sora-Req-Blinded-CID` et `Sora-Content-CID`. تعمية الدائرة/الطلب (`CircuitBlindingKey::derive`) موجودة في `iroha_crypto::soranet::blinding`.
- Relais pour relais de garde `SaltRecoveryEventV1`, pour les gardes de garde كاشارة pagination.

## بيانات الدليل وسياسة gardes

- Microdescripteurs et relais (Ed25519 + ML-DSA-65) et PQ et TLV et garde et garde. sel المعلن عنها.
- يقوم clients بتثبيت مجموعات guard لمدة 30 يوما ويخزنون كاشات `guard_set` بجانب snapshot الموقع للدليل. La CLI et le SDK sont également compatibles avec les fonctionnalités de mise à jour.

## Télémétrie وقائمة تحقق الطرح

- المقاييس التي يجب تصديرها قبل الانتاج:
  -`soranet_handshake_success_total{role}`
  -`soranet_handshake_failure_total{reason}`
  -`soranet_handshake_latency_seconds`
  -`soranet_capability_mismatch_total`
  -`soranet_salt_rotation_lag_seconds`
- Prise en charge des sels SLO pour SOP (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) et Alertmanager الشبكة.
- Paramètres : معدل فشل >5% خلال 5 دقائق، salt lag >15 دقيقة، او رصد في القدرات بالانتاج.
- خطوات الطرح :
  1. Utiliser le relais/client pour la mise en scène et le PQ.
  2. اجراء بروفة SOP تدوير sels (`docs/source/soranet_salt_plan.md`) et artefacts التمرين الى سجل التغيير.
  3. Les services de relais des clients et des clients.
  4. Utiliser le cache de garde et la télémétrie pour le sel et la télémétrie وارفاق حزمة الادلة الى `status.md`.

Vous pouvez également utiliser le SDK pour SoraNet avec les clients et les SDK pour SoraNet. Vous êtes connecté à SNNet.