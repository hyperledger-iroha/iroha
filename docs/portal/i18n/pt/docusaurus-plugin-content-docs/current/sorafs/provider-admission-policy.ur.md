---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md) سے ماخوذ۔

# SoraFS فراہم کنندگان کی قبولیت اور شناخت کی پالیسی (SF-2b مسودہ)

یہ نوٹ **SF-2b** کے لیے قابلِ عمل ڈیلیوریبلز کو سمیٹتا ہے: SoraFS اسٹوریج فراہم کنندگان کے لیے قبولیت کے ورک فلو، شناختی تقاضے, اور توثیقی payloads کی تعریف اور Não یہ SoraFS Arquitetura RFC میں بیان کردہ اعلی سطح کے عمل کو وسعت دیتا ہے اور باقی کام کو قابلِ ٹریک انجینئرنگ ٹاسکس میں تقسیم کرتا ہے۔

## پالیسی کے اہداف

- یہ یقینی بنانا کہ صرف تصدیق شدہ آپریٹرز ہی `ProviderAdvertV1` ریکارڈز شائع کر سکیں جنہیں نیٹ ورک قبول کرے۔
- ہر اعلان کلید کو گورننس سے منظور شدہ شناختی دستاویز, توثیق شدہ endpoints, اور کم از کم stake شراکت کے ساتھ باندھنا۔
- Torii, gateways, e `sorafs-node` ایک ہی چیک نافذ کریں اس کے لیے ڈیٹرمنسٹک ویریفیکیشن ٹولنگ فراہم کرنا۔
- ڈیٹرمنزم یا ٹولنگ ergonomia کو توڑے بغیر تجدید اور ہنگامی منسوخی کی حمایت کرنا۔

## شناخت اور participação تقاضے

| تقاضا | وضاحت | ڈیلیوریبل |
|-------|-------|-----------|
| اعلان کلید کا ماخذ | فراہم کنندگان کو Ed25519 keypair رجسٹر کرنا ہوگا جو ہر anúncio پر دستخط کرے۔ pacote de admissão گورننس دستخط کے ساتھ پبلک chave محفوظ کرتا ہے۔ | `ProviderAdmissionProposalV1` اسکیمہ میں `advert_key` (32 bytes) شامل کریں اور اسے رجسٹری (`sorafs_manifest::provider_admission`) سے ریفرنس کریں۔ |
| Estaca پوائنٹر | قبولیت کے لیے فعال staking pool کی طرف اشارہ کرنے والا غیر صفر `StakePointer` درکار ہے۔ | `sorafs_manifest::provider_advert::StakePointer::validate()` میں ویلیڈیشن شامل کریں اور CLI/testes میں erros ظاہر کریں۔ |
| Jurisdição ٹیگز | فراہم کنندگان jurisdição + قانونی رابطہ ظاہر کرتے ہیں۔ | پروپوزل اسکیمہ میں `jurisdiction_code` (ISO 3166-1 alpha-2) اور اختیاری `contact_uri` شامل کریں۔ |
| Ponto final ہر اعلان شدہ endpoint کو mTLS یا QUIC سرٹیفکیٹ رپورٹ سے سپورٹ کرنا لازم ہے۔ | Carga útil Norito `EndpointAttestationV1` کی تعریف کریں اور اسے pacote de admissão کے اندر ہر endpoint کے ساتھ اسٹور کریں۔ |

## قبولیت کا ورک فلو

1. **پروپوزل تیار کرنا**
   -CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     شامل کریں جو `ProviderAdmissionProposalV1` + توثیقی pacote بنائے۔
   - ویلیڈیشن: ضروری فیلڈز, aposta> 0, اور `profile_id` میں canonical chunker handle کو یقینی بنائیں۔
2. **گورننس کی منظوری**
   - Ferramentas de envelope کونسل موجودہ (ماڈیول `sorafs_manifest::governance`) استعمال کرتے ہوئے
     `blake3("sorafs-provider-admission-v1" || canonical_bytes)` پر دستخط کرتی ہے۔
   - Envelope کو `governance/providers/<provider_id>/admission.json` میں محفوظ کیا جاتا ہے۔
3. **رجسٹری میں اندراج**
   - ایک مشترکہ verificador (`sorafs_manifest::provider_admission::validate_envelope`) نافذ کریں جسے Torii/gateways/CLI دوبارہ استعمال کریں۔
   - Torii کے admissão راستے کو اپڈیٹ کریں تاکہ وہ ایسے anúncios کو مسترد کرے جن کا resumo یا envelope de expiração سے مختلف ہو۔
4. **تجدید اور منسوخی**
   - اختیاری endpoint/stake اپڈیٹس کے ساتھ `ProviderAdmissionRenewalV1` شامل کریں۔
   - ایک CLI راستہ `--revoke` فراہم کریں جو منسوخی کی وجہ ریکارڈ کرے اور گورننس ایونٹ بھیجے۔

