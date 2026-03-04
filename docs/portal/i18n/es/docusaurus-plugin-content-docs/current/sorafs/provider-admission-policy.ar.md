---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> مقتبس من [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# سياسة قبول وهوية مزودي SoraFS (módulo SF-2b)

توثق هذه المذكرة المخرجات العملية لـ **SF-2b**: تعريف مسار القبول، ومتطلبات الهوية، وحمولات
Lea la información de la unidad SoraFS y. Y توسع العملية عالية المستوى الموضحة في RFC
معمارية SoraFS وتجزئ العمل المتبقي إلى مهام هندسية قابلة للتتبع.

## أهداف السياسة

- ضمان أن المشغلين المُدقَّقين فقط يمكنهم نشر سجلات `ProviderAdvertV1` التي تقبلها الشبكة.
- ربط كل مفتاح إعلان بوثيقة هوية معتمدة من الحوكمة، ونقاط نهاية مستوثقة، ومساهمة حد أدنى من الـ estaca.
- توفير أدوات تحقق حتمية لكي يطبق Torii والبوابات و`‎sorafs-node` نفس الفحوصات.
- دعم التجديد وإلغاء الطوارئ دون كسر الحتمية أو ergonómico الأدوات.

## متطلبات الهوية والـ participación| المتطلب | الوصف | المخرج |
|---------|-------|--------|
| مصدر مفتاح الإعلان | يجب أن يسجل المزودون زوج مفاتيح Ed25519 يوقع كل anuncio. تقوم حزمة القبول بتخزين المفتاح العام مع توقيع الحوكمة. | Coloque el cable `ProviderAdmissionProposalV1` o el `advert_key` (32 bits) y el cable (`sorafs_manifest::provider_admission`). |
| participación de مؤشر | يتطلب القبول `StakePointer` غير صفري يشير إلى مجمع estacando نشط. | Conecte el `sorafs_manifest::provider_advert::StakePointer::validate()` y conecte el CLI/الاختبارات. |
| Artículos y artículos relacionados | يعلن المزودون الاختصاص + جهة اتصال قانونية. | Utilice `jurisdiction_code` (ISO 3166-1 alfa-2) e `contact_uri`. |
| استيثاق نقطة النهاية | يجب أن تكون كل نقطة نهاية مُعلن عنها مدعومة بتقرير شهادة mTLS أو QUIC. | تعريف حمولة Norito `EndpointAttestationV1` وتخزينها لكل نقطة نهاية داخل حزمة القبول. |

## سير عمل القبول1. **إنشاء المقترح**
   - CLI: Número `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     لإنتاج `ProviderAdmissionProposalV1` + حزمة الاستيثاق.
   - التحقق: ضمان الحقول المطلوبة, وstake > 0, ومقبض fragment قياسي في `profile_id`.
2. **اعتماد الحوكمة**
   - يوقع المجلس `blake3("sorafs-provider-admission-v1" || canonical_bytes)` باستخدام أدوات sobre الحالية
     (Más información `sorafs_manifest::governance`).
   - يتم حفظ الـ sobre في `governance/providers/<provider_id>/admission.json`.
3. **إدخال السجل**
   - Utilice el software (`sorafs_manifest::provider_admission::validate_envelope`) para acceder a Torii/البوابات/CLI.
   - تحديث مسار القبول في Torii لرفض ads التي يختلف digest أو تاريخ الانتهاء فيها عن الـ sobre.
4. **التجديد yالإلغاء**
   - إضافة `ProviderAdmissionRenewalV1` مع تحديثات اختيارية لنقاط النهاية/الـ estaca.
   - Utilice CLI `--revoke` para conectar el dispositivo y el dispositivo.

## مهام التنفيذ

| المجال | المهمة | Propietario(s) | الحالة |
|--------|-------|----------|--------|
| المخطط | Utilice `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1` y `EndpointAttestationV1` (Norito) o `crates/sorafs_manifest/src/provider_admission.rs`. مُنفذ داخل `sorafs_manifest::provider_admission` مع مساعدات تحقق.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Almacenamiento / Gobernanza | ✅ مكتمل |
| أدوات CLI | Nombres de usuario `sorafs_manifest_stub`: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Grupo de Trabajo sobre Herramientas | ✅ |Configuración de CLI para la configuración de archivos (`--endpoint-attestation-intermediate`) y bytes de configuración/sobre y configuración de sobres أثناء `sign`/`verify`. يمكن للمشغلين توفير أجسام anuncios مباشرة أو إعادة استخدام anuncios موقعة، ويمكن تمرير ملفات التواقيع عبر الجمع Aquí están `--council-signature-public-key` e `--council-signature-file`.

### مرجع CLI

نفّذ كل أمر عبر `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.- `proposal`
  - Nombre del producto: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, y `--endpoint=<kind:host>` y.
  - استيثاق كل نقطة نهاية يتطلب `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>` ، وشهادة عبر
    `--endpoint-attestation-leaf=<path>` (مع `--endpoint-attestation-intermediate=<path>` اختياري لكل عنصر في السلسلة) y معرفات ALPN متفاوض عليها
    (`--endpoint-attestation-alpn=<token>`). يمكن لنقاط نهاية QUIC توفير تقارير النقل عبر
    `--endpoint-attestation-report[-hex]=...`.
  - Contenido: bytes de configuración Norito (`--proposal-out`) y JSON
    (salida estándar según `--json-out`).
- `sign`
  - المدخلات: مقترح (`--proposal`), anuncio موقع (`--advert`), ، جسم anuncio اختياري
    (`--advert-body`) ، época احتفاظ، وتوقيع مجلس واحد على الأقل. يمكن تمرير التواقيع
    en línea (`--council-signature=<signer_hex:signature_hex>`) أو عبر ملفات باستخدام
    `--council-signature-public-key` a `--council-signature-file=<path>`.
  - ينتج sobre مُحقق (`--envelope-out`) وتقرير JSON يوضح روابط digest وعدد الموقّعين ومسارات الإدخال.
-`verify`
  - يتحقق من sobre موجود (`--envelope`) مع فحص اختياري للمقترح المطابق أو anuncio أو جسم anuncio. يسلط تقرير JSON الضوء على قيم digest وحالة تحقق التواقيع وأي artefactos اختيارية مطابقة.
- `renewal`
  - يربط sobre مُعتمد جديد بالـ resumen الذي تم التصديق عليه سابقا. يتطلب
    `--previous-envelope=<path>` y `--envelope=<path>` التالي (كلاهما حمولة Norito).يتحقق CLI من بقاء alias de perfil والقدرات ومفاتيح anuncio دون تغيير، مع السماح بتحديثات estaca ونقاط النهاية y metadatos. ينتج bytes قياسية
    `ProviderAdmissionRenewalV1` (`--renewal-out`) Establece un archivo JSON.
- `revoke`
  - يصدر حزمة طوارئ `ProviderAdmissionRevocationV1` لمزود يجب سحب sobre الخاص به. يتطلب `--envelope=<path>`, `--reason=<text>`, توقيع مجلس yاحد على الأقل
    `--council-signature` y `--revoked-at`/`--notes`. Utilice CLI y resúmenes de archivos Norito o `--revocation-out` y ​​JSON y resúmenes.
| التحقق | Utilice los dispositivos Torii y `‎sorafs-node`. توفير اختبارات وحدة + تكامل CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Redes TL / Almacenamiento | ✅ مكتمل |
| Actualización Torii | تمرير المدقق في إدخال anuncios في Torii ورفض anuncios خارج السياسة وإصدار التليمترية. | Redes TL | ✅ مكتمل | يقوم Torii الآن بتحميل sobres الحوكمة (`torii.sorafs.admission_envelopes_dir`) والتحقق من تطابق digest/التوقيع أثناء الإدخال وإبراز 【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 || التجديد | Haga clic en la CLI y haga clic en la CLI y haga clic en la CLI para ejecutarla. `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Almacenamiento / Gobernanza | ✅ مكتمل |
| التليمترية | تعريف لوحات/تنبيهات `provider_admission` (تجديد مفقود، انتهاء sobre). | Observabilidad | 🟠 جار | Más información `torii_sorafs_admission_total{result,reason}` 【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### دليل التجديد والإلغاء

#### تجديد مجدول (تحديثات estaca/topología)
1. أنشئ زوج المقترح/anuncio اللاحق باستخدام `provider-admission proposal` و `provider-admission sign`, مع زيادة `--retention-epoch` وتحديث estaca/puntos finales حسب الحاجة.
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
   يقوم الأمر بالتحقق من ثبات حقول القدرة/الملف عبر `AdmissionRecord::apply_renewal`, ويصدر `ProviderAdmissionRenewalV1` y digests لسجل الحوكمة.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Inserte el sobre en `torii.sorafs.admission_envelopes_dir`, y agregue Norito/JSON para agregar datos y hash en + época de retención. Aquí `docs/source/sorafs/migration_ledger.md`.
4. أخطر المشغلين بأن الـ sobre الجديد أصبح نشطا وراقب `torii_sorafs_admission_total{result="accepted",reason="stored"}` لتأكيد الإدخال.
5. أعد توليد وتثبيت accesorios القياسية عبر `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`؛ CI (`ci/check_sorafs_fixtures.sh`) está conectado a la interfaz Norito.#### إلغاء طارئ
1. حدد الـ sobre المخترق واصدر إلغاء:
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
   Haga clic en CLI `ProviderAdmissionRevocationV1` y en el resumen de `verify_revocation_signatures`. الإلغاء.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Utilice el sobre `torii.sorafs.admission_envelopes_dir` y Norito/JSON para almacenar cachés y hash para obtener el resultado deseado.
3. راقب `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` لتأكيد أن cachés تُسقط anuncio الملغى؛ واحتفظ بآثار الإلغاء في مراجعات الحوادث.

## الاختبارات والتليمترية

- إضافة accesorios ذهبية لمقترحات وأغلفة القبول تحت
  `fixtures/sorafs_manifest/provider_admission/`.
- توسيع CI (`ci/check_sorafs_fixtures.sh`) لإعادة توليد المقترحات والتحقق من الـ sobres.
- تضمن accesorios المولدة `metadata.json` مع resúmenes قياسية؛ تؤكد اختبارات aguas abajo أن
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- توفير اختبارات تكامل:
  - يرفض Torii anuncios ذات sobres قبول مفقودة أو منتهية الصلاحية.
  - يقوم CLI بعملية ida y vuelta للمقترح → sobre → التحقق.
  - يقوم تجديد الحوكمة بتدوير استيثاق endpoint دون تغيير معرف المزود.
- متطلبات التليمترية:
  - إصدار عدادات `provider_admission_envelope_{accepted,rejected}` a Torii. ✅ `torii_sorafs_admission_total{result,reason}` يعرض نتائج القبول/الرفض.
  - إضافة تحذيرات انتهاء الصلاحية إلى لوحات المراقبة (تجديد مستحق خلال 7 أيام).

## الخطوات التالية1. ✅ تم إنهاء تغييرات مخطط Norito and مساعدات التحقق في `sorafs_manifest::provider_admission`. لا حاجة لميزات.
2. ✅ Abra la CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) y haga clic en اختبارات التكامل؛ Utilice el runbook para crear aplicaciones.
3. ✅ يقوم Torii admisión/descubrimiento بإدخال الـ sobres ويعرض عدادات التليمترية للقبول/الرفض.
4. التركيز على الملاحظة: إنهاء لوحات/تنبيهات القبول بحيث تطلق تنبيهات عند قرب الاستحقاق خلال سبعة أيام (`torii_sorafs_admission_total`, indicadores de caducidad).