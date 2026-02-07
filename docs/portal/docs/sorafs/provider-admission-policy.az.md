---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 17fcb22d5be25f601d4096c3a3488b7be2dd92dcf27019b678634590cd3bdde4
source_last_modified: "2025-12-29T18:16:35.197199+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

> [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md)-dən uyğunlaşdırılıb.

# SoraFS Provayder Qəbulu və Şəxsiyyət Siyasəti (SF-2b Qaralama)

Bu qeyd **SF-2b** üçün icra edilə bilən nəticələri əks etdirir: müəyyən edən və
qəbul iş axını, şəxsiyyət tələbləri və attestasiyanın həyata keçirilməsi
SoraFS yaddaş provayderləri üçün faydalı yüklər. Yüksək səviyyəli prosesi genişləndirir
SoraFS Arxitektura RFC-də təsvir edilmişdir və qalan işi bölür
izlənilə bilən mühəndislik vəzifələri.

## Siyasət Məqsədləri

- Yalnız yoxlanılmış operatorların `ProviderAdvertV1` qeydlərini dərc edə biləcəyinə əmin olun
  şəbəkə qəbul edəcək.
- Hər bir reklam açarını idarəetmə tərəfindən təsdiq edilmiş şəxsiyyət sənədinə bağlayın,
  təsdiq edilmiş son nöqtələr və minimum pay töhfəsi.
- Torii, şlüzlər və deterministik yoxlama alətlərini təmin edin
  `sorafs-node` eyni yoxlamaları həyata keçirir.
- Determinizmi pozmadan yeniləmə və təcili ləğvi dəstəkləyin
  alətlərin erqonomikası.

## Şəxsiyyət və Stake Tələbləri

| Tələb | Təsvir | Çatdırılır |
|-------------|-------------|-------------|
| Reklamın əsas mənbəsi | Provayderlər hər bir reklamı imzalayan Ed25519 açar cütünü qeydiyyatdan keçirməlidirlər. Qəbul paketi ictimai açarı idarəetmə imzası ilə birlikdə saxlayır. | `ProviderAdmissionProposalV1` sxemini `advert_key` (32 bayt) ilə genişləndirin və reyestrdən (`sorafs_manifest::provider_admission`) istinad edin. |
| Bahis göstəricisi | Qəbul üçün aktiv staking hovuzunu göstərən sıfırdan fərqli `StakePointer` tələb olunur. | `sorafs_manifest::provider_advert::StakePointer::validate()`-də doğrulama və CLI/testlərdə səth xətaları əlavə edin. |
| Yurisdiksiya teqləri | Provayderlər yurisdiksiyanı bəyan edir + hüquqi əlaqə. | Təklif sxemini `jurisdiction_code` (ISO 3166-1 alfa-2) və əlavə `contact_uri` ilə genişləndirin. |
| Son nöqtənin attestasiyası | Hər bir elan edilmiş son nöqtə mTLS və ya QUIC sertifikat hesabatı ilə dəstəklənməlidir. | `EndpointAttestationV1` Norito faydalı yükü təyin edin və qəbul paketinin daxilində son nöqtəyə görə saxlayın. |

## Qəbul İş axını

1. **Təklifin yaradılması**
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal …` əlavə edin
     `ProviderAdmissionProposalV1` + attestasiya paketi istehsal edir.
   - Təsdiqləmə: tələb olunan sahələri təmin edin, pay > 0, `profile_id`-də kanonik chunker sapı.
2. **İdarəetmə təsdiqi**
   - Şura mövcud istifadə edərək `blake3("sorafs-provider-admission-v1" || canonical_bytes)` imzalar
     zərf alətləri (`sorafs_manifest::governance` modulu).
   - Zərf `governance/providers/<provider_id>/admission.json`-ə qədər saxlanılır.
3. **Qeydiyyatın qəbulu**
   - Paylaşılan doğrulayıcı tətbiq edin (`sorafs_manifest::provider_admission::validate_envelope`)
     Torii/şlüzlər/CLI-nin təkrar istifadəsi.
   - Həzm və ya müddəti zərfdən fərqli olan reklamları rədd etmək üçün Torii qəbul yolunu yeniləyin.
4. **Yeniləmə və ləğvetmə**
   - Əlavə son nöqtə/stake yeniləmələri ilə `ProviderAdmissionRenewalV1` əlavə edin.
   - Ləğv etmə səbəbini qeyd edən və idarəetmə hadisəsinə təkan verən `--revoke` CLI yolunu ifşa edin.

## İcra Tapşırıqları

| Ərazi | Tapşırıq | Sahib(lər) | Status |
|------|------|----------|--------|
| Sxe | `crates/sorafs_manifest/src/provider_admission.rs` altında `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) təyin edin. Doğrulama köməkçiləri ilə `sorafs_manifest::provider_admission`-də həyata keçirilir.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Saxlama / İdarəetmə | ✅ Tamamlandı |
| CLI alətləri | `sorafs_manifest_stub`-i alt əmrlərlə genişləndirin: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Tooling WG | ✅ |