## عمل درآمد کے کام| علاقہ | کام | Proprietário(s) | حالت |
|-------|-----|----------|------|
| اسکیمہ | `crates/sorafs_manifest/src/provider_admission.rs` کے تحت `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) کی تعریف کریں۔ `sorafs_manifest::provider_admission` میں ویلیڈیشن helpers کے ساتھ نافذ کیا گیا ہے۔【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Armazenamento / Governança | ✅ مکمل |
| CLI Módulo | `sorafs_manifest_stub` é um dispositivo de armazenamento de dados: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | GT Ferramentaria | ✅ مکمل |

CLI é um pacote configurável de pacotes (`--endpoint-attestation-intermediate`) que está disponível para download
proposta canônica/bytes de envelope جاری کرتا ہے, اور `sign`/`verify` کے دوران کونسل assinaturas کو ویری فائی کرتا ہے۔ آپریٹرز corpos de anúncios براہ راست فراہم کر سکتے ہیں یا anúncios assinados دوبارہ استعمال کر سکتے ہیں, اور arquivos de assinatura O `--council-signature-public-key` é um `--council-signature-file` que é uma ferramenta de automação de automação

### CLI

ہر کمانڈ کو `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...` کے ذریعے چلائیں۔-`proposal`
  - Nome de usuário: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, اور کم از کم ایک `--endpoint=<kind:host>`۔
  - ہر endpoint کے لیے توثیق میں `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, ایک سرٹیفکیٹ بذریعہ
    `--endpoint-attestation-leaf=<path>` (ہر چین عنصر کے لیے اختیاری `--endpoint-attestation-intermediate=<path>`)
    Qual é o número de IDs ALPN (`--endpoint-attestation-alpn=<token>`) disponíveis? Endpoints QUIC são e quais são os pontos de extremidade
    `--endpoint-attestation-report[-hex]=...` کے ذریعے فراہم کر سکتے ہیں۔
  - Isso é: bytes de proposta canônicos Norito (`--proposal-out`) e JSON خلاصہ
    (ڈیفالٹ stdout یا `--json-out`).
-`sign`
  - ان پٹس: ایک پروپوزل (`--proposal`), ایک anúncio assinado (`--advert`), اختیاری corpo do anúncio
    (`--advert-body`), época de retenção, اور کم از کم ایک کونسل assinatura۔ assinaturas کو inline
    (`--council-signature=<signer_hex:signature_hex>`) یا فائلز کے ذریعے فراہم کیا جا سکتا ہے جب
    `--council-signature-public-key` کو `--council-signature-file=<path>` کے ساتھ ملایا جائے۔
  - Envelope validado (`--envelope-out`) e JSON رپورٹ تیار کرتا ہے, ligações de resumo, contagem de signatários, e caminhos de entrada دکھاتی ہے۔
-`verify`
  - موجودہ envelope (`--envelope`) کی تصدیق کرتا ہے, اور اختیاری طور پر proposta correspondente, anúncio, یا corpo do anúncio چیک کرتا ہے۔ Valores de resumo JSON رپورٹ, status de verificação de assinatura, اور وہ artefatos opcionais دکھاتی ہے جو correspondência ہوئے۔
-`renewal`
  - نئے منظور شدہ envelope کو پہلے سے منظور شدہ digest کے ساتھ جوڑتا ہے۔ اس کے لیے
    `--previous-envelope=<path>` e `--envelope=<path>` (cargas úteis Norito)
    CLI tem vários aliases de perfil, recursos, chaves de anúncio, chaves de anúncio, endpoints e metadados اپڈیٹس کی اجازت دیتا ہے۔ bytes canônicos `ProviderAdmissionRenewalV1` (`--renewal-out`) e JSON
-`revoke`
  - ایسے provedor کے لیے ہنگامی pacote `ProviderAdmissionRevocationV1` جاری کرتا ہے جس کا envelope واپس لینا ضروری ہو۔
    `--envelope=<path>`, `--reason=<text>`, کم از کم ایک `--council-signature` درکار ہے, اور
    `--revoked-at`/`--notes` اختیاری ہیں۔ Resumo de revogação CLI کو assinar/verificar کرتا ہے, carga útil Norito
    `--revocation-out` کے ذریعے لکھتا ہے، اور resumo e contagem de assinatura کے ساتھ JSON رپورٹ پرنٹ کرتا ہے۔
| ویریفیکیشن | Torii, gateways, e `sorafs-node` کے لیے مشترکہ verificador نافذ کریں۔ testes de integração de unidade + CLI Rede TL / Armazenamento | ✅ مکمل |
| Torii Máquina | verificador کو Torii میں Ads Ingest کرنے کے دوران شامل کریں, پالیسی سے باہر anúncios مسترد کریں, اور telemetria جاری کریں۔ | Rede TL | ✅ مکمل | Torii اب envelopes de governança (`torii.sorafs.admission_envelopes_dir`) لوڈ کرتا ہے, ingerir کے دوران digest/signature match کی تصدیق کرتا ہے, اور admissão telemetria ظاہر کرتا ہے。【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 || تجدید | تجدید/منسوخی اسکیمہ + CLI helpers شامل کریں, اور guia de ciclo de vida کو docs میں شائع کریں (نیچے runbook اور `provider-admission renewal`/`revoke` CLI Cartão دیکھیں)。【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Armazenamento / Governança | ✅ مکمل |
| ٹیلیمیٹری | Painéis `provider_admission` e alertas کی تعریف کریں (تجدید کی کمی, expiração do envelope)۔ | Observabilidade | 🟠 جاری | کاؤنٹر `torii_sorafs_admission_total{result,reason}` موجود ہے؛ painéis/alertas زیر التوا ہیں۔【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### تجدید اور منسوخی کا رن بُک

