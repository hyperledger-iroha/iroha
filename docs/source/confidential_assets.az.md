---
lang: az
direction: ltr
source: docs/source/confidential_assets.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 969ffd4cee6ee4880d5f754fb36adaf30dde532a29e4c6397cf0f358438bb57e
source_last_modified: "2026-01-22T16:26:46.566038+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->
# Məxfi Aktivlər və ZK Transfer Dizaynı

## Motivasiya
- Domenlərin şəffaf dövriyyəni dəyişmədən tranzaksiya məxfiliyini qoruyub saxlaya bilməsi üçün daxil olmaqla qorunan aktiv axınını təmin edin.
- Auditorları və operatorları sxemlər və kriptoqrafik parametrlər üçün həyat dövrünə nəzarət vasitələri (aktivləşdirmə, fırlanma, ləğvetmə) ilə təmin etmək.

## Təhdid Modeli
- Qiymətləndiricilər dürüst, lakin maraqlıdırlar: onlar konsensusa sədaqətlə yanaşırlar, lakin mühasibat kitabını/vəziyyətini yoxlamağa çalışırlar.
- Şəbəkə müşahidəçiləri blok məlumatlarını və qeybət edilən əməliyyatları görür; özəl dedi-qodu kanallarının heç bir fərziyyəsi yoxdur.
- Əhatə dairəsi xaricində: kitabdan kənar trafik təhlili, kvant rəqibləri (PQ yol xəritəsi altında ayrıca izlənilir), kitab əlçatanlığına hücumlar.

## Dizayn Baxışı
- Aktivlər mövcud şəffaf balanslara əlavə olaraq *qorlanmış hovuz* elan edə bilər; qorunan dövriyyə kriptoqrafik öhdəliklər vasitəsilə təmsil olunur.
- Qeydlər `(asset_id, amount, recipient_view_key, blinding, rho)` ilə əhatə edir:
  - Öhdəlik: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, qeyd sifarişindən asılı olmayaraq.
  - Şifrələnmiş faydalı yük: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Əməliyyatlar aşağıdakıları ehtiva edən Norito kodlu `ConfidentialTransfer` faydalı yükləri nəql edir:
  - İctimai girişlər: Merkle ankeri, ləğvedicilər, yeni öhdəliklər, aktiv identifikatoru, dövrə versiyası.
  - Alıcılar və əlavə auditorlar üçün şifrələnmiş yüklər.
  - Dəyərin qorunmasını, sahibliyini və icazəsini təsdiqləyən sıfır bilik sübutu.
- Doğrulama açarları və parametr dəstləri aktivləşdirmə pəncərələri olan kitab registrləri vasitəsilə idarə olunur; qovşaqlar naməlum və ya ləğv edilmiş qeydlərə istinad edən sübutları təsdiqləməkdən imtina edir.
- Konsensus başlıqları aktiv məxfi xüsusiyyət həzminə əməl edir ki, bloklar yalnız reyestr və parametr vəziyyəti uyğunlaşdıqda qəbul edilir.
- Proof konstruksiya etibarlı quraşdırma olmadan Halo2 (Plonkish) yığınından istifadə edir; Groth16 və ya digər SNARK variantları v1-də qəsdən dəstəklənmir.

### Deterministik qurğular

Məxfi memo zərfləri indi `fixtures/confidential/encrypted_payload_v1.json`-də kanonik qurğu ilə göndərilir. Verilənlər dəsti müsbət v1 zərfini və mənfi formalaşdırılmamış nümunələri tutur ki, SDK-lar təhlil paritetini təsdiq edə bilsin. Rust data-model testləri (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) və Swift dəsti (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) həm qurğunu birbaşa yükləyir, həm də Norito kodlaşdırması, xəta səthləri və reqressiya əhatə dairəsi kodek inkişaf etdikcə uyğunlaşmağa zəmanət verir.

Swift SDK-ları indi sifarişli JSON yapışdırıcısı olmadan qalxan təlimatları yaya bilər:
32 baytlıq qeyd öhdəliyi, şifrələnmiş faydalı yük və debet metadata ilə `ShieldRequest`,
sonra imzalamaq və ötürmək üçün `IrohaSDK.submit(shield:keypair:)` (və ya `submitAndWait`) zəng edin.
`/v1/pipeline/transactions` üzərində əməliyyat. Köməkçi öhdəlik uzunluqlarını təsdiqləyir,
`ConfidentialEncryptedPayload`-i Norito kodlayıcısına keçirin və `zk::Shield`-i əks etdirir
cüzdanlar Rust ilə kilidli addımda qalması üçün aşağıda təsvir edilən layout.## Konsensus Öhdəlikləri və Bacarıq Gating
- Blok başlıqları `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`-i ifşa edir; həzm konsensus hashında iştirak edir və blokun qəbulu üçün yerli reyestr görünüşünə bərabər olmalıdır.
- İdarəetmə `next_conf_features`-i gələcək `activation_height` ilə proqramlaşdırmaqla təkmilləşdirmələri həyata keçirə bilər; o yüksəkliyə qədər, blok istehsalçıları əvvəlki həzm yaymağa davam etməlidirlər.
- Validator qovşaqları `confidential.enabled = true` və `assume_valid = false` ilə işləməlidir. Hər iki şərt uğursuz olarsa və ya yerli `conf_features` fərqli olarsa, başlanğıc yoxlamaları validator dəstinə qoşulmaqdan imtina edir.
- P2P əl sıxma metadatasına indi `{ enabled, assume_valid, conf_features }` daxildir. Dəstəklənməyən funksiyaları reklam edən həmyaşıdlar `HandshakeConfidentialMismatch` ilə rədd edilir və heç vaxt konsensus rotasiyasına daxil olmurlar.
- Validator olmayan müşahidəçilər `assume_valid = true` təyin edə bilərlər; onlar məxfi deltaları kor-koranə tətbiq edirlər, lakin konsensusun təhlükəsizliyinə təsir göstərmirlər.## Aktiv Siyasətləri
- Hər bir aktiv tərifi yaradıcı tərəfindən və ya idarəetmə vasitəsilə təyin edilmiş `AssetConfidentialPolicy`-i daşıyır:
  - `TransparentOnly`: standart rejim; yalnız şəffaf təlimatlara (`MintAsset`, `TransferAsset` və s.) icazə verilir və qorunan əməliyyatlar rədd edilir.
  - `ShieldedOnly`: bütün emissiya və köçürmələr məxfi təlimatlardan istifadə etməlidir; `RevealConfidential` qadağandır, ona görə də tarazlıqlar heç vaxt ictimaiyyətə görünmür.
  - `Convertible`: sahiblər aşağıdakı on/off-ramp təlimatlarından istifadə edərək şəffaf və qorunan təsvirlər arasında dəyəri köçürə bilərlər.
- Siyasətlər qapalı vəsaitlərin qarşısını almaq üçün məhdud FSM-ə əməl edir:
  - `TransparentOnly → Convertible` (qorlanmış hovuzun dərhal işə salınması).
  - `TransparentOnly → ShieldedOnly` (gözləyən keçid və dönüşüm pəncərəsi tələb olunur).
  - `Convertible → ShieldedOnly` (məcburi minimum gecikmə).
  - `ShieldedOnly → Convertible` (miqrasiya planı tələb olunur ki, qorunan qeydlər xərclənə bilsin).
  - `ShieldedOnly → TransparentOnly` qorunan hovuz boş olduqda və ya idarəetmə görkəmli qeydləri qoruyan miqrasiyanı kodlaşdırmadıqda icazə verilmir.