CLI axını indi aralıq sertifikat paketlərini (`--endpoint-attestation-intermediate`) qəbul edir, emissiya edir
kanonik təklif/zərf baytları və `sign`/`verify` zamanı şura imzalarını təsdiqləyir. Operatorlar bilər
reklam orqanlarını birbaşa təmin edin və ya imzalanmış reklamları təkrar istifadə edin və imza faylları cütləşmə yolu ilə təmin edilə bilər
Avtomatlaşdırmaya uyğunluq üçün `--council-signature-file` ilə `--council-signature-public-key`.

### CLI Referansı

Hər əmri `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission …` vasitəsilə yerinə yetirin.

- `proposal`
  - Tələb olunan bayraqlar: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>` və ən azı bir `--endpoint=<kind:host>`.
  - Son nöqtə üzrə attestasiya `--endpoint-attestation-attested-at=<secs>` gözləyir,
    `--endpoint-attestation-expires-at=<secs>`, vasitəsilə sertifikat
    `--endpoint-attestation-leaf=<path>` (üstəlik isteğe bağlı `--endpoint-attestation-intermediate=<path>`
    hər bir zəncir elementi üçün) və hər hansı razılaşdırılmış ALPN ID-ləri
    (`--endpoint-attestation-alpn=<token>`). QUIC son nöqtələri nəqliyyat hesabatları ilə təmin edə bilər
    `--endpoint-attestation-report[-hex]=…`.
  - Nəticə: kanonik Norito təklif baytları (`--proposal-out`) və JSON xülasəsi
    (standart stdout və ya `--json-out`).
- `sign`
  - Girişlər: təklif (`--proposal`), imzalanmış reklam (`--advert`), isteğe bağlı reklam orqanı
    (`--advert-body`), saxlama dövrü və ən azı bir şura imzası. İmzalar verilə bilər
    inline (`--council-signature=<signer_hex:signature_hex>`) və ya fayllar vasitəsilə birləşdirərək
    `--council-signature-public-key`, `--council-signature-file=<path>` ilə.
  - Təsdiqlənmiş zərf (`--envelope-out`) və həzm bağlamalarını göstərən JSON hesabatı hazırlayır,
    imzalayanların sayı və giriş yolları.
- `verify`
  - Mövcud zərfi (`--envelope`) təsdiqləyir, istəyə uyğun gələn təklifi yoxlayır,
    reklam və ya reklam orqanı. JSON hesabatı həzm dəyərlərini, imza doğrulama statusunu,
    və hansı isteğe bağlı artefaktların uyğun gəldiyi.
- `renewal`
  - Yeni təsdiq edilmiş zərfi əvvəllər ratifikasiya olunmuş həzm ilə əlaqələndirir. Tələb edir
    `--previous-envelope=<path>` və varisi `--envelope=<path>` (hər ikisi Norito faydalı yükləri).
    CLI profil ləqəblərinin, imkanlarının və reklam açarlarının dəyişməz qaldığını yoxlayır
    pay, son nöqtələr və metadata yeniləmələrinə icazə verir. Kanonik çıxarır
    `ProviderAdmissionRenewalV1` bayt (`--renewal-out`) üstəgəl JSON xülasəsi.
- `revoke`
  - Zərfi lazım olan provayder üçün təcili `ProviderAdmissionRevocationV1` paketini verir
    geri çəkilmək. `--envelope=<path>`, `--reason=<text>`, ən azı bir tələb edir
    `--council-signature` və isteğe bağlı `--revoked-at`/`--notes`. CLI imzalayır və təsdiqləyir
    ləğvetmə həzmi, `--revocation-out` vasitəsilə Norito faydalı yükünü yazır və JSON hesabatını çap edir
    həzm və imza sayının tutulması.
| Doğrulama | Torii, şlüzlər və `sorafs-node` tərəfindən istifadə edilən paylaşılan doğrulayıcını tətbiq edin. Vahid + CLI inteqrasiya testlərini təmin edin.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Şəbəkə TL / Saxlama | ✅ Tamamlandı |
| Torii inteqrasiya | Doğrulayıcını Torii reklam qəbuluna daxil edin, siyasətdən kənar reklamları rədd edin, telemetriya buraxın. | Şəbəkə TL | ✅ Tamamlandı | Torii indi idarəetmə zərflərini yükləyir (`torii.sorafs.admission_envelopes_dir`), qəbul zamanı həzm/imza uyğunluqlarını yoxlayır və qəbulu üzə çıxarır telemetriya.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.
| Yenilənmə | Yeniləmə/ləğv etmə sxemi + CLI köməkçiləri əlavə edin, sənədlərdə həyat dövrü bələdçisini dərc edin (aşağıda runbook və CLI əmrlərinə baxın) `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md: |120 Saxlama / İdarəetmə | ✅ Tamamlandı |
| Telemetriya | `provider_admission` tablosunu və xəbərdarlıqlarını təyin edin (yenilənmə yoxdur, zərfin müddəti). | Müşahidə qabiliyyəti | 🟠 Davam edir | Sayğac `torii_sorafs_admission_total{result,reason}` mövcuddur; idarə panelləri/silahlar gözləyir.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |
### Yeniləmə və Ləğvetmə Runbook