#### شیڈول شدہ تجدید (estaca/topologia اپڈیٹس)
1. `provider-admission proposal` ou `provider-admission sign` کے ذریعے جانشین proposta/anúncio جوڑا بنائیں،
   `--retention-epoch` بڑھائیں اور ضرورت کے مطابق stake/endpoints اپڈیٹ کریں۔
2. Carreira
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   یہ کمانڈ `AdmissionRecord::apply_renewal` کے ذریعے capacidade/perfil فیلڈز کو غیر تبدیل شدہ ہونے کی تصدیق کرتی ہے،
   `ProviderAdmissionRenewalV1` جاری کرتی ہے، اور گورنس لاگ کے لیے digests پرنٹ کرتی ہے۔【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. `torii.sorafs.admission_envelopes_dir` میں پچھلا envelope تبدیل کریں, renovação Norito/JSON کو گورنس ریپوزٹری میں commit کریں,
   Hash de renovação + época de retenção کو `docs/source/sorafs/migration_ledger.md` میں شامل کریں۔
4. آپریٹرز کو بتائیں کہ نیا envelope فعال ہے اور ingerir کی تصدیق کے لیے
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` مانیٹر کریں۔
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` کے ذریعے luminárias canônicas دوبارہ بنائیں اور commit کریں؛
   CI (`ci/check_sorafs_fixtures.sh`) Norito آؤٹ پٹس کی estabilidade چیک کرتا ہے۔

#### ہنگامی منسوخی
1. Envelope متاثرہ کی شناخت کریں اور منسوخی جاری کریں:
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
   CLI `ProviderAdmissionRevocationV1` پر دستخط کرتا ہے، `verify_revocation_signatures` کے ذریعے assinaturas کا سیٹ ویریفائی کرتا ہے،
   Para o resumo de revogação رپورٹ کرتا ہے۔【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. `torii.sorafs.admission_envelopes_dir` سے envelope ہٹا دیں, revogação Norito/JSON کو caches de admissão میں تقسیم کریں،
   Qual é o hash گورننس منٹس میں ریکارڈ کریں۔
3. `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` دیکھیں تاکہ تصدیق ہو سکے کہ caches نے anúncio revogado کو drop کر دیا ہے؛
   artefatos de revogação کو retrospectivas de incidentes میں محفوظ رکھیں۔

## ٹیسٹنگ اور ٹیلیمیٹری- propostas de admissão اور envelopes کے لیے luminárias douradas کو `fixtures/sorafs_manifest/provider_admission/` کے تحت شامل کریں۔
- CI (`ci/check_sorafs_fixtures.sh`) کو وسعت دیں تاکہ propostas دوبارہ بنائے جائیں اور envelopes ویریفائی ہوں۔
- تیار شدہ fixtures میں resumos canônicos کے ساتھ `metadata.json` شامل ہوتا ہے؛ testes downstream یہ assert کرتے ہیں کہ
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- testes de integração فراہم کریں:
  - Torii ایسے anúncios مسترد کرتا ہے جن کے envelopes de admissão غائب یا میعاد ختم ہو چکے ہوں۔
  - Proposta CLI → envelope → verificação کا ida e volta چلاتا ہے۔
  - گورننس تجدید ID do provedor تبدیل کیے بغیر endpoint توثیق کو girar کرتی ہے۔
- ٹیلیمیٹری کی ضروریات:
  - Torii میں `provider_admission_envelope_{accepted,rejected}` کاؤنٹرز emite کریں۔ ✅ `torii_sorafs_admission_total{result,reason}` اب aceito/rejeitado نتائج دکھاتا ہے۔
  - painéis de observabilidade میں avisos de expiração شامل کریں (7 دن کے اندر تجدید درکار ہونے پر)۔

## اگلے اقدامات

1. ✅ Norito اسکیمہ تبدیلیاں حتمی ہو چکی ہیں اور `sorafs_manifest::provider_admission` میں auxiliares de validação شامل ہو گئے ہیں۔ Quais são os sinalizadores de recurso?
2. ✅ CLI فلو (`proposal`, `sign`, `verify`, `renewal`, `revoke`) دستاویزی ہیں اور testes de integração سے گزارے گئے ہیں؛ گورننس اسکرپٹس کو runbook کے ساتھ ہم آہنگ رکھیں۔
3. ✅ Ingestão de envelopes de admissão/descoberta Torii کرتا ہے اور قبولیت/رد کے contadores de telemetria دکھاتا ہے۔
4. observabilidade پر توجہ: painéis/alertas de admissão (`torii_sorafs_admission_total`, medidores de validade)۔