- İdarəetmə təlimatları `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }`-i `ScheduleConfidentialPolicyTransition` ISI vasitəsilə təyin edir və `CancelConfidentialPolicyTransition` ilə planlaşdırılan dəyişiklikləri ləğv edə bilər. Mempool yoxlanışı heç bir əməliyyatın keçid hündürlüyündən kənara çıxmamasını təmin edir və siyasət yoxlanışı blokun ortasında dəyişərsə, daxiletmə qəti şəkildə uğursuz olur.
- Gözləyən keçidlər yeni blok açıldıqda avtomatik olaraq tətbiq edilir: blok hündürlüyü dönüşüm pəncərəsinə daxil olduqda (`ShieldedOnly` təkmilləşdirmələri üçün) və ya proqramlaşdırılmış `effective_height` səviyyəsinə çatdıqda, icra müddəti `AssetConfidentialPolicy` yenilənir, `AssetConfidentialPolicy` yenilənir, Prometheus-u təzələyir və metatatları silir. `ShieldedOnly` keçidi yetişəndə ​​şəffaf təchizat qalırsa, icra müddəti dəyişikliyi dayandırır və əvvəlki rejimi toxunulmaz qoyaraq xəbərdarlıq edir.
- `policy_transition_delay_blocks` və `policy_transition_window_blocks` konfiqurasiya düymələri pul kisələrinin keçid ətrafında qeydləri çevirməsinə imkan vermək üçün minimum bildiriş və güzəşt müddətlərini tətbiq edir.
- `pending_transition.transition_id` audit dəstəyi funksiyasını yerinə yetirir; Keçidləri yekunlaşdırarkən və ya ləğv edərkən idarəetmə ondan sitat gətirməlidir ki, operatorlar enişdən kənar hesabatları əlaqələndirə bilsinlər.
- `policy_transition_window_blocks` defolt olaraq 720 (≈60 s blok müddətində ≈12 saat). Qovşaqlar daha qısa bildiriş cəhdi edən idarəetmə sorğularını sıxışdırır.
- Yaradılış təzahür edir və CLI cari və gözlənilən siyasətləri səthə gətirir. Qəbul məntiqi hər bir məxfi təlimatın icazəli olduğunu təsdiqləmək üçün icra zamanı siyasəti oxuyur.
- Miqrasiya yoxlama siyahısı — Milestone M0-un izlədiyi mərhələli təkmilləşdirmə planı üçün aşağıdakı “Miqrasiya ardıcıllığı”na baxın.

#### Torii vasitəsilə keçidlərin monitorinqiPul kisələri və auditorlar yoxlamaq üçün `GET /v1/confidential/assets/{definition_id}/transitions` sorğusu keçirirlər
aktiv `AssetConfidentialPolicy`. JSON yükünə həmişə kanonik daxildir
aktiv id, ən son müşahidə edilən blok hündürlüyü, siyasətin `current_mode`, rejim
həmin hündürlükdə qüvvəyə minir (dönüşüm pəncərələri müvəqqəti olaraq `Convertible` məlumat verir) və
gözlənilən `vk_set_hash`/Poseidon/Pedersen parametr identifikatorları. Swift SDK istehlakçıları zəng edə bilər
`ToriiClient.getConfidentialAssetPolicy` olmadan yazılmış DTO-larla eyni məlumatları qəbul etmək
əl ilə yazılmış dekodlaşdırma. İdarəetmə keçidi gözlənildikdə cavab həmçinin aşağıdakıları ehtiva edir:

- `transition_id` — `ScheduleConfidentialPolicyTransition` tərəfindən qaytarılan audit dəstəyi.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` və əldə edilən `window_open_height` (pul kisələrinin lazım olduğu blok
  ShieldedOnly cut-overs üçün çevrilməyə başlayın).

Cavab nümunəsi:

```json
{
  "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

`404` cavabı heç bir uyğun aktiv tərifinin olmadığını göstərir. Heç bir keçid olmadıqda
planlaşdırılmış `pending_transition` sahəsi `null`-dir.

### Siyasət dövlət maşını| Cari rejim | Növbəti rejim | İlkin şərtlər | Effektiv yüksəklikdə işləmə | Qeydlər |
|--------------------|------------------|-----------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------|
| TransparentOnly | Kabriolet | İdarəetmə yoxlayıcı/parametr reyestrinin qeydlərini aktivləşdirib. `ScheduleConfidentialPolicyTransition` ilə `effective_height ≥ current_height + policy_transition_delay_blocks` təqdim edin. | Keçid tam olaraq `effective_height`-də yerinə yetirilir; qorunan hovuz dərhal istifadəyə verilir.                   | Şəffaf axınları qoruyarkən məxfiliyi təmin etmək üçün defolt yol.               |
| TransparentOnly | ShieldedOnly | Yuxarıdakı kimi, üstəgəl `policy_transition_window_blocks ≥ 1`.                                                         | `effective_height - policy_transition_window_blocks`-də iş vaxtı avtomatik olaraq `Convertible`-ə daxil olur; `effective_height`-də `ShieldedOnly`-ə çevrilir. | Şəffaf təlimatlar deaktiv edilməzdən əvvəl deterministik çevirmə pəncərəsini təmin edir.   |
| Kabriolet | ShieldedOnly | `effective_height ≥ current_height + policy_transition_delay_blocks` ilə planlaşdırılmış keçid. İdarəetmə audit metadatası vasitəsilə (`transparent_supply == 0`) sertifikatlaşdırmalı olmalıdır; iş vaxtı bunu kəsmə zamanı tətbiq edir. | Yuxarıdakı kimi eyni pəncərə semantikası. `effective_height`-də şəffaf təchizat sıfırdan fərqlidirsə, keçid `PolicyTransitionPrerequisiteFailed` ilə dayandırılır. | Aktivi tam məxfi dövriyyəyə kilidləyir.                                     |
| ShieldedOnly | Kabriolet | Planlaşdırılmış keçid; aktiv təcili geri çəkilmə yoxdur (`withdraw_height` qurulmayıb).                                    | `effective_height`-də vəziyyət çevrilir; qorunan qeydlər etibarlı qalarkən aşkar rampalar yenidən açılır.                           | Baxım pəncərələri və ya auditor rəyləri üçün istifadə olunur.                                          |
| ShieldedOnly | TransparentOnly | İdarəetmə `shielded_supply == 0`-i sübut etməli və ya imzalanmış `EmergencyUnshield` planını hazırlamalıdır (auditor imzaları tələb olunur). | Runtime `effective_height`-dən əvvəl `Convertible` pəncərəsini açır; yüksəklikdə məxfi təlimatlar uğursuz olur və aktiv yalnız şəffaf rejimə qayıdır. | Son çıxış yolu. Pəncərə zamanı hər hansı məxfi qeyd sərf edilərsə, keçid avtomatik olaraq ləğv edilir. |
| İstənilən | İndiki ilə eyni | `CancelConfidentialPolicyTransition` gözlənilən dəyişikliyi təmizləyir.                                                        | `pending_transition` dərhal çıxarıldı.                                                                          | Status-kvonu saxlayır; tamlığı üçün göstərilir.                                             |Yuxarıda sadalanmayan keçidlər idarəetmə təqdimatı zamanı rədd edilir. İş vaxtı planlaşdırılmış keçidi tətbiq etməzdən əvvəl ilkin şərtləri yoxlayır; ilkin şərtlərin uğursuzluğu aktivi əvvəlki rejiminə qaytarır və telemetriya və blok hadisələri vasitəsilə `PolicyTransitionPrerequisiteFailed` yayır.

### Miqrasiya ardıcıllığı

2. **Keçid mərhələsi:** `ScheduleConfidentialPolicyTransition`-i `policy_transition_delay_blocks`-ə hörmət edən `effective_height` ilə təqdim edin. `ShieldedOnly`-ə doğru hərəkət edərkən, bir çevirmə pəncərəsini (`window ≥ policy_transition_window_blocks`) göstərin.
3. **Operator təlimatını dərc edin:** Qaytarılan `transition_id`-i qeyd edin və eniş/düşmə kitabçasını yayımlayın. Pul kisələri və auditorlar pəncərənin açıq hündürlüyünü öyrənmək üçün `/v1/confidential/assets/{id}/transitions`-ə abunə olurlar.
4. **Pəncərənin tətbiqi:** Pəncərə açıldığında, icra müddəti siyasəti `Convertible`-ə keçir, `PolicyTransitionWindowOpened { transition_id }` yayır və ziddiyyətli idarəetmə sorğularını rədd etməyə başlayır.
5. **Sonlaşdırın və ya dayandırın:** `effective_height`-də icra müddəti keçid ilkin şərtlərini yoxlayır (sıfır şəffaf təchizat, fövqəladə hallarda geri çəkilmə və s.). Uğur siyasəti tələb olunan rejimə çevirir; uğursuzluq `PolicyTransitionPrerequisiteFailed` yayır, gözlənilən keçidi təmizləyir və siyasəti dəyişməz qoyur.
6. **Sxem təkmilləşdirmələri:** Uğurlu keçiddən sonra idarəetmə aktivin sxem versiyasına (məsələn, `asset_definition.v2`) zərbə vurur və manifestləri seriallaşdırarkən CLI alətləri `confidential_policy` tələb edir. Genesis təkmilləşdirmə sənədləri operatorlara təsdiqləyiciləri yenidən işə salmazdan əvvəl siyasət parametrləri və reyestr barmaq izlərini əlavə etməyi tapşırır.

Məxfiliyin aktivləşdirilməsi ilə başlayan yeni şəbəkələr istənilən siyasəti birbaşa başlanğıcda kodlayır. Onlar hələ də işə salındıqdan sonra rejimləri dəyişdirərkən yuxarıdakı yoxlama siyahısına əməl edirlər ki, konversiya pəncərələri deterministik olaraq qalsın və pul kisələrinin tənzimləmək üçün vaxtı olsun.

### Norito manifest versiyası və aktivləşdirilməsi- Yaradılış manifestlərinə xüsusi `confidential_registry_root` açarı üçün `SetParameter` daxil olmalıdır. Faydalı yük Norito `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`-ə uyğun JSON-dur: heç bir doğrulayıcı daxiletmə aktiv olmadıqda sahəni buraxın (`null`), əks halda 32 baytlıq hex sətri (`0x…`) təmin edin: manifestdə göndərilən doğrulama təlimatları. Parametr yoxdursa və ya hash kodlanmış reyestr yazıları ilə razılaşmırsa, qovşaqlar başlamaqdan imtina edir.
- On-wire `ConfidentialFeatureDigest::conf_rules_version` manifest layout versiyasını daxil edir. v1 şəbəkələri üçün o, `Some(1)` olaraq qalmalıdır və `iroha_config::parameters::defaults::confidential::RULES_VERSION`-ə bərabərdir. Qaydalar dəsti təkamül etdikdə, sabiti döyün, manifestləri bərpa edin və ikili faylları kilidləmə addımında yaymaq; versiyaların qarışdırılması validatorların `ConfidentialFeatureDigestMismatch` ilə blokları rədd etməsinə səbəb olur.
- Aktivləşdirmə təzahür edir ki, həzm ardıcıl olaraq qalsın, beləliklə reyestr yeniləmələrini, parametrlərin həyat dövrü dəyişikliklərini və siyasət keçidlərini GÖRMƏLİDİR:
  1. Planlaşdırılmış reyestr mutasiyalarını (`Publish*`, `Set*Lifecycle`) oflayn vəziyyət görünüşündə tətbiq edin və `compute_confidential_feature_digest` ilə aktivləşdirmədən sonrakı həzmini hesablayın.
  2. Hesablanmış hash-dən istifadə edərək `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})`-i buraxın ki, geridə qalan həmyaşıdlar hətta aralıq reyestr təlimatlarını qaçırsalar belə, düzgün həzmi bərpa edə bilsinlər.
  3. `ScheduleConfidentialPolicyTransition` təlimatlarını əlavə edin. Hər bir təlimat idarəetmə tərəfindən verilmiş `transition_id`-dən sitat gətirməlidir; unudduğu təzahürlər icra zamanı tərəfindən rədd ediləcək.
  4. Aktivləşdirmə planında istifadə edilən manifest baytlarını, SHA-256 barmaq izini və həzmi davam etdirin. Operatorlar arakəsmələrin qarşısını almaq üçün manifestə səs verməzdən əvvəl hər üç artefaktı yoxlayır.
- Yayımlamalar təxirə salınmış kəsmə tələb etdikdə, hədəf hündürlüyünü köməkçi fərdi parametrdə qeyd edin (məsələn, `custom.confidential_upgrade_activation_height`). Bu, auditorlara Norito kodlu sübut verir ki, təsdiqləyicilər həzm dəyişikliyi qüvvəyə minməzdən əvvəl bildiriş pəncərəsinə hörmət ediblər.## Doğrulayıcı və Parametrlərin Həyat Dövrü
### ZK Qeydiyyatı
- Ledger `proving_system`-in hazırda `Halo2`-də sabit olduğu `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }`-ni saxlayır.
- `(circuit_id, version)` cütləri qlobal miqyasda unikaldır; reyestr dövrə metaməlumatları üzrə axtarışlar üçün ikinci dərəcəli indeks saxlayır. Qəbul zamanı dublikat cütü qeydiyyatdan keçirmək cəhdləri rədd edilir.
- `circuit_id` boş olmamalıdır və `public_inputs_schema_hash` təmin edilməlidir (adətən doğrulayıcının kanonik ictimai giriş kodlaşdırmasının Blake2b-32 heşidir). Qəbul bu sahələri buraxan qeydləri rədd edir.
- İdarəetmə təlimatlarına aşağıdakılar daxildir:
  - Yalnız metadata ilə `Proposed` girişini əlavə etmək üçün `PUBLISH`.
  - `ACTIVATE { vk_id, activation_height }` bir dövr sərhədində giriş aktivasiyasını planlaşdırmaq üçün.
  - `DEPRECATE { vk_id, deprecation_height }`, sübutların girişə istinad edə biləcəyi son hündürlüyü qeyd etmək üçün.
  - təcili söndürmə üçün `WITHDRAW { vk_id, withdraw_height }`; təsirə məruz qalan aktivlər yeni girişlər aktivləşənə qədər geri çəkilmə hündürlüyündən sonra məxfi xərcləri dondurur.
- Yaradılış `vk_set_hash` aktiv qeydlərə uyğun gələn `confidential_registry_root` xüsusi parametrini avtomatik yayır; Doğrulama, node konsensusa qoşulmazdan əvvəl bu həzmi yerli reyestr vəziyyəti ilə müqayisə edir.
- Doğrulayıcının qeydiyyatı və ya yenilənməsi üçün `gas_schedule_id` tələb olunur; yoxlama reyestr qeydinin `Active` olduğunu, `(circuit_id, version)` indeksində olduğunu və Halo2 sübutlarının `circuit_id`, `circuit_id`, `circuit_id`, Prometheus000, Prometheus000101010101 ilə uyğun gələn `OpenVerifyEnvelope` təmin etdiyini məcbur edir. reyestr qeydi.

### Sübut edən açarlar
- Təsdiq açarları kitabdan kənar olaraq qalır, lakin yoxlayıcı metadata ilə birlikdə dərc edilmiş məzmun ünvanlı identifikatorlar (`pk_cid`, `pk_hash`, `pk_len`) tərəfindən istinad edilir.
- Pulqabı SDK-ları PK məlumatlarını alır, hashları yoxlayır və yerli olaraq keşləyir.

### Pedersen və Poseidon Parametrləri
- Ayrı-ayrı registrlər (`PedersenParams`, `PoseidonParams`) güzgü yoxlayıcı həyat dövrü nəzarətləri, hər biri `params_id`, generatorların/sabitlərin heşləri, aktivləşdirmə, köhnəlmə və geri çəkilmə yüksəklikləri.

## Deterministik Sifariş və Nullifiers
- Hər bir aktiv `next_leaf_index` ilə `CommitmentTree` saxlayır; bloklar deterministik qaydada öhdəlikləri əlavə edir: əməliyyatları blok qaydasında təkrarlayın; hər bir tranzaksiya daxilində seriallaşdırılmış `output_idx` yüksəlməklə qorunan çıxışları təkrarlayın.
- `note_position` ağac ofsetlərindən alınmışdır, lakin ləğvedicinin ** hissəsi deyil; yalnız sübut şahidi daxilində üzvlük yollarını qidalandırır.
- Yenidən qurulma zamanı nullifier sabitliyi PRF dizaynı ilə təmin edilir; PRF girişi `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`-i bağlayır və `max_anchor_age_blocks` ilə məhdudlaşdırılan tarixi Merkle köklərinə istinad edir.## Ledger axını
1. **MintConfidential { aktiv_id, məbləğ, alıcı_göstəricisi }**
   - `Convertible` və ya `ShieldedOnly` aktiv siyasətini tələb edir; qəbul aktiv orqanı yoxlayır, cari `params_id`, nümunələr `rho`, öhdəlik buraxır, Merkle ağacını yeniləyir.
   - Yeni öhdəlik, Merkle kök deltası və audit yolları üçün əməliyyat çağırışı hash ilə `ConfidentialEvent::Shielded` yayır.
2. **TransferConfidential { aktiv_id, sübut, dövrə_id, versiya, ləğvedicilər, yeni_öhdəliklər, enc_payloads, anchor_root, memo }**
   - VM syscall reyestr girişindən istifadə edərək sübutu yoxlayır; host ləğvedicilərin istifadə olunmamasını, öhdəliklərin deterministik şəkildə əlavə olunmasını təmin edir, lövbər yenidir.
   - Ledger `NullifierSet` qeydlərini qeyd edir, alıcılar/auditorlar üçün şifrələnmiş faydalı yükləri saxlayır və ləğvediciləri, sifarişli çıxışları, sübut hashını və Merkle köklərini ümumiləşdirən `ConfidentialEvent::Transferred` yayır.
3. **RevealConfidential { aktiv_id, sübut, dövrə_id, versiya, ləğvedici, məbləğ, alıcı_hesabı, anchor_root }**
   - Yalnız `Convertible` aktivləri üçün əlçatandır; sübut, qeydin dəyərinin aşkar edilmiş məbləğə bərabər olduğunu təsdiqləyir, mühasibat kitabı şəffaf balansı kreditləşdirir və xərclənmiş ləğvedicini qeyd etməklə qorunan notu yandırır.
   - İctimai məbləğ, istehlak edilən ləğvedicilər, sübut identifikatorları və əməliyyat çağırışı hash ilə `ConfidentialEvent::Unshielded` yayır.

## Məlumat Modeli Əlavələri
- Aktivləşdirmə bayrağı ilə `ConfidentialConfig` (yeni konfiqurasiya bölməsi), `assume_valid`, qaz/limit düymələri, lövbər pəncərəsi, doğrulayıcı arxa ucu.
- Açıq versiya baytı olan `ConfidentialNote`, `ConfidentialTransfer` və `ConfidentialMint` Norito sxemləri (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload`, XChaCha20-Poly1305 tərtibatı üçün defolt olaraq `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` olaraq `{ version, ephemeral_pubkey, nonce, ciphertext }` ilə AEAD memo baytlarını əhatə edir.
- Kanonik açar törəmə vektorları `docs/source/confidential_key_vectors.json`-də yaşayır; həm CLI, həm də Torii son nöqtəsi bu qurğulara qarşı geriləyir. Xərcləmə/nullifier/baxış nərdivanı üçün pul kisəsi ilə əlaqəli törəmələr `fixtures/confidential/keyset_derivation_v1.json`-də dərc olunur və dillər arası paritetə ​​zəmanət vermək üçün Rust + Swift SDK testləri ilə həyata keçirilir.
- `asset::AssetDefinition` `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }` qazanır.
- `ZkAssetState` köçürmə/ekrandan çıxarma yoxlayıcıları üçün `(backend, name, commitment)` bağlamasını davam etdirir; icra, istinad edilmiş və ya daxili yoxlama açarı qeydə alınmış öhdəliyə uyğun gəlməyən sübutları rədd edir və mutasiya vəziyyətindən əvvəl həll edilmiş arxa uç açarına qarşı köçürmə/açma sübutlarını yoxlayır.
- `CommitmentTree` (sərhəd keçid məntəqələri olan aktivə görə), `NullifierSet`, `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` dünyada saxlanılır.
- Mempool dublikatın erkən aşkarlanması və lövbər yaşının yoxlanılması üçün keçici `NullifierIndex` və `AnchorIndex` strukturlarını saxlayır.
- Norito sxem yeniləmələrinə ictimai girişlər üçün kanonik sifariş daxildir; gediş-gəliş testləri kodlaşdırma determinizmini təmin edir.
- Şifrələnmiş faydalı yükün gediş-gəlişi vahid testləri (`crates/iroha_data_model/src/confidential.rs`) vasitəsilə kilidlənir və yuxarıdakı pul kisəsi açarının törəmə vektorları auditorlar üçün AEAD zərfinin törəmələrini birləşdirir. `norito.md` zərf üçün naqil başlığını sənədləşdirir.## IVM İnteqrasiya və Syscall
- Qəbul edən `VERIFY_CONFIDENTIAL_PROOF` sistem çağırışını təqdim edin:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` və nəticədə `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - Syscall reyestrdən yoxlayıcı metadata yükləyir, ölçü/vaxt məhdudiyyətlərini tətbiq edir, deterministik qazı hesablayır və yalnız sübut uğurlu olarsa, delta tətbiq edir.
- Host Merkle kök snapshotlarını və sıfırlayıcı statusunu əldə etmək üçün yalnız oxumaq üçün `ConfidentialLedger` xüsusiyyətini ifşa edir; Kotodama kitabxanası şahid montaj köməkçiləri və şemanın təsdiqlənməsini təmin edir.
- Göstərici-ABI sənədləri sübut bufer tərtibatını və reyestr tutacaqlarını aydınlaşdırmaq üçün yeniləndi.

## Düyün Qabiliyyəti Danışığı
- Handshake `feature_bits.confidential`-i `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` ilə birlikdə reklam edir. Validatorun iştirakı üçün `confidential.enabled=true`, `assume_valid=false`, eyni doğrulayıcı arxa uç identifikatorları və uyğun həzmlər tələb olunur; uyğunsuzluqlar `HandshakeConfidentialMismatch` ilə əl sıxışmağı bacarmır.
- Konfiqurasiya yalnız müşahidəçi qovşaqları üçün `assume_valid` dəstəkləyir: deaktiv edildikdə, məxfi təlimatlarla qarşılaşmaq panik olmadan deterministik `UnsupportedInstruction` verir; aktiv olduqda, müşahidəçilər sübutları yoxlamadan elan edilmiş dövlət deltalarını tətbiq edirlər.
- Mempool yerli imkanlar deaktiv edilərsə, məxfi əməliyyatları rədd edir. Qeybət filtrləri ölçü hədləri daxilində naməlum doğrulayıcı identifikatorları kor yönləndirərkən, uyğun qabiliyyət olmadan qorunan əməliyyatları həmyaşıdlarına göndərməkdən çəkinir.

### Budama və Nullifier Saxlama Siyasətini aşkar edin

Məxfi dəftərlər qeydin təzəliyini sübut etmək üçün kifayət qədər tarixi saxlamalıdır
idarəetməyə əsaslanan auditləri təkrarlayın. Defolt siyasət, tərəfindən tətbiq edilir
`ConfidentialLedger`, bu:

- **Nullifier saxlama:** sərf edilmiş ləğvediciləri *minimum* `730` gün ərzində saxlayın (24
  ay) xərcləmə hündürlüyündən sonra və ya daha uzunsa, tənzimləyici tərəfindən təyin olunan pəncərə.
  Operatorlar pəncərəni `confidential.retention.nullifier_days` vasitəsilə genişləndirə bilərlər.
  Saxlama pəncərəsindən daha gənc nullifiers Torii vasitəsilə sorğulanmalıdır.
  auditorlar ikiqat xərclərin olmamasını sübut edə bilərlər.
- **Budamanı aşkar edin:** şəffaf aşkar edir (`RevealConfidential`) budama
  blok başa çatdıqdan dərhal sonra əlaqəli qeyd öhdəlikləri, lakin
  istehlak edilən sıfırlayıcı yuxarıdakı saxlama qaydasına tabe olaraq qalır. Açıqlama ilə bağlıdır
  hadisələr (`ConfidentialEvent::Unshielded`) ictimai məbləği, alıcını,
  və sübut hash belə rekonstruksiya tarixi ortaya budama tələb etmir
  şifrəli mətn.
- **Sərhəd keçid məntəqələri:** öhdəlik sərhədləri yuvarlanan yoxlama məntəqələrini saxlayır
  daha böyük `max_anchor_age_blocks` və saxlama pəncərəsini əhatə edir. Düyünlər
  köhnə yoxlama məntəqələrini yalnız intervaldakı bütün ləğvedicilərin müddəti bitdikdən sonra yığcamlaşdırın.
- **Köhnə sindirim bərpası:** əgər `HandshakeConfidentialMismatch` səbəbiylə yüksəldilibsə
  drifti həzm etmək üçün operatorlar (1) nullifier saxlama pəncərələrini yoxlamalıdırlar
  klaster üzrə hizalayın, (2) `iroha_cli app confidential verify-ledger`-i işə salın
  saxlanılan nullifier dəstinə qarşı həzmi bərpa edin və (3) yenidən yerləşdirin
  yenilənmiş manifest. Vaxtından əvvəl kəsilmiş hər hansı nullifiers bərpa edilməlidir
  şəbəkəyə yenidən qoşulmazdan əvvəl soyuq anbar.Əməliyyatlar kitabında yerli ləğvləri sənədləşdirin; idarəetmə siyasətini genişləndirir
saxlama pəncərəsi qovşaq konfiqurasiyasını və arxiv saxlama planlarını yeniləməlidir
kilid addımı.

### Evdən çıxarma və Bərpa axını

1. Yığma zamanı `IrohaNetwork` reklam edilən imkanları müqayisə edir. Hər hansı uyğunsuzluq `HandshakeConfidentialMismatch` artırır; əlaqə bağlanır və həmyaşıd heç vaxt `Ready`-ə yüksəlmədən kəşf növbəsində qalır.
2. Uğursuzluq şəbəkə xidməti jurnalı (uzaqdan həzm və arxa plan daxil olmaqla) vasitəsilə aşkarlanır və Sumeragi heç vaxt həmyaşıdları təklif və ya səsvermə üçün planlaşdırmır.
3. Operatorlar yoxlama registrlərini və parametr dəstlərini (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) uyğunlaşdırmaqla və ya `next_conf_features`-i razılaşdırılmış `next_conf_features` ilə uyğunlaşdırmaqla düzəliş edir. Həzm uyğunlaşdıqdan sonra növbəti əl sıxma avtomatik olaraq uğurlu olur.
4. Əgər köhnə həmyaşıd bloku yayımlamağı bacarırsa (məsələn, arxivin təkrarı vasitəsilə), validatorlar onu `BlockRejectionReason::ConfidentialFeatureDigestMismatch` ilə qəti şəkildə rədd edir, şəbəkə üzrə mühasibat dəftərinin vəziyyətini ardıcıl saxlayır.

### Təkrarla təhlükəsiz əl sıxma axını

1. Hər bir çıxış cəhdi yeni Səs/X25519 əsas materialı ayırır. İmzalanmış əl sıxma yükü (`handshake_signature_payload`) yerli və uzaq efemer açıq açarları, Norito kodlu reklam edilmiş rozetka ünvanını və `handshake_chain_id` ilə tərtib edildikdə zəncir identifikatorunu birləşdirir. Mesaj qovşağı tərk etməzdən əvvəl AEAD ilə şifrələnir.
2. Cavab verən həmyaşıd/yerli açar sırasını tərsinə çevirməklə faydalı yükü yenidən hesablayır və `HandshakeHelloV1`-ə daxil edilmiş Ed25519 imzasını yoxlayır. Həm efemer açarlar, həm də reklam edilən ünvan imza domeninin bir hissəsi olduğundan, tutulan mesajı digər həmyaşıdlara qarşı təkrar oxutmaq və ya köhnə əlaqəni bərpa etmək deterministik şəkildə yoxlanışı uğursuz edir.
3. Məxfi qabiliyyət bayraqları və `ConfidentialFeatureDigest` `HandshakeConfidentialMeta` daxilində hərəkət edir. Qəbuledici `{ enabled, assume_valid, verifier_backend, digest }` konfiqurasiyasını yerli olaraq konfiqurasiya edilmiş `ConfidentialHandshakeCaps` ilə müqayisə edir; hər hansı uyğunsuzluq `Ready`-ə nəqliyyat keçidindən əvvəl `HandshakeConfidentialMismatch` ilə erkən çıxır.
4. Operatorlar yenidən qoşulmadan əvvəl həzmi (`compute_confidential_feature_digest` vasitəsilə) yenidən hesablamalı və qovşaqları yenilənmiş reyestrlər/siyasətlərlə yenidən işə salmalıdırlar. Köhnə həzmləri reklam edən həmyaşıdlar əl sıxışdırmada uğursuzluqla davam edir, köhnə vəziyyətin validator dəstinə yenidən daxil olmasına mane olur.
5. Əl sıxma uğurları və uğursuzluqları standart `iroha_p2p::peer` sayğaclarını (`handshake_failure_count`, səhv taksonomiyası köməkçiləri) güncəlləşdirir və uzaqdan həmyaşıd identifikatoru və həzm barmaq izi ilə işarələnmiş strukturlaşdırılmış jurnal qeydlərini yayır. Yayım zamanı təkrar oxutma cəhdlərini və ya yanlış konfiqurasiyaları tutmaq üçün bu göstəricilərə nəzarət edin.## Açar İdarəetmə və Yükləmələr
- Hər hesaba görə əsas törəmə iyerarxiyası:
  - `sk_spend` → `nk` (nülledici açar), `ivk` (daxil olan baxış açarı), `ovk` (gidən baxış açarı), `fvk`.
- Şifrələnmiş qeyd yükləri ECDH-dən əldə edilən paylaşılan açarlarla AEAD-dan istifadə edir; isteğe bağlı auditor baxış açarları aktiv siyasətinə görə çıxışlara əlavə edilə bilər.
- CLI əlavələri: `confidential create-keys`, `confidential send`, `confidential export-view-key`, qeydlərin şifrəsini açmaq üçün auditor aləti və Norito oflayn zərfləri hazırlamaq/yoxlamaq üçün `iroha app zk envelope` köməkçisi. Torii `POST /v1/confidential/derive-keyset` vasitəsilə eyni törəmə axınını ifşa edir, həm hex, həm də base64 formalarını qaytarır, beləliklə, pul kisələri proqramlı şəkildə əsas iyerarxiyaları əldə edə bilsin.

## Qaz, Limitlər və DoS Nəzarətləri
- Deterministik qaz cədvəli:
  - Halo2 (Plonkish): əsas `250_000` qaz + `2_000` hər ictimai giriş üçün qaz.
  - Hər bir sübut baytı üçün `5` qaz, üstəgəl nulledici (`300`) və öhdəlik üzrə (`500`) ödənişlər.
  - Operatorlar qovşaq konfiqurasiyası (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`) vasitəsilə bu sabitləri ləğv edə bilər; dəyişikliklər başlanğıcda və ya konfiqurasiya qatı isti yenidən yükləndikdə yayılır və klasterdə deterministik şəkildə tətbiq olunur.
- Sərt məhdudiyyətlər (konfiqurasiya edilə bilən standart):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. `verify_timeout_ms`-dən çox olan sübutlar təlimatı qəti şəkildə dayandırır (idarəetmə bülletenləri `proof verification exceeded timeout` buraxır, `VerifyProof` xəta qaytarır).
- Əlavə kvotalar canlılığı təmin edir: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block` və `max_public_inputs` bağlı blok qurucuları; `reorg_depth_bound` (≥ `max_anchor_age_blocks`) sərhəd keçid məntəqəsinin saxlanmasını tənzimləyir.
- İcra müddətinin icrası indi bu əməliyyat başına və ya blok üzrə limitləri aşan əməliyyatları rədd edir, deterministik `InvalidParameter` xətaları yaradır və mühasibat uçotu vəziyyətini dəyişməz qoyur.
- Mempool resurs istifadəsini məhdud saxlamaq üçün doğrulayıcıya müraciət etməzdən əvvəl məxfi əməliyyatları `vk_id`, sübut uzunluğu və lövbər yaşı ilə əvvəlcədən filtrləyir.
- Yoxlama fasiləsi və ya həddi pozulduğunda müəyyən şəkildə dayandırılır; əməliyyatlar açıq səhvlərlə uğursuz olur. SIMD arxa uçları isteğe bağlıdır, lakin qaz uçotunu dəyişdirmir.

### Kalibrləmə Bazaları və Qəbul Qapıları
- **İstinad platformaları.** Kalibrləmə işləri aşağıdakı üç aparat profilini əhatə etməlidir. Bütün profilləri tuta bilməyən qaçışlar nəzərdən keçirilərkən rədd edilir.| Profil | Memarlıq | CPU / Nümunə | Kompilyator bayraqları | Məqsəd |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) və ya Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Vektor intrinsics olmadan mərtəbə dəyərləri qurmaq; ehtiyat xərcləri cədvəllərini tənzimləmək üçün istifadə olunur. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | standart buraxılış | AVX2 yolunu təsdiq edir; SIMD sürətləndiricilərinin neytral qazın tolerantlığı daxilində qaldığını yoxlayır. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | standart buraxılış | NEON arxa ucunun deterministik qalmasını və x86 cədvəllərinə uyğun olmasını təmin edir. |

- **Benchmark qoşqu.** Bütün qaz kalibrləmə hesabatları aşağıdakılarla hazırlanmalıdır:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - Deterministik qurğunu təsdiqləmək üçün `cargo test -p iroha_core bench_repro -- --ignored`.
  - VM əməliyyat kodu xərcləri dəyişdikdə `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`.

- **Sabit təsadüfilik.** `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` skamyalarını işə salmazdan əvvəl ixrac edin ki, `iroha_test_samples::gen_account_in` deterministik `KeyPair::from_seed` yoluna keçsin. Qoşqu bir dəfə `IROHA_CONF_GAS_SEED_ACTIVE=…` çap edir; dəyişən yoxdursa, baxış uğursuz olmalıdır. İstənilən yeni kalibrləmə utilitləri köməkçi təsadüfiliyi təqdim edərkən bu env var-a hörmət etməyə davam etməlidir.

- **Nəticənin tutulması.**
  - Hər bir profil üçün Kriteriya xülasələrini (`target/criterion/**/raw.csv`) buraxılış artefaktına yükləyin.
  - Əldə edilmiş ölçüləri (`ns/op`, `gas/op`, `ns/gas`) `docs/source/confidential_assets_calibration.md`-də istifadə edilən git commit və kompilyator versiyası ilə birlikdə saxlayın.
  - Hər bir profil üçün son iki əsas xətti qoruyun; ən yeni hesabat təsdiq edildikdən sonra köhnə şəkilləri silin.

- **Qəbul tolerantlıqları.**
  - `baseline-simd-neutral` və `baseline-avx2` arasındakı qaz deltaları ≤ ±1,5% QALMALIDIR.
  - `baseline-simd-neutral` və `baseline-neon` arasındakı qaz deltaları ≤ ±2,0% QALMALIDIR.
  - Bu hədləri aşan kalibrləmə təklifləri ya cədvələ düzəlişlər, ya da uyğunsuzluğu və təsirin azaldılmasını izah edən RFC tələb edir.

- **Yoxlama siyahısını nəzərdən keçirin.** Təqdimatçılar aşağıdakılara cavabdehdirlər:
  - Kalibrləmə jurnalına `uname -a`, `/proc/cpuinfo` çıxarışları (model, addım) və `rustc -Vv` daxil olmaqla.
  - `IROHA_CONF_GAS_SEED`-in təsdiqlənməsi dəzgah çıxışında əks-səda verdi (skamyalar aktiv toxumu çap edir).
  - Kardiostimulyator və konfidensial yoxlayıcı funksiyanın güzgü istehsalının təmin edilməsi (Telemetriya ilə skamyalarda işləyərkən `--features confidential,telemetry`).

## Konfiqurasiya və Əməliyyatlar
- `iroha_config` `[confidential]` bölməsini qazanır:
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- Telemetry emits aggregate metrics: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, and `confidential_policy_transitions_total`, never exposing açıq mətn məlumatları.
- RPC səthləri:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`## Test Strategiyası
- Determinizm: bloklar daxilində təsadüfi əməliyyatların qarışdırılması eyni Merkle kökləri və ləğvedici dəstlər verir.
- Yenidən qurulma davamlılığı: lövbərlərlə çoxbloklu reorqları simulyasiya edin; nullifiers sabit qalır və köhnəlmiş lövbərlər rədd edilir.
- Qaz invariantları: SIMD sürətləndirilməsi olan və olmayan qovşaqlar arasında eyni qaz istifadəsini yoxlayın.
- Sərhəd testi: ölçüdə/qaz tavanlarında sübutlar, maksimum giriş/çıxış sayları, vaxt aşımı tətbiqi.
- Həyat dövrü: yoxlayıcı və parametrin aktivləşdirilməsi/köhnəlməsi üçün idarəetmə əməliyyatları, fırlanma xərci testləri.
- FSM siyasəti: icazə verilən/icazə verilməyən keçidlər, gözlənilən keçid gecikmələri və effektiv yüksəkliklər ətrafında mempooldan imtina.
- Fövqəladə hallar reyestrində: fövqəladə halların çıxarılması təsirə məruz qalmış aktivləri `withdraw_height`-də dondurur və sonra sübutları rədd edir.
- Qabiliyyət qapısı: uyğunsuz `conf_features` blokları olan validatorlar; `assume_valid=true` ilə müşahidəçilər konsensusa təsir etmədən davam edirlər.
- Dövlət ekvivalentliyi: validator/tam/müşahidəçi qovşaqları kanonik zəncirdə eyni vəziyyət kökləri yaradır.
- Mənfi fuzzing: səhv formalaşmış sübutlar, böyük ölçülü faydalı yüklər və ləğvedici toqquşmalar qəti şəkildə rədd edilir.

## Görkəmli İş
- Benchmark Halo2 parametr dəstləri (dövrə ölçüsü, axtarış strategiyası) və nəticələri kalibrləmə kitabçasına qeyd edin ki, qaz/taxtam defoltları növbəti `confidential_assets_calibration.md` yeniləməsi ilə birlikdə yenilənə bilsin.
- İdarəetmə layihəsi imzalandıqdan sonra təsdiq edilmiş iş axınını Torii-ə birləşdirərək auditorun açıqlaması siyasətlərini və əlaqəli seçmə-baxış API-lərini yekunlaşdırın.
- SDK icraçıları üçün zərf formatını sənədləşdirərək, çoxlu qəbuledici çıxışları və toplu qeydləri əhatə etmək üçün şahid şifrələmə sxemini genişləndirin.
- Sxemlərin, reyestrlərin və parametrlərin fırlanma prosedurlarının xarici təhlükəsizlik baxışını tapşırın və tapıntıları daxili audit hesabatlarının yanında arxivləşdirin.
- Pulqabı satıcılarının eyni attestasiya semantikasını həyata keçirə bilməsi üçün auditor xərclərinin uzlaşdırılması API-lərini təyin edin və baxış açarının əhatə dairəsi üzrə təlimatı dərc edin.## İcra mərhələsi
1. **Faza M0 — Dayandırılmış Sərtləşdirmə**
   - ✅ Nullifier hasilatı indi Poseidon PRF dizaynını izləyir (`nk`, `rho`, `asset_id`, `chain_id`) kitab yeniləmələrində tətbiq edilən deterministik öhdəlik sifarişi ilə.
   - ✅ İcra sübut ölçüsü hədlərini və hər əməliyyat/blok üzrə məxfi kvotaları tətbiq edir, deterministik səhvlərlə həddindən artıq büdcə əməliyyatlarını rədd edir.
   - ✅ P2P əl sıxma `ConfidentialFeatureDigest` (backend digest + registr barmaq izləri) reklam edir və `HandshakeConfidentialMismatch` vasitəsilə müəyyən uyğunsuzluqları aradan qaldırır.
   - ✅ Məxfi icra yollarında çaxnaşmaları aradan qaldırın və uyğunlaşma qabiliyyəti olmadan qovşaqlar üçün rol qapısı əlavə edin.
   - ⚪ Sərhəd keçid məntəqələri üçün yoxlayıcı vaxt aşımı büdcələrini və dərinlik sərhədlərini yenidən təşkil edin.
     - ✅ Tətbiq olunan büdcələrin yoxlanılması müddəti; `verify_timeout_ms`-dən çox olan sübutlar indi deterministik şəkildə uğursuz olur.
     - ✅ Sərhəd keçid məntəqələri indi `reorg_depth_bound` standartına hörmət edir, deterministik snapshotları saxlayaraq konfiqurasiya edilmiş pəncərədən köhnə yoxlama məntəqələrini kəsir.
   - `AssetConfidentialPolicy`, siyasət FSM və nanə/ötürmə/aşkar göstərişlər üçün icra qapılarını təqdim edin.
   - Blok başlıqlarında `conf_features`-ə əməl edin və reyestr/parametr həzmləri fərqli olduqda validatorun iştirakından imtina edin.
2. **M1 Faza — Qeydiyyatlar və Parametrlər**
   - Torpaq `ZkVerifierEntry`, `PedersenParams` və `PoseidonParams` reyestrləri idarəetmə əməliyyatları, genezis ankrajı və keşin idarə edilməsi ilə.
   - Reyestr axtarışlarını, qaz cədvəlinin identifikatorlarını, sxemlərin hashingini və ölçü yoxlamalarını tələb etmək üçün simli sistem zəngi.
   - Şifrələnmiş faydalı yük formatı v1, pul kisəsi açarı çıxarma vektorlarını və məxfi açar idarəçiliyi üçün CLI dəstəyini göndərin.
3. **M2 Faza — Qaz və Performans**
   - Deterministik qaz cədvəlini, blok başına sayğacları və telemetriya ilə etalon qoşquları həyata keçirin (gecikmə müddətini, sübut ölçülərini, mempool rəddlərini yoxlayın).
   - Çox aktiv iş yükləri üçün CommitmentTree yoxlama nöqtələrini, LRU yükləməsini və ləğvedici indeksləri sərtləşdirin.
4. **M3 Fazası — Rotasiya və Pulqabı Alətləri**
   - Çox parametrli və çox versiyalı sübut qəbulunu aktivləşdirin; keçid runbooks ilə idarəetməyə əsaslanan aktivləşdirmə/depresiyanı dəstəkləyin.
   - Pul kisəsinin SDK/CLI miqrasiya axınlarını, auditorun skan edilməsi iş axınlarını və xərcləmələrin uyğunlaşdırılması alətlərini təqdim edin.
5. **M4 Faza — Audit və Əməliyyatlar**
   - Auditor əsas iş axınlarını, seçmə açıqlama API-lərini və əməliyyat kitabçalarını təmin edin.
   - Xarici kriptoqrafiya/təhlükəsizlik baxışını planlaşdırın və nəticələri `status.md`-də dərc edin.

Hər bir mərhələ blokçeyn şəbəkəsi üçün deterministik icra zəmanətlərini qorumaq üçün yol xəritəsi mərhələlərini və əlaqəli testləri yeniləyir.

### SDK və Quraşdırma Əhatəsi (M1 Faza)

Şifrələnmiş faydalı yük v1 indi kanonik qurğularla göndərilir, beləliklə hər SDK istehsal edir
eyni Norito zərfləri və əməliyyat hashləri. Qızıl əşyalar yaşayır
`fixtures/confidential/wallet_flows_v1.json` və birbaşa tərəfindən həyata keçirilir
Rust və Swift paketləri (`crates/iroha_data_model/tests/confidential_wallet_fixtures.rs`,
`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialWalletFixturesTests.swift`):

```bash
# Rust parity (verifies the signed hex + hash for every case)
cargo test -p iroha_data_model confidential_wallet_fixtures

# Swift parity (builds the same envelopes via TxBuilder/NativeBridge)
cd IrohaSwift && swift test --filter ConfidentialWalletFixturesTests
```Hər bir qurğu iş identifikatorunu, imzalanmış əməliyyat hexini və gözlənilən məlumatları qeyd edir
hash. Swift enkoderi hələ işi yarada bilmədikdə—`zk-transfer-basic`
hələ də `ZkTransfer` qurucusu tərəfindən qorunur - sınaq dəsti `XCTSkip` yayır, beləliklə
yol xəritəsi hələ də bağlama tələb edən axınları aydın şəkildə izləyir. Aparatın yenilənməsi
format versiyasını vurmadan fayl SDK-ları saxlayaraq hər iki paketi uğursuz edəcək
və kilidləmə addımında Rust istinad tətbiqi.

#### Sürətli inşaatçılar
`TxBuilder` hər biri üçün asinxron və geri çağırışa əsaslanan köməkçiləri ifşa edir.
məxfi sorğu (`IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1183`).
İnşaatçılar `connect_norito_bridge` ixracına güvənirlər
(`crates/connect_norito_bridge/src/lib.rs:3337`,
`IrohaSwift/Sources/IrohaSwift/NativeBridge.swift:1014`) belə yaradıldı
faydalı yüklər Rust host kodlayıcıları ilə bayt-bayt uyğun gəlir. Misal:

```swift
let account = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
let request = RegisterZkAssetRequest(
    chainId: chainId,
    authority: account,
    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    zkParameters: myZkParams,
    ttlMs: 60_000
)
let envelope = try TxBuilder(client: client)
    .buildRegisterZkAsset(request: request, keypair: keypair)
try await TxBuilder(client: client)
    .submit(registerZkAsset: request, keypair: keypair)
```

Ekranlama/açma eyni nümunəyə əməl edir (`submit(shield:)`,
`submit(unshield:)`) və Swift qurğusu inşaatçıları yenidən sınaqdan keçirir.
yaradılan əməliyyat hashlərinin qalmasına zəmanət vermək üçün deterministik əsas material
`wallet_flows_v1.json`-də saxlanılanlara bərabərdir.

#### JavaScript qurucuları
JavaScript SDK ixrac edilən əməliyyat köməkçiləri vasitəsilə eyni axınları əks etdirir
`javascript/iroha_js/src/transaction.js`-dən. kimi inşaatçılar
`buildRegisterZkAssetTransaction` və `buildRegisterZkAssetInstruction`
(`javascript/iroha_js/src/instructionBuilders.js:1832`) doğrulama açarını normallaşdırır
identifikatorlar və Rust hostunun heç bir ehtiyac olmadan qəbul edə biləcəyi Norito faydalı yükləri buraxın
adapterlər. Misal:

```js
import {
  buildRegisterZkAssetTransaction,
  signTransaction,
  ToriiClient,
} from "@hyperledger/iroha";

const unsigned = buildRegisterZkAssetTransaction({
  registration: {
    authority: "i105...",
    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    zkParameters: {
      commit_params: "vk_shield",
      reveal_params: "vk_unshield",
    },
    metadata: { displayName: "Rose (Shielded)" },
  },
  chainId: "00000000-0000-0000-0000-000000000000",
});
const signed = signTransaction(unsigned, myKeypair);
await new ToriiClient({ baseUrl: "https://torii" }).submitTransaction(signed);
```

Qalxan, köçürmə və ekrandan çıxaran inşaatçılar eyni nümunəyə əməl edərək JS verir
zəng edənlər Swift və Rust ilə eyni erqonomikaya malikdir. Testlər altında
`javascript/iroha_js/test/transactionBuilder.test.js` normallaşmanı əhatə edir
yuxarıdakı qurğular imzalanmış əməliyyat baytlarını ardıcıl saxlayarkən məntiq.

### Telemetriya və Monitorinq (M2 Faza)

Faza M2 indi birbaşa Prometheus və Grafana vasitəsilə CommitmentTree sağlamlığını ixrac edir:

- `iroha_confidential_tree_commitments`, `iroha_confidential_tree_depth`, `iroha_confidential_root_history_entries` və `iroha_confidential_frontier_checkpoints` hər aktiv üçün canlı Merkle sərhədini ifşa edir, `iroha_confidential_root_evictions_total` / Grafana Lütfən `zk.root_history_cap` və yoxlama nöqtəsi dərinliyi pəncərəsi.
- `iroha_confidential_frontier_last_checkpoint_height` və `iroha_confidential_frontier_last_checkpoint_commitments` ən son sərhəd keçid məntəqəsinin hündürlüyü + öhdəliklərin sayını dərc edir, beləliklə, təlimlər və geri çəkilmələr yoxlama məntəqələrinin irəlilədiyini və gözlənilən yük həcmini saxladığını sübut edə bilər.
- Grafana lövhəsi (`dashboards/grafana/confidential_assets.json`) dərinlik seriyası, evakuasiya dərəcəsi panelləri və mövcud yoxlayıcı keş vidcetlərini ehtiva edir ki, operatorlar CommitmentTree dərinliyinin hətta yoxlama məntəqələri boşaldıqda belə heç vaxt dağılmadığını sübut edə bilsinlər.
- Xəbərdarlıq `ConfidentialTreeDepthZero` (`dashboards/alerts/confidential_assets_rules.yml`-də) öhdəliklərə əməl edildikdən sonra səfərlər, lakin bildirilmiş dərinlik beş dəqiqə ərzində sıfıra bərabər qalır.

Grafana kabelini çəkməzdən əvvəl yerli olaraq ölçüləri yoxlaya bilərsiniz:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="4cuvDVPuLBKJyN6dPbRQhmLh68sU"}'
```

Dərinliyin yeni öhdəliklərlə artdığını təsdiq etmək üçün bunu `rg 'iroha_confidential_tree_depth'` ilə birləşdirin, evakuasiya sayğacları isə yalnız tarixçə girişləri kəsəndə artır. Bu dəyərlər idarəetmə sübut paketlərinə əlavə etdiyiniz Grafana idarə paneli ixracına uyğun olmalıdır.

#### Qaz cədvəli telemetriya və xəbərdarlıqlarFaza M2 həmçinin konfiqurasiya edilə bilən qaz çarpanlarını telemetriya boru kəmərinə qoyur ki, operatorlar buraxılışı təsdiq etməzdən əvvəl hər validatorun eyni yoxlama xərclərini bölüşdüyünü sübut etsinlər:

- `iroha_confidential_gas_base_verify` güzgüləri `confidential.gas.proof_base` (defolt olaraq `250_000`).
- `iroha_confidential_gas_per_public_input`, `iroha_confidential_gas_per_proof_byte`, `iroha_confidential_gas_per_nullifier` və `iroha_confidential_gas_per_commitment` `ConfidentialConfig`-də müvafiq düymələri əks etdirir. Dəyərlər işə salındıqda və konfiqurasiya yenidən yükləndikdə yenilənir; `irohad` (`crates/irohad/src/main.rs:1591,1642`) aktiv cədvəli `Telemetry::set_confidential_gas_schedule` vasitəsilə itələyir.

Düymələrin həmyaşıdları arasında eyni olduğunu təsdiqləmək üçün CommitmentTree göstəriciləri ilə yanaşı ölçüləri qırın:

```bash
# compare active multipliers across validators
for host in validator-a validator-b validator-c; do
  curl -s "http://$host:8180/metrics" \
    | rg 'iroha_confidential_gas_(base_verify|per_public_input|per_proof_byte|per_nullifier|per_commitment)'
done
```

Grafana tablosuna `confidential_assets.json` indi beş ölçünü göstərən və fərqliliyi vurğulayan “Qaz cədvəli” panelini ehtiva edir. `dashboards/alerts/confidential_assets_rules.yml`-də xəbərdarlıq qaydaları:
- `ConfidentialGasMismatch`: 3 dəqiqədən çox hər hansı bir sapma zamanı bütün sıyrılma hədəfləri və səhifələr üzrə hər çarpanın maksimum/dəqiqini yoxlayır, operatorları isti yenidən yükləmə və ya yenidən yerləşdirmə vasitəsilə `confidential.gas`-i uyğunlaşdırmağa çağırır.
- `ConfidentialGasTelemetryMissing`: Prometheus beş çarpandan hər hansı birini 5 dəqiqə ərzində qıra bilmədiyi zaman xəbərdar edir, bu, itkin sıyrılma hədəfini və ya əlil telemetriyanı göstərir.

Zəng üzrə araşdırmalar üçün aşağıdakı PromQL-i əlinizdə saxlayın:

```promql
# ensure every multiplier matches across validators (uses the same projection as the alert)
(max without(instance, job) (iroha_confidential_gas_per_public_input)
  - min without(instance, job) (iroha_confidential_gas_per_public_input)) == 0
```

Nəzarət edilən konfiqurasiya yayımlarından kənarda sapma sıfır olaraq qalmalıdır. Qaz cədvəlini dəyişdirərkən, qırıntılardan əvvəl/sonra çəkin, onları dəyişiklik sorğusuna əlavə edin və `docs/source/confidential_assets_calibration.md`-i yeni çarpanlarla yeniləyin ki, idarəetməni nəzərdən keçirənlər telemetriya sübutlarını kalibrləmə hesabatı ilə əlaqələndirə bilsinlər.