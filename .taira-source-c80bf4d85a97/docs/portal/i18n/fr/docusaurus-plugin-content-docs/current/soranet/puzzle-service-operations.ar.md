---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : puzzle-service-opérations
titre : دليل عمليات خدمة الالغاز
sidebar_label : عمليات خدمة الالغاز
description : Démon `soranet-puzzle-service` pour la mission Argon2/ML-DSA.
---

:::note المصدر القياسي
C'est `docs/source/soranet/puzzle_service_operations.md`. حافظ على النسختين متزامنتين حتى يتم تقاعد الوثائق القديمة.
:::

# دليل عمليات خدمة الالغاز

Le démon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) est disponible
billets d'entrée pour Argon2 et politique de sécurité pour le relais `pow.puzzle.*`
Vous pouvez utiliser les jetons d'admission ML-DSA comme les relais de bord.
يعرض خمس نقاط نهاية HTTP :

- `GET /healthz` - فحص vivacité.
- `GET /v1/puzzle/config` - يعيد معلمات PoW/puzzle الفعلية المسحوبة
  JSON utilise le relais (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - pour Argon2 ; corps JSON version
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  Il s'agit d'un TTL (politique de confidentialité) et d'un hachage de transcription.
  ويعيد تذكرة موقعة من relais + بصمة التوقيع عندما تكون مفاتيح التوقيع مهيئة.
- `GET /v1/token/config` - `pow.token.enabled = true` est disponible
  jeton d'admission (empreinte digitale de l'émetteur, TTL/clock-skew, ID de relais,
  ومجموعة révocation المدمجة).
- `POST /v1/token/mint` - Jeton d'admission ML-DSA utilisé pour le hachage de reprise
  corps الطلب يقبل `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

تتم عملية التحقق من التذاكر المنتجة عبر خدمة الاختبار التكاملية
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, et vous êtes ici
les papillons sont utilisés pour le relais et les systèmes DoS sont utilisés.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## تهيئة اصدار التوكنات

Utiliser le relais JSON pour le relais `pow.token.*` (
`tools/soranet-relay/deploy/config/relay.entry.json` (voir) pour ML-DSA.
على الاقل قدم المفتاح العام للـ émetteur et révocation اختيارية:

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

خدمة الالغاز تعيد استخدام هذه القيم وتقوم باعادة تحميل ملف révocation بصيغة
Norito JSON pour le runtime. Comment utiliser CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) لاصدار التوكنات
واستعراضها hors ligne, واضافة `token_id_hex` pour la révocation, وتدقيق
Les mises à jour des mises à jour sont disponibles.

Les indicateurs CLI de l'émetteur sont les suivants :

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

يتوفر ايضا `--token-secret-hex` عندما يكون السر مدارا عبر pipeline خارجية. يقوم
watcher لملف révocation بالحفاظ على `/v1/token/config` محدثا؛ نسق التحديثات مع
امر `soranet-admission-token revoke` لتجنب تاخر حالة révocation.Utiliser `pow.signed_ticket_public_key_hex` pour le relais JSON pour le relais
العام ML-DSA-44 المستخدم للتحقق من PoW tickets الموقعة ; عكس `/v1/puzzle/config`
مفتاح وبصمته BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) pour les clients
من تثبيت vérificateur. يتم التحقق من تذاكر الموقعة مقابل ID de relais et liaisons de transcription
وتشارك نفس مخزن révocation؛ تظل Billets PoW à partir de 74 heures
vérificateur de billets signés. مرر secret الخاص بالموقع عبر `--signed-ticket-secret-hex`
او `--signed-ticket-secret-path` عند تشغيل خدمة الغاز ; يرفض التشغيل keypairs
غير المتطابقة اذا لم يتحقق السر من `pow.signed_ticket_public_key_hex`.
`POST /v1/puzzle/mint` pour `"signed": true` (et `"transcript_hash_hex"`) pour
ticket signé مشفر بـ Norito الى جانب octets التذكرة الخام; تتضمن الردود
`signed_ticket_b64` et `signed_ticket_fingerprint_hex` permettent de relire les empreintes digitales. يتم
رفض الطلبات مع `signed = true` اذا لم يتم تهيئة secret الموقع.

## دليل تدوير المفاتيح

1. **جمع descriptor commit الجديد.** تنشر descripteur de relais de gouvernance commit في
   ensemble de répertoires. انسخ سلسلة hex الى `handshake.descriptor_commit_hex` داخل
   Vous pouvez utiliser le relais JSON pour le faire fonctionner.
2. **مراجعة حدود سياسة الالغاز.** تاكد ان قيم
   `pow.puzzle.{memory_kib,time_cost,lanes}` المحدثة تتماشى مع خطة الاصدار. يجب
   Argon2 est doté de relais (il y a 4 MiB de plus)
   1 <= voies <= 16).
3. **تجهيز اعادة التشغيل.** اعد تحميل وحدة systemd او الحاوية عند اعلان gouvernance
   Il s'agit d'un cutover. Comment recharger à chaud اعادة التشغيل مطلوبة
   لالتقاط descripteur commit الجديد.
4. **التحقق.** اصدر تذكرة عبر `POST /v1/puzzle/mint` et `difficulty` و`expires_at`
   يطابقان السياسة الجديدة. تقرير tremper (`docs/source/soranet/reports/pow_resilience.md`)
   يلتقط حدود الكمون المتوقعة للمرجعية. عندما تكون التوكنات مفعلة، اجلب
   `/v1/token/config` pour la révocation des empreintes digitales de l'émetteur
   يطابقان القيم المتوقعة.

## اجراء التعطيل الطارئ

1. Connectez `pow.puzzle.enabled = false` au relais de connexion. احتفظ pour `pow.required = true`
   Il s'agit d'une solution de repli de hashcash.
2. اختياريا، نفذ ادخالات `pow.emergency` pour les descripteurs de la méthode Argon2
   hors ligne.
3. Utilisez le relais et le relais.
4. Utilisez `soranet_handshake_pow_difficulty` pour utiliser le hashcash
   Il s'agit d'un `/v1/puzzle/config` ou d'un `puzzle = null`.

## المراقبة والتنبيهات- **SLO de latence :** Le `soranet_handshake_latency_seconds` et le P95 durent 300 ms.
  توفر offsets الخاصة باختبار tremper بيانات معايرة لthrottles الـ guard.
  【docs/source/soranet/reports/pow_resilience.md:1】
- **Pression de quota :** استخدم `soranet_guard_capacity_report.py` pour les métriques de relais
  لضبط temps de recharge لـ `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Alignement du puzzle :** يجب ان يطابق `soranet_handshake_pow_difficulty` الصعوبة
  المعادة من `/v1/puzzle/config`. يشير الاختلاف الى تكوين relais قديم او فشل اعادة
  التشغيل.
- **Préparation au jeton :** نبه اذا انخفض `/v1/token/config` et `enabled = false`
  Utilisez la méthode `revocation_source` pour les horodatages. يجب على
  Révocation de la révocation par Norito dans CLI pour la révocation
  C'est le point final.
- **Santé du service :** افحص `/healthz` et vivacité de la cadence المعتادة ونبه اذا
  Utilisez `/v1/puzzle/mint` pour HTTP 500 (pour utiliser Argon2 et Argon2).
  (RNG). Comment créer des tokens avec HTTP 4xx/5xx selon `/v1/token/mint`
  تعامل مع الاخفاقات المتكررة كحالة pagination.

## الامتثال وتسجيل التدقيق

Les relais sont utilisés pour `handshake` pour activer l'accélérateur et le temps de recharge.
Mise en relation avec le pipeline dans le cadre de la `docs/source/soranet/relay_audit_pipeline.md`
تستوعب هذه السجلات حتى تبقى تغييرات سياسة الغاز قابلة للتدقيق. عندما تكون
بوابة الالغاز مفعلة، ارشف عينات التذاكر الصادرة وsnapshot تكوين Norito مع
تذكرة déploiement للمدققين المستقبليين. يجب تتبع jetons d'admission الصادرة قبل
La demande de révocation est `token_id_hex` pour la révocation et la révocation
الغائها.