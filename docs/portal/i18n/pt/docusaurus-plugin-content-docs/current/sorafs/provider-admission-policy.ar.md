---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> مقتبس من [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Você pode usar o SoraFS (nome SF-2b)

توثق هذه المذكرة المخرجات العملية لـ **SF-2b**: تعريف مسار القبول, ومتطلبات الهوية, وحمولات
A solução de problemas é SoraFS. Isso é importante para o RFC
A chave SoraFS pode ser usada para remover o problema.

## أهداف السياسة

- ضمان أن المشغلين المُدقَّقين فقط يمكنهم نشر سجلات `ProviderAdvertV1` التي تقبلها الشبكة.
- ربط كل مفتاح إعلان بوثيقة هوية معتمدة من الحوكمة, ونقاط نهاية مستوثقة, ومساهمة حد أدنى من الـ estaca.
- Verifique se o Torii e o `‎sorafs-node` estão danificados.
- دعم التجديد وإلغاء الطوارئ دون كسر الحتمية, e ergonomia الأدوات.

## متطلبات الهوية والـ participação

| المتطلب | الوصف | المخرج |
|--------|-------|--------|
| مصدر مفتاح الإعلان | يجب أن يسجل المزودون زوج مفاتيح Ed25519 não está no anúncio. تقوم حزمة القبول بتخزين المفتاح العام مع توقيع الحوكمة. | Você pode usar `ProviderAdmissionProposalV1` para `advert_key` (32 páginas) e usar o mesmo dispositivo (`sorafs_manifest::provider_admission`). |
| Participação de مؤشر | Use o `StakePointer` para fazer o piqueteamento. | Você pode usar o `sorafs_manifest::provider_advert::StakePointer::validate()` e usar o CLI/الاختبارات. |
| وسوم الاختصاص القضائي | يعلن المزودون الاختصاص + جهة اتصال قانونية. | A solução de problemas é `jurisdiction_code` (ISO 3166-1 alpha-2) e `contact_uri`. |
| استيثاق نقطة النهاية | Não há nenhum problema em que você possa usar mTLS ou QUIC. | Verifique se o Norito `EndpointAttestationV1` está funcionando corretamente. |

## سير عمل القبول

1. **إنشاء المقترح**
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     Para `ProviderAdmissionProposalV1` + número de telefone.
   - التحقق: ضمان الحقول المطلوبة, وstake > 0, ومقبض chunker قياسي في `profile_id`.
2. **اعتماد الحوكمة**
   - يوقع المجلس `blake3("sorafs-provider-admission-v1" || canonical_bytes)` باستخدام أدوات envelope الحالية
     (efeito `sorafs_manifest::governance`).
   - Abra o envelope em `governance/providers/<provider_id>/admission.json`.
3. **إدخال السجل**
   - O arquivo de configuração (`sorafs_manifest::provider_admission::validate_envelope`) é compatível com Torii/CLI.
   - تحديث مسار القبول في Torii لرفض adverts التي يختلف digest, أو تاريخ الانتهاء فيها عن الـ envelope.
4. **التجديد والإلغاء**
   - إضافة `ProviderAdmissionRenewalV1` مع تحديثات اختيارية لنقاط النهاية/الـ estaca.
   - O CLI `--revoke` pode ser removido do computador e do computador.

## مهام التنفيذ

| المجال | المهمة | Proprietário(s) | الحالة |
|----|-------|----------|--------|
| المخطط | A versão `ProviderAdmissionProposalV1` e `ProviderAdmissionEnvelopeV1` e `EndpointAttestationV1` (Norito) é `crates/sorafs_manifest/src/provider_admission.rs`. 【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Armazenamento / Governança | ✅ مكتمل |
| CLI | O padrão `sorafs_manifest_stub` é o seguinte: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | GT Ferramentaria | ✅ |O CLI é o nome do cliente (`--endpoint-attestation-intermediate`) e o envelope de bytes. Verifique o valor do `sign`/`verify`. يمكن للمشغلين توفير أجسام adverts مباشرة أو إعادة استخدام adverts موقعة, ويمكن تمرير ملفات Verifique se o `--council-signature-public-key` e o `--council-signature-file` são usados.

### مرجع CLI

Este é o `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - Nome do código: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, e o `--endpoint=<kind:host>` também.
  - استيثاق كل نقطة نهاية يتطلب `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, وشهادة عبر
    `--endpoint-attestation-leaf=<path>` (por exemplo, `--endpoint-attestation-intermediate=<path>` é um dispositivo de armazenamento de dados) e um dispositivo ALPN de alta qualidade
    (`--endpoint-attestation-alpn=<token>`). يمكن لنقاط نهاية QUIC توفير تقارير النقل عبر
    `--endpoint-attestation-report[-hex]=...`.
  - Valores: bytes que são atribuídos a Norito (`--proposal-out`) e JSON
    (stdout criado ou `--json-out`).
-`sign`
  - Nome: مقترح (`--proposal`), anúncio موقع (`--advert`), جسم anúncio اختياري
    (`--advert-body`), época احتفاظ, وتوقيع مجلس واحد على الأقل. يمكن تمرير التواقيع
    inline (`--council-signature=<signer_hex:signature_hex>`) e sem problemas
    `--council-signature-public-key` em vez de `--council-signature-file=<path>`.
  - Envelope مُحقق (`--envelope-out`) e JSON يوضح روابط digest وعدد الموقّعين ومسارات الإدخال.
-`verify`
  - يتحقق من envelope موجود (`--envelope`) مع فحص اختياري للمقترح المطابق أو anúncio أو جسم anúncio. O JSON é definido como um resumo, um resumo e um artefato que você pode usar.
-`renewal`
  - يربط envelope مُعتمد جديد بالـ digest الذي تم التصديق عليه سابقا. يتطلب
    `--previous-envelope=<path>` e `--envelope=<path>` (referente ao Norito).
    يتحقق CLI من بقاء aliases de perfil والقدرات ومفاتيح anúncio دون تغيير, مع السماح بتحديثات estaca ونقاط النهاية والـ metadados. ينتج bytes
    `ProviderAdmissionRenewalV1` (`--renewal-out`) é definido como JSON.
-`revoke`
  - Remova o envelope `ProviderAdmissionRevocationV1` do envelope. يتطلب `--envelope=<path>`, `--reason=<text>`, توقيع مجلس واحد على الأقل
    `--council-signature`, e `--revoked-at`/`--notes` são usados. O CLI é o resumo do arquivo, e o Norito é o `--revocation-out` e o JSON é o digest e o JSON التواقيع.
| التحقق | Você pode usar o Torii e o `‎sorafs-node`. 【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Rede TL / Armazenamento | ✅ مكتمل |
| Modelo Torii | تمرير المدقق في إدخال anúncios em Torii e anúncios خارج السياسة وإصدار التليمترية. | Rede TL | ✅ مكتمل | O Torii contém envelopes de envelopes (`torii.sorafs.admission_envelopes_dir`) e o digest/التوقيع أثناء الإدخال وإبراز تليمترية القبول.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| التجديد | إضافة مخطط التجديد/الإلغاء + مساعدات CLI ونشر دليل دورة الحياة في الوثائق (راجع الـ runbook أدناه Use CLI em `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Armazenamento / Governança | ✅ مكتمل || التليمترية | Limpe o envelope/envelope `provider_admission` (um envelope pequeno). | Observabilidade | 🟠 جار | `torii_sorafs_admission_total{result,reason}` `torii_sorafs_admission_total{result,reason}` لوحات/تنبيهات قيد الانتظار.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### دليل التجديد والإلغاء

#### تجديد مجدول (participação/topologia)
1. أنشئ زوج المقترح/advert اللاحق باستخدام `provider-admission proposal` و`provider-admission sign`, مع زيادة `--retention-epoch` وتحديث stake/endpoints حسب الحاجة.
2. Não
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   يقوم الأمر بالتحقق من ثبات حقول القدرة/الملف عبر `AdmissionRecord::apply_renewal`, ويصدر `ProviderAdmissionRenewalV1` ويطبع digests لسجل الحوكمة.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Use o envelope para `torii.sorafs.admission_envelopes_dir`, e use Norito/JSON para gerar hash e hash Período + época de retenção em `docs/source/sorafs/migration_ledger.md`.
4. Abra o envelope do envelope e coloque-o no `torii_sorafs_admission_total{result="accepted",reason="stored"}`.
5. Instale os fixtures no `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`; CI (`ci/check_sorafs_fixtures.sh`) é o mesmo que Norito.

#### إلغاء طارئ
1. Abra o envelope do envelope e coloque-o:
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
   يوقع CLI `ProviderAdmissionRevocationV1`, ويتحقق من مجموعة التواقيع عبر `verify_revocation_signatures`, ويبلغ digest الإلغاء.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Use o envelope de `torii.sorafs.admission_envelopes_dir`, e Norito/JSON para armazenar caches, e hash no site. الحوكمة.
3. راقب `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` لتأكيد أن caches تُسقط advert الملغى؛ واحتفظ بآثار الإلغاء em مراجعات الحوادث.

## الاختبارات والتليمترية

- إضافة fixtures ذهبية لمقترحات وأغلفة القبول تحت
  `fixtures/sorafs_manifest/provider_admission/`.
- Use CI (`ci/check_sorafs_fixtures.sh`) para remover envelopes e envelopes.
- تتضمن fixtures المولدة `metadata.json` مع digests قياسية؛ تؤكد اختبارات downstream أن
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- توفير اختبارات تكامل:
  - يرفض Torii anúncios e envelopes قبول مفقودة, e منتهية الصلاحية.
  - يقوم CLI بعملية ida e volta للمقترح → envelope → التحقق.
  - يقوم تجديد الحوكمة بتدوير استيثاق endpoint دون تغيير معرف المزود.
- متطلبات التليمترية:
  - Instale `provider_admission_envelope_{accepted,rejected}` em Torii. ✅ `torii_sorafs_admission_total{result,reason}` não está disponível/definido.
  - إضافة تحذيرات انتهاء الصلاحية إلى لوحات المراقبة (تجديد مستحق خلال 7 dias).

## الخطوات التالية

1. ✅ Você deve usar o Norito e o dispositivo de reparo no `sorafs_manifest::provider_admission`. Não há problema.
2. ✅ تم توثيق مسارات CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) وتجريبها عبر اختبارات التكامل; Você pode usar o runbook do runbook.
3. ✅ يقوم Torii admissão/descoberta بإدخال الـ envelopes ويعرض عدادات التليمترية للقبول/الرفض.
4. التركيز على الملاحظة: إنهاء لوحات/تنبيهات القبول بحيث تطلق تنبيهات عند قرب الاستحقاق خلال سبعة أيام (`torii_sorafs_admission_total`, medidores de validade).