#### Planlaşdırılmış yenilənmə (pay/topologiya yeniləmələri)
1. `provider-admission proposal` və `provider-admission sign` ilə davamçı təklif/reklam cütünü yaradın, `--retention-epoch` artırın və tələb olunduqda pay/son nöqtələri yeniləyin.
2. İcra etmək  
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   Komanda dəyişdirilməmiş qabiliyyət/profil sahələrini vasitəsilə təsdiqləyir
   `AdmissionRecord::apply_renewal`, `ProviderAdmissionRenewalV1` yayır və həzmləri çap edir
   idarəetmə jurnalı.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. `torii.sorafs.admission_envelopes_dir`-də əvvəlki zərfi dəyişdirin, Norito/JSON yenilənməsini idarəetmə repozitoriyasına həvalə edin və yenilənmə hashı + saxlama epoxasını `docs/source/sorafs/migration_ledger.md`-ə əlavə edin.
4. Operatorlara yeni zərfin canlı olduğunu bildirin və qəbulu təsdiqləmək üçün `torii_sorafs_admission_total{result="accepted",reason="stored"}`-ə nəzarət edin.
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` vasitəsilə kanonik qurğuları bərpa edin və icra edin; CI (`ci/check_sorafs_fixtures.sh`) Norito çıxışlarının sabit qalmasını təsdiqləyir.

#### Təcili ləğv
1. Təhlükəli zərfi müəyyən edin və geri götürün:
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
   CLI `ProviderAdmissionRevocationV1` imzalayır, imza dəstini təsdiqləyir
   `verify_revocation_signatures` və ləğvetmə həzmini bildirir.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#48】L
2. `torii.sorafs.admission_envelopes_dir`-dən zərfi çıxarın, ləğvetmə Norito/JSON-u qəbul keşlərinə paylayın və idarəetmə protokolunda hash səbəbini qeyd edin.
3. Keşlərin ləğv edilmiş reklamı buraxdığını təsdiqləmək üçün `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`-ə baxın; ləğvetmə artefaktlarını hadisənin retrospektivlərində saxlamaq.

## Test və Telemetriya- Altına qəbul təklifləri və zərflər üçün qızıl qurğular əlavə edin
  `fixtures/sorafs_manifest/provider_admission/`.
- Təklifləri bərpa etmək və zərfləri yoxlamaq üçün CI (`ci/check_sorafs_fixtures.sh`) genişləndirin.
- Yaradılmış qurğulara kanonik həzmləri olan `metadata.json` daxildir; aşağı axın testləri təsdiqləyir
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- İnteqrasiya testlərini təmin edin:
  - Torii buraxılış zərfləri əskik olan və ya vaxtı keçmiş reklamları rədd edir.
  - CLI gediş-gəliş təklifi → zərf → yoxlama.
  - İdarəetmə yenilənməsi provayder ID-ni dəyişdirmədən son nöqtənin attestasiyasını dəyişir.
- Telemetriya tələbləri:
  - Torii-də `provider_admission_envelope_{accepted,rejected}` sayğaclarını buraxın. ✅ `torii_sorafs_admission_total{result,reason}` indi qəbul edilən/rədd edilən nəticələri göstərir.
  - Müşahidə oluna bilən tablolara son istifadə müddəti xəbərdarlığı əlavə edin (yenilənmə 7 gün ərzində həyata keçirilir).

## Növbəti Addımlar

1. ✅ Norito sxem dəyişiklikləri və təsdiqləmə köməkçiləri tamamlandı
   `sorafs_manifest::provider_admission`. Heç bir xüsusiyyət bayraqları tələb olunmur.
2. ✅ CLI iş axınları (`proposal`, `sign`, `verify`, `renewal`, `revoke`) inteqrasiya testləri vasitəsilə sənədləşdirilir və həyata keçirilir; idarəetmə skriptlərini runbook ilə sinxronlaşdırın.
3. ✅ Torii qəbul/kəşf zərfləri qəbul edir və telemetriya sayğaclarını qəbul/rədd üçün ifşa edir.
4. Müşahidə oluna bilənliyə diqqət yetirin: qəbul panellərini/xəbərdarlıqlarını bitirin ki, yeddi gün ərzində yenilənmələr xəbərdarlıqları artırsın (`torii_sorafs_admission_total`, istifadə müddəti ölçüləri).