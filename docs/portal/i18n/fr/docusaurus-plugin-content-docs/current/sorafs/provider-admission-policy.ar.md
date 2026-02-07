---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> مقتبس من [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# سياسة قبول وهوية مزودي SoraFS (مسودة SF-2b)

توثق هذه المذكرة المخرجات العملية لـ **SF-2b**: تعريف مسار القبول، ومتطلبات الهوية، وحمولات
Il s'agit d'un produit SoraFS. Et vous avez besoin d'aide pour RFC
معمارية SoraFS وتجزئ العمل المتبقي إلى مهام هندسية قابلة للتتبع.

## أهداف السياسة

- ضمان أن المشغلين المُدقَّقين فقط يمكنهم نشر سجلات `ProviderAdvertV1` التي تقبلها الشبكة.
- ربط كل مفتاح إعلان بوثيقة هوية معتمدة من الحوكمة، ونقاط نهاية مستوثقة، ومساهمة حد أدنى من الـ pieu.
- توفير أدوات تحقق حتمية لكي يطبق Torii والبوابات و`‎sorafs-node` نفس الفحوصات.
- دعم التجديد وإلغاء الطوارئ دون كسر الحتمية أو ergonomique الأدوات.

## متطلبات الهوية والـ mise

| المتطلب | الوصف | المخرج |
|---------|-------|--------|
| مصدر مفتاح الإعلان | يجب أن يسجل المزودون زوج مفاتيح Ed25519 يوقع كل annonce. تقوم حزمة القبول بتخزين المفتاح العام مع توقيع الحوكمة. | Utilisez `ProviderAdmissionProposalV1` pour `advert_key` (32 pièces) et utilisez-le pour (`sorafs_manifest::provider_admission`). |
| مؤشر participation | يتطلب القبول `StakePointer` غير صفري يشير إلى مجمع jalonnement نشط. | إضافة التحقق في `sorafs_manifest::provider_advert::StakePointer::validate()` وإظهار الأخطاء في CLI/اختبارات. |
| وسوم الاختصاص القضائي | يعلن المزودون الاختصاص + جهة اتصال قانونية. | Vous pouvez utiliser `jurisdiction_code` (ISO 3166-1 alpha-2) et `contact_uri`. |
| استيثاق نقطة النهاية | Il s'agit d'une solution à base de mTLS et QUIC. | تعريف حمولة Norito `EndpointAttestationV1` وتخزينها لكل نقطة نهاية داخل حزمة القبول. |

## سير عمل القبول

1. **إنشاء المقترح**
   - CLI : إضافة `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     Pour `ProviderAdmissionProposalV1` + حزمة الاستيثاق.
   - Titre : Vous avez une mise en jeu > 0 et un chunker pour `profile_id`.
2. **اعتماد الحوكمة**
   - يوقع المجلس `blake3("sorafs-provider-admission-v1" || canonical_bytes)` pour enveloppe d'enveloppe
     (et `sorafs_manifest::governance`).
   - يتم حفظ الـ enveloppe في `governance/providers/<provider_id>/admission.json`.
3. **إدخال السجل**
   - تنفيذ مدقق مشترك (`sorafs_manifest::provider_admission::validate_envelope`) et Torii/البوابات/CLI.
   - تحديث مسار القبول في Torii لرفض adverts التي يختلف digest أو تاريخ الانتهاء فيها عن الـ enveloppe.
4. **التجديد والإلغاء**
   - إضافة `ProviderAdmissionRenewalV1` مع تحديثات اختيارية لنقاط النهاية/الـ participation.
   - إتاحة مسار CLI `--revoke` يسجل سبب الإلغاء ويدفع حدث حوكمة.

## مهام التنفيذ

| المجال | المهمة | Propriétaire(s) | الحالة |
|--------|-------|----------|--------|
| المخطط | عريف `ProviderAdmissionProposalV1` و`ProviderAdmissionEnvelopeV1` و`EndpointAttestationV1` (Norito) ضمن `crates/sorafs_manifest/src/provider_admission.rs`. مُنفذ داخل `sorafs_manifest::provider_admission` مع مساعدات تحقق.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Stockage / Gouvernance | ✅ مكتمل |
| أدوات CLI | Utiliser `sorafs_manifest_stub` pour les modèles : `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | GT Outillage | ✅ |مسار CLI يدعم الآن حزم الشهادات الوسيطة (`--endpoint-attestation-intermediate`) et octets pour l'enveloppe/enveloppe Utilisez `sign`/`verify`. يمكن للمشغلين توفير أجسام annonces مباشرة أو إعادة استخدام annonces موقعة، ويمكن تمرير ملفات التواقيع عبر Il s'agit de `--council-signature-public-key` et `--council-signature-file`.

### CLI

Il s'agit de `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - Nom de l'utilisateur : `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, et `--endpoint=<kind:host>` et `--endpoint=<kind:host>`.
  - استيثاق كل نقطة نهاية يتطلب `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, وشهادة عبر
    `--endpoint-attestation-leaf=<path>` (pour `--endpoint-attestation-intermediate=<path>` pour la connexion avec l'appareil photo) et pour l'ALPN
    (`--endpoint-attestation-alpn=<token>`). يمكن لنقاط نهاية QUIC توفير تقارير النقل عبر
    `--endpoint-attestation-report[-hex]=...`.
  - Paramètres : octets pour Norito (`--proposal-out`) et JSON
    (stdout est disponible sur `--json-out`).
-`sign`
  - Éléments : élément (`--proposal`) et annonce (`--advert`) et élément d'annonce
    (`--advert-body`), époque احتفاظ، وتوقيع مجلس واحد على الأقل. يمكن تمرير التواقيع
    inline (`--council-signature=<signer_hex:signature_hex>`) pour les utilisateurs
    `--council-signature-public-key` ou `--council-signature-file=<path>`.
  - ينتج enveloppe مُحقق (`--envelope-out`) et JSON يوضح روابط digest et عدد الموقّعين ومسارات الإدخال.
-`verify`
  - Il s'agit d'une enveloppe (`--envelope`) qui est en contact avec une annonce ou une annonce. Vous pouvez utiliser JSON pour digérer et ajouter des objets et des artefacts.
-`renewal`
  - يربط enveloppe مُعتمد جديد بالـ digest الذي تم التصديق عليه سابقا. يتطلب
    `--previous-envelope=<path>` et `--envelope=<path>` التالي (كلاهما حمولة Norito).
    La CLI contient des alias de profil, des publicités et des métadonnées. ينتج bytes قياسية
    `ProviderAdmissionRenewalV1` (`--renewal-out`) est compatible avec JSON.
-`revoke`
  - يصدر حزمة طوارئ `ProviderAdmissionRevocationV1` لمزود يجب سحب الخاص به. يتطلب `--envelope=<path>`, `--reason=<text>`, توقيع مجلس واحد على الأقل
    `--council-signature`, و`--revoked-at`/`--notes`. La CLI est une version digest de Norito et une version `--revocation-out` d'un JSON plus digest.
| التحقق | Utilisez le code Torii et `‎sorafs-node`. توفير اختبارات وحدة + تكامل CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Mise en réseau TL / Stockage | ✅ مكتمل |
| Télécharger Torii | تمرير المدقق في إدخال adverts في Torii ورفض خارج السياسة وإصدار التليمترية. | Réseautage TL | ✅ مكتمل | يقوم Torii enveloppes pour enveloppes (`torii.sorafs.admission_envelopes_dir`) et pour digest/التوقيع أثناء الإدخال وإبراز تليمترية القبول.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| التجديد | إضافة مخطط التجديد/الإلغاء + مساعدات CLI et دورة الحياة في الوثائق (راجع الـ runbook أدناه وأوامر CLI Dans `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Stockage / Gouvernance | ✅ مكتمل || التليمترية | تعريف لوحات/تنبيهات `provider_admission` (تجديد مفقود، انتهاء). | Observabilité | 🟠جار | Contact `torii_sorafs_admission_total{result,reason}` لوحات/تنبيهات قيد الانتظار.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### دليل التجديد والإلغاء

#### تجديد مجدول (تحديثات pieu/topologie)
1. أنشئ زوج المقترح/advert اللاحق باستخدام `provider-admission proposal` et `provider-admission sign`, مع زيادة `--retention-epoch` et enjeux/points de terminaison حسب الحاجة.
2. نفذ
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   يقوم الأمر بالتحقق من ثبات حقول القدرة/الملف عبر `AdmissionRecord::apply_renewal`, ويصدر `ProviderAdmissionRenewalV1` et digests لسجل Fichier.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Ajouter l'enveloppe à `torii.sorafs.admission_envelopes_dir` et utiliser Norito/JSON pour utiliser le hachage + rétention époque إلى `docs/source/sorafs/migration_ledger.md`.
4. Mettre l'enveloppe en place avec l'enveloppe `torii_sorafs_admission_total{result="accepted",reason="stored"}`.
5. أعد توليد وتثبيت luminaires القياسية عبر `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`؛ CI (`ci/check_sorafs_fixtures.sh`) est compatible avec Norito.

#### إلغاء طارئ
1. حدد الـ enveloppe المخترق واصدر إلغاء:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   يوقع CLI `ProviderAdmissionRevocationV1`, ويتحقق من مجموعة التواقيع عبر `verify_revocation_signatures`, ويبلغ digest Fichier.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Utilisez l'enveloppe avec `torii.sorafs.admission_envelopes_dir` et Norito/JSON pour mettre en cache les caches et le hachage.
3. راقب `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` pour les caches publicitaires واحتفظ بآثار الإلغاء في مراجعات الحوادث.

## الاختبارات والتليمترية

- إضافة luminaires ذهبية لمقترحات وأغلفة القبول تحت
  `fixtures/sorafs_manifest/provider_admission/`.
- توسيع CI (`ci/check_sorafs_fixtures.sh`) pour les enveloppes et les enveloppes.
- تتضمن luminaires المولدة `metadata.json` مع digests قياسية؛ تؤكد اختبارات en aval أن
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- توفير اختبارات تكامل:
  - يرفض Torii adverts ذات enveloppes قبول مفقودة أو منتهية الصلاحية.
  - يقوم CLI بعملية aller-retour للمقترح → enveloppe → التحقق.
  - يقوم تجديد الحوكمة بتدوير استيثاق endpoint دون تغيير معرف المزود.
- متطلبات التليمترية:
  - Remplacez les `provider_admission_envelope_{accepted,rejected}` par Torii. ✅ `torii_sorafs_admission_total{result,reason}` يعرض نتائج القبول/الرفض.
  - إضافة تحذيرات انتهاء الصلاحية إلى لوحات المراقبة (تجديد مستحق خلال 7 أيام).

## الخطوات التالية

1. ✅ Utilisez le code Norito et le code `sorafs_manifest::provider_admission`. لا حاجة لميزات.
2. ✅ تم توثيق مسارات CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) et اختبارات التكامل؛ Vous devez utiliser le runbook.
3. ✅ يقوم Torii admission/découverte sous enveloppes ويعرض عدادات التليمترية للقبول/الرفض.
4. التركيز على الملاحظة: إنهاء لوحات/تنبيهات القبول بحيث تطلق تنبيهات عند قرب الاستحقاق خلال سبعة أيام (`torii_sorafs_admission_total`, jauges de péremption).