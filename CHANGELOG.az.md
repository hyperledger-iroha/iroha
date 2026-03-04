---
lang: az
direction: ltr
source: CHANGELOG.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26f5115a14476de15fbc8f26c5a9807954df6884763a818b2bc98ec6cfe1a4cc
source_last_modified: "2026-01-05T09:28:11.640562+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Dəyişikliklər qeydi

[Unreleased]: https://github.com/hyperledger-iroha/iroha/compare/v2.0.0-rc.2.0...HEAD
[2.0.0-rc.2.0]: https://github.com/hyperledger-iroha/iroha/releases/tag/v2.0.0-rc.2.0

Bu layihəyə edilən bütün nəzərəçarpacaq dəyişikliklər bu faylda sənədləşdiriləcək.

## [Yayımlanmamış]- SCALE şimini buraxın; `norito::codec` indi yerli Norito serializasiyası ilə həyata keçirilir.
- `parity_scale_codec` istifadələrini qutular arasında `norito::codec` ilə əvəz edin.
- Alətləri yerli Norito serializasiyasına köçürməyə başlayın.
- Yerli Norito serializasiyasının xeyrinə iş sahəsindən qalan `parity-scale-codec` asılılığını silin.
- Qalıq SCALE əlamət törəmələrini yerli Norito tətbiqləri ilə əvəz edin və versiyalı kodek modulunun adını dəyişin.
- Xüsusiyyət qapalı makrolarla `iroha_config_base_derive` və `iroha_futures_derive`-i `iroha_derive`-ə birləşdirin.
- *(multisig)* Sabit xəta kodu/səbəbi ilə multisig orqanlarından birbaşa imzaları rədd edin, iç içə daxil edilmiş relayerlər arasında multisig TTL qapaqlarını tətbiq edin və təqdim etməzdən əvvəl CLI-də TTL qapaqlarını tətbiq edin (SDK pariteti gözlənilir).
- FFI prosedur makrolarını `iroha_ffi`-ə köçürün və `iroha_ffi_derive` qutusunu çıxarın.
- *(schema_gen)* Lazımsız `transparent_api` xüsusiyyətini `iroha_data_model` asılılığından çıxarın.
- *(data_model)* Təkrar başlatma yükünü azaltmaq üçün `Name` təhlili üçün ICU NFC normalizatorunu keş edin.
- 📚 Torii müştərisi üçün sənəd JS sürətli başlanğıcı, konfiqurasiya həlledicisi, nəşr iş prosesi və konfiqurasiyadan xəbərdar resept.
- *(IrohaSwift)* Minimum yerləşdirmə hədəflərini iOS 15 / macOS 12-yə yüksəldin, Torii müştəri API-lərində Swift paralelliyini qəbul edin və ictimai modelləri `Sendable` kimi qeyd edin.
- *(IrohaSwift)* Əlavə edilmiş `ToriiDaProofSummaryArtifact` və `DaProofSummaryArtifactEmitter.emit` beləliklə, Swift proqramları həm yaddaşda, həm də diskdə olan sənədləri və reqressiya testlərini əhatə edən sənədlər və reqressiya testləri ilə tam CLI-yə daxil olmadan CLI-a uyğun DA sübut paketləri yarada/yaya bilsin. iş axınları.【F:IrohaSwift/Mənbələr/IrohaSwift/ToriiDaProofSummaryArtifact.swift:1】【F:IrohaSwift/Test s/IrohaSwiftTests/ToriiDaProofSummaryArtifactTests.swift:1】【F:docs/source/sdk/swift/index.md:260】
- *(data_model/js_host)* Arxivləşdirilmiş təkrar istifadə bayrağını `KaigiParticipantCommitment`-dən silməklə Kaigi Seçiminin serializasiyasını düzəldin, yerli gediş-gəliş testləri əlavə edin və JS deşifrəsini buraxın ki, Kaigi təlimatları indi Norito gediş-gəlişdən əvvəl olsun. təqdim.【F:crates/iroha_data_model/src/kaigi.rs:128】【F:crates/iroha_js_host/src/lib.rs:1379】【F:javascript/iroha_js/test/instructionBuilders.test.js:30【
- *(javascript)* `ToriiClient` zəng edənlərə defolt başlıqları silməyə icazə verin (`null`-dən keçməklə) beləliklə, `getMetrics` JSON və Prometheus mətni arasında təmiz keçsin Qəbul edin başlıqlar.【F:javascript/iroha_js/src/toriiClient.js:488】【F:javascript/iroha_js/src/toriiClient.js:761】
- *(javascript)* NFT-lər, hesab üzrə aktiv qalıqları və aktiv tərif sahibləri (TypeScript defs, sənədlər və testlər ilə) üçün təkrarlana bilən köməkçilər əlavə edildi, beləliklə Torii səhifələməsi indi qalan tətbiqi əhatə edir son nöqtələr.【F:javascript/iroha_js/src/toriiClient.js:105】【F:javascript/iroha_js/index.d.ts:8 0】【F:javascript/iroha_js/test/toriiClient.test.js:365】【F:javascript/iroha_js/README.md:470】
- *(javascript)* İdarəetmə təlimatı/əməliyyat qurucuları və idarəetmə resepti əlavə edildi ki, JS müştəriləri təkliflər, səsvermə bülletenləri, qanun qəbulu və şuranın əzmkarlığı ilə sona çata bilsinlər. son.【F:javascript/iroha_js/src/instructionBuilders.js:1012】【F:javascript/iroha_js/src/transaction.js:1082】【F:javascript/iroha_js/recipes/governance.mjs:】
- *(javascript)* Əlavə edilmiş ISO 20022 paket.008 göndərmə/status köməkçiləri və uyğun resept, JS zəng edənlərə xüsusi HTTP olmadan Torii ISO körpüsünü istifadə etməyə imkan verir santexnika.【F:javascript/iroha_js/src/toriiClient.js:888】【F:javascript/iroha_js/index.d.ts:706】【F:javascript/iroha_js/recipes/iso_bridge.mjs:】- *(javascript)* Əlavə edilmiş pacs.008/pacs.009 qurucu köməkçiləri və konfiqurasiyaya əsaslanan resept, beləliklə JS zəng edənlər təsdiqlənmiş BIC/IBAN metadatası ilə ISO 20022 faydalı yüklərini sintez edə bilsinlər. körpü.【F:javascript/iroha_js/src/isoBridge.js:1】【F:javascript/iroha_js/test/isoBridge.test.js :1】【F:javascript/iroha_js/recipes/iso_bridge_builder.mjs:1】【F:javascript/iroha_js/index.d.ts:1】
- *(javascript)* DA qəbul etmə/gətirmə/sübut etmə döngəsini tamamladı: `ToriiClient.fetchDaPayloadViaGateway` indi chunker tutacaqlarını avtomatik əldə edir (yeni `deriveDaChunkerHandle` bağlama vasitəsilə), isteğe bağlı sübut xülasələri yerli `generateDaProofSummary`-dən təkrar istifadə edir, zəng edənlər sifarişsiz `iroha da get-blob/prove-availability`-i əks etdirə bilərlər santexnika.【F:javascript/iroha_js/src/toriiClient.js:1123】【F:javascript/iroha_js/src/dataAvailability.js:1】【F:javascrip t/iroha_js/test/toriiClient.test.js:1454】【F:javascript/iroha_js/index.d.ts:3275】【F:javascript/iroha_js/README.md:760】
- *(javascript/js_host)* `sorafsGatewayFetch` skorbord metadatası indi şlüz provayderlərindən istifadə edildikdə şlüz manifest id/CID-ni qeyd edir, beləliklə qəbul artefaktları CLI ilə uyğunlaşdırılır tutur.【F:crates/iroha_js_host/src/lib.rs:3017】【F:docs/source/sorafs_orchestrator_rollout.md:23】
- *(torii/cli)* ISO piyada keçidlərini tətbiq edin: Torii indi naməlum agent BIC-ləri ilə `pacs.008` təqdimatlarını rədd edir və DvP CLI önizləməsi `--delivery-instrument-id` vasitəsilə təsdiqləyir `--iso-reference-crosswalk`.【F:crates/iroha_torii/src/iso20022_bridge.rs:704】【F:crates/iroha_cli/src/main.rs:3892】
- *(torii)* Tikintidən əvvəl `Purp=SECU` və BIC arayış-data yoxlamalarını həyata keçirərək, `POST /v1/iso20022/pacs009` vasitəsilə PvP nağd pul qəbulunu əlavə edin köçürmələr.【F:crates/iroha_torii/src/iso20022_bridge.rs:1070】【F:crates/iroha_torii/src/lib.rs:4759】
- *(alət)* Repozitoriya ilə yanaşı ISIN/CUSIP, BIC↔LEI və MIC snapşotlarını doğrulamaq üçün `cargo xtask iso-bridge-lint` (üstəgəl `ci/check_iso_reference_data.sh`) əlavə edildi qurğular.【F:xtask/src/main.rs:146】【F:ci/check_iso_reference_data.sh:1】
- *(javascript)* Repository metadata, açıq faylların icazə siyahısı, mənşəyi aktivləşdirən `publishConfig`, `prepublishOnly` dəyişiklik jurnalı/test qoruyucusu və Node20-də istifadə edən GitHub Actions iş axını elan etməklə sərtləşdirilmiş npm nəşri CI【F:javascript/iroha_js/package.json:1】【F:javascript/iroha_js/scripts/check-changelog .mjs:1】【F:docs/source/sdk/js/publishing.md:1【F:.github/workflows/javascript-sdk.yml:1】
- *(ivm/cuda)* BN254 sahəsi add/sub/mul indi `bn254_launch_kernel` vasitəsilə host-side batching ilə yeni CUDA ləpələrində yerinə yetirilir, deterministik parametrləri qoruyaraq Poseidon və ZK qadcetləri üçün aparat sürətləndirilməsinə imkan verir. ehtiyatlar.【F:crates/ivm/cuda/bn254.cu:1】【F:crates/ivm/src/cuda.rs:66】【F:crates/ivm/src/cuda.rs:1244】

## [2.0.0-rc.2.0] - 05-08-2025

### 🚀 Xüsusiyyətlər

- *(cli)* `iroha transaction get` və digər vacib əmrləri əlavə edin (#5289)
- [**breaking**] Ayrı-ayrı fungiable və qeyri-işlənə aktivlər (#5308)
- [**qırma**] Boş olmayan blokları, onlardan sonra boş bloklara icazə verməklə yekunlaşdırın (#5320)
- Sxem və müştəridə telemetriya növlərini ifşa edin (#5387)
- *(iroha_torii)* Xüsusiyyət qapalı son nöqtələr üçün stublar (#5385)
- Öhdəlik müddəti ölçülərini əlavə edin (#5380)

### 🐛 Baqların düzəldilməsi

- Sıfır olmayanları yenidən nəzərdən keçirin (#5278)
- Sənəd fayllarında hərf səhvləri (#5309)
- *(kripto)* `Signature::payload` alıcısını ifşa edin (#5302) (#5310)
- *(əsas)* Təqdim etməzdən əvvəl rolun olub-olmaması üçün yoxlamalar əlavə edin (#5300)
- *(əsas)* Bağlantı kəsilmiş həmyaşıdını yenidən birləşdirin (#5325)
- Mağaza aktivləri və NFT ilə əlaqəli pytestləri düzəldin (#5341)
- *(CI)* Şeir v2 üçün python statik analiz iş axınını düzəldin (#5374)
- Müddəti bitmiş tranzaksiya hadisəsi icra edildikdən sonra görünür (#5396)

### 💼 Digər- `rust-toolchain.toml` (#5376) daxil edin
- `deny` deyil, `unused`-də xəbərdarlıq edin (#5377)

### 🚜 Refaktor

- Umbrella Iroha CLI (#5282)
- *(iroha_test_network)* Qeydlər üçün gözəl formatdan istifadə edin (#5331)
- [**breaking**] `genesis.json`-də `NumericSpec` seriyasını sadələşdirin (#5340)
- Uğursuz p2p bağlantısı üçün girişi təkmilləşdirin (#5379)
- `logger.level`-i geri qaytarın, `logger.filter` əlavə edin, konfiqurasiya marşrutlarını genişləndirin (#5384)

### 📚 Sənədləşmə

- `network.public_address`-i `peer.template.toml`-ə əlavə edin (#5321)

### ⚡ Performans

- *(kura)* Diskə lazımsız blokun yazılmasının qarşısını alın (#5373)
- Tranzaksiyaların hashləri üçün xüsusi yaddaş tətbiq edildi (#5405)

### ⚙️ Müxtəlif Tapşırıqlar

- Şeir istifadəsini düzəldin (#5285)
- `iroha_torii_const` (#5322)-dən lazımsız sabitləri çıxarın
- İstifadə edilməmiş `AssetEvent::Metadata*` (#5339) silin
- Bump Sonarqube Action versiyası (#5337)
- İstifadə edilməmiş icazələri silin (#5346)
- Ci-image-ə zip paketini əlavə edin (#5347)
- Bəzi şərhləri düzəldin (#5397)
- İnteqrasiya testlərini `iroha` qutusundan çıxarın (#5393)
- Defectdojo işini söndürün (#5406)
- Çatışmayan öhdəliklər üçün DCO imzasını əlavə edin
- İş axınını yenidən təşkil edin (ikinci cəhd) (#5399)
- Əsasa təkanla Pull Request CI-ni işə salmayın (#5415)

<!-- generated by git-cliff -->

## [2.0.0-rc.1.3] - 03-07-2025

### Əlavə edilib

- boş olmayan blokları onlardan sonra boş bloklara icazə verməklə yekunlaşdırın (#5320)

## [2.0.0-rc.1.2] - 25-02-2025

### Sabit

- yenidən qeydiyyatdan keçmiş həmyaşıdlar indi həmyaşıdlar siyahısında düzgün şəkildə əks olunur (#5327)

## [2.0.0-rc.1.1] - 2025-02-12

### Əlavə edilib

- `iroha transaction get` və digər vacib əmrləri əlavə edin (#5289)

## [2.0.0-rc.1.0] - 12-06-2024

### Əlavə edilib

- sorğu proqnozlarını həyata keçirin (#5242)
- davamlı icraçıdan istifadə edin (#5082)
- iroha cli-yə dinləmə müddətləri əlavə edin (#5241)
- torii-yə /peers API son nöqtəsini əlavə edin (#5235)
- aqnostik p2p ünvanı (#5176)
- multisig yardım proqramını və istifadəni yaxşılaşdırın (#5027)
- `BasicAuth::password` çap olunmaqdan qoruyun (#5195)
- `FindTransactions` sorğusunda azalan üzrə çeşidləyin (#5190)
- blok başlığını hər ağıllı müqavilənin icrası kontekstinə daxil edin (#5151)
- görünüş dəyişikliyi indeksinə əsaslanan dinamik icra müddəti (#4957)
- standart icazə dəstini təyin edin (#5075)
- `Option<Box<R>>` (#5094) üçün Niş tətbiqini əlavə edin
- əməliyyat və blok predikatları (#5025)
- sorğuda qalan maddələrin məbləğini bildirin (#5016)
- məhdud diskret vaxt (#4928)
- çatışmayan riyazi əməliyyatları `Numeric`-ə əlavə edin (#4976)
- blok sinxronizasiya mesajlarını doğrulayın (#4965)
- sorğu filtrləri (#4833)

### Dəyişdirilib

- peer id təhlilini sadələşdirin (#5228)
- əməliyyat xətasını blok yükündən kənara köçürün (#5118)
- JsonString adını Json olaraq dəyişdirin (#5154)
- ağıllı müqavilələrə müştəri qurumu əlavə edin (#5073)
- əməliyyat sifarişi xidməti kimi lider (#4967)
- yaddaşdan köhnə blokları silmək (#5103)
- `Executable`-dəki təlimatlar üçün `ConstVec` istifadə edin (#5096)
- ən çox bir dəfə qeybət edin (#5079)
- `CommittedTransaction` yaddaş istifadəsini azaldır (#5089)
- sorğu kursoru xətalarını daha konkret etmək (#5086)
- qutuları yenidən təşkil edin (#4970)
- `FindTriggers` sorğusunu təqdim edin, `FindTriggerById` (#5040) silin
- yeniləmə üçün imzalardan asılı olmayın (#5039)
- genesis.json-da parametrlərin formatını dəyişdirin (#5020)
- yalnız cari və əvvəlki görünüş dəyişikliyinin sübutunu göndərin (#4929)
- məşğul döngənin qarşısını almaq üçün hazır olmadıqda mesaj göndərilməsini söndürün (#5032)
- ümumi aktiv miqdarını aktivin tərifinə köçürün (#5029)
- bütün faydalı yükü deyil, yalnız blokun başlığını imzalayın (#5000)
- blok hash növü kimi `HashOf<BlockHeader>` istifadə edin (#4998)
- `/health` və `/api_version` (#4960) sadələşdirin
- `configs` adını `defaults` olaraq dəyişdirin, `swarm` (#4862) silin

### Sabit- json-da daxili rolu düzəldin (#5198)
- `cargo audit` xəbərdarlıqlarını düzəldin (#5183)
- imza indeksinə diapazon yoxlanışı əlavə edin (#5157)
- sənədlərdə model makro nümunəsini düzəldin (#5149)
- bloklar/hadisələr axınında ws-ləri düzgün bağlayın (#5101)
- sınmış etibarlı həmyaşıdların yoxlanışı (#5121)
- növbəti blokun hündürlüyünün +1 olduğunu yoxlayın (#5111)
- genezis blokunun vaxt damğasını düzəltmək (#5098)
- `iroha_genesis` kompilyasiyasını `transparent_api` xüsusiyyəti olmadan düzəldin (#5056)
- düzgün idarə edin `replace_top_block` (#4870)
- icraçının klonlaşdırılmasını düzəldin (#4955)
- daha çox səhv təfərrüatını göstərin (#4973)
- blok axını üçün `GET` istifadə edin (#4990)
- növbə əməliyyatlarının idarə edilməsini təkmilləşdirin (#4947)
- lazımsız blok sinxronizasiya blok mesajlarının qarşısını almaq (#4909)
- böyük mesajın eyni vaxtda göndərilməsində blokadanın qarşısını almaq (#4948)
- vaxtı keçmiş əməliyyatı keşdən silin (#4922)
- torii url-ni yol ilə düzəldin (#4903)

### Silindi

- modul əsaslı api-ni müştəridən çıxarın (#5184)
- `riffle_iter` (#5181) silin
- istifadə olunmamış asılılıqları silin (#5173)
- `max` prefiksini `blocks_in_memory`-dən çıxarın (#5145)
- konsensus qiymətləndirməsini çıxarın (#5116)
- blokdan `event_recommendations` çıxarın (#4932)

### Təhlükəsizlik

## [2.0.0-pre-rc.22.1] - 30-07-2024

### Sabit

- docker şəklinə `jq` əlavə etdi

## [2.0.0-pre-rc.22.0] - 25-07-2024

### Əlavə edilib

- zəncirdə olan parametrləri açıq şəkildə genezisdə göstərin (#4812)
- çoxsaylı `Instruction` (#4805) ilə turbobalığa icazə verin
- çox imzalı əməliyyatları yenidən həyata keçirin (#4788)
- daxili və xüsusi zəncir parametrlərini həyata keçirin (#4731)
- fərdi təlimat istifadəsini təkmilləşdirmək (#4778)
- JsonString (#4732) tətbiq etməklə metaməlumatları dinamik etmək
- çoxsaylı həmyaşıdların genezis blokunu təqdim etməsinə icazə verin (#4775)
- həmyaşıdlara `SignedTransaction` əvəzinə `SignedBlock` təqdim edin (#4739)
- icraçıda fərdi təlimatlar (#4645)
- json sorğularını tələb etmək üçün müştəri cli-ni genişləndirin (#4684)
 - `norito_decoder` (#4680) üçün aşkarlama dəstəyi əlavə edin
- icraçı məlumat modelinə icazə sxeminin ümumiləşdirilməsi (#4658)
- standart icraçıda əlavə registr tetikleme icazələri (#4616)
 - `norito_cli`-də JSON-u dəstəkləyin
- p2p boş vaxt aşımını təqdim edin

### Dəyişdirilib

- `lol_alloc`-i `dlmalloc` (#4857) ilə əvəz edin
- sxemdə `type_` adını `type` olaraq dəyişdirin (#4855)
- sxemdə `Duration`-i `u64` ilə əvəz edin (#4841)
- giriş üçün `RUST_LOG` kimi EnvFilter istifadə edin (#4837)
- mümkün olduqda səsvermə blokunu saxlayın (#4828)
- warpdan axuma miqrasiya (#4718)
- split icraçı məlumat modeli (#4791)
- dayaz məlumat modeli (#4734) (#4792)
- açıq açarı imza ilə göndərməyin (#4518)
- `--outfile` adını `--out-file` (#4679) olaraq dəyişdirin
- iroha server və müştərinin adını dəyişdirin (#4662)
- `PermissionToken` adını `Permission` (#4635) olaraq dəyişdirin
- `BlockMessages`-i həvəslə rədd edin (#4606)
- `SignedBlock` dəyişməz olun (#4620)
- TransactionValue adını CommittedTransaction olaraq dəyişdirin (#4610)
- Şəxsi hesabları şəxsiyyət vəsiqəsi ilə təsdiqləyin (#4411)
- şəxsi açarlar üçün multihash formatından istifadə edin (#4541)
 - `parity_scale_decoder` adını `norito_cli` olaraq dəyişdirin
- Set B təsdiqləyicilərinə bloklar göndərin
- `Role` şəffaf olun (#4886)
- başlıqdan blok hash əldə edin (#4890)

### Sabit- ötürmək üçün domenin sahibi olduğunu yoxlayın (#4807)
- logger cüt başlanğıcını çıxarın (#4800)
- aktivlər və icazələr üçün adlandırma konvensiyasını düzəldin (#4741)
- genezis blokunda (#4757) ayrıca əməliyyatda icraçının təkmilləşdirilməsi
- `JsonString` üçün düzgün standart dəyər (#4692)
- serializasiya xətası mesajını təkmilləşdirin (#4659)
- əgər ötürülən Ed25519Sha512 açıq açarı etibarsız uzunluqdadırsa, panik etməyin (#4650)
- başlanğıc blok yükləməsində düzgün görünüş dəyişmə indeksindən istifadə edin (#4612)
- `start` vaxt möhüründən (#4333) əvvəl vaxt tetikleyicilerini vaxtından əvvəl icra etməyin
- `torii_url` (#4601) (#4617) üçün `https` dəstəyi
- SetKeyValue/RemoveKeyValue-dən (#4547) serdeni çıxarın (düzləşdirin)
- trigger dəsti düzgün seriallaşdırılıb
- `Upgrade<Executor>`-də silinmiş `PermissionToken`-ləri ləğv edin (#4503)
- Cari tur üçün düzgün görünüş dəyişikliyi indeksini bildirin
- `Unregister<Domain>` (#4461)-də müvafiq tetikleyiciləri çıxarın
- genesis raundunda genesis pub açarını yoxlayın
- genesis Domain və ya Hesabın qeydiyyata alınmasının qarşısını almaq
- qurumun qeydiyyatdan çıxarılması ilə bağlı rollardan icazələrin silinməsi
- tetikleyici metadata ağıllı müqavilələrdə əlçatandır
- uyğunsuz vəziyyət görünüşünün qarşısını almaq üçün rw kilidindən istifadə edin (#4867)
- snapshotda yumşaq çəngəli idarə edin (#4868)
- ChaCha20Poly1305 üçün MinSize-i düzəldin
- yüksək yaddaş istifadəsinin qarşısını almaq üçün LiveQueryStore-a məhdudiyyətlər əlavə edin (#4893)

### Silindi

- açıq açarı ed25519 şəxsi açarından çıxarın (#4856)
- kura.lock silin (#4849)
- konfiqurasiyada `_ms` və `_bytes` şəkilçilərini geri qaytarın (#4667)
- genezis sahələrindən `_id` və `_file` şəkilçisini çıxarın (#4724)
- AssetDefinitionId (#4701) ilə AssetsMap-da indeks aktivlərini silin
- tetik identifikasiyasından domeni silin (#4640)
- Iroha (#4673)-dən genezis imzalanmasını silin
- `Visit`-ni `Validate`-dən çıxarın (#4642)
- `TriggeringEventFilterBox` (#4866) silin
- p2p əl sıxmasında `garbage`-i silin (#4889)
- `committed_topology`-i blokdan çıxarın (#4880)

### Təhlükəsizlik

- sirrin sızmasına qarşı qoruyun

## [2.0.0-pre-rc.21] - 19-04-2024

### Əlavə edilib

- trigger id-ni trigger giriş nöqtəsinə daxil edin (#4391)
- hadisə dəstini sxemdə bit sahələri kimi ifşa edin (#4381)
- dənəvər girişi olan yeni `wsv` təqdim edin (#2664)
- `PermissionTokenSchemaUpdate`, `Configuration` və `Executor` hadisələri üçün hadisə filtrləri əlavə edin
- snapshot "rejimini" təqdim edin (#4365)
- rol icazələrinin verilməsinə/ləğv edilməsinə icazə verin (#4244)
- aktivlər üçün ixtiyari dəqiqlikli ədədi növü təqdim edin (bütün digər rəqəm növlərini silin) (#3660)
- İcraçı üçün fərqli yanacaq limiti (#3354)
- pprof profilini birləşdirin (#4250)
- müştəri CLI-də aktiv alt əmri əlavə edin (#4200)
- `Register<AssetDefinition>` icazələri (#4049)
- təkrar hücumların qarşısını almaq üçün `chain_id` əlavə edin (#4185)
- müştəri CLI-də domen metadatasını redaktə etmək üçün alt əmrlər əlavə edin (#4175)
- Client CLI-də mağaza dəstini həyata keçirin, silin, əməliyyatlar alın (#4163)
- tetikleyiciler üçün eyni ağıllı müqavilələri sayın (#4133)
- domenləri köçürmək üçün müştəri CLI-yə alt əmr əlavə edin (#3974)
- FFI-də qutulu dilimləri dəstəkləyin (#4062)
- git müştəri CLI-yə SHA-nı yerinə yetirir (#4042)
- defolt təsdiqləyici qazan lövhəsi üçün proc makrosu (#3856)
- Client API-də sorğu sorğusu qurucusunu təqdim etdi (#3124)
- ağıllı müqavilələr daxilində tənbəl sorğular (#3929)
- `fetch_size` sorğu parametri (#3900)
- aktiv mağazasının köçürülməsi təlimatı (#4258)
- sirrin sızmasına qarşı qoruyun (#3240)
- eyni mənbə kodu ilə təkmilləşdirici tetikler (#4419)

### Dəyişdirilib- 18-04-2024-cü il gecəsi paslı alətlər silsiləsi
- blokları B təsdiqləyicilərinə göndərin (#4387)
- boru kəməri hadisələrini blok və əməliyyat hadisələrinə ayırın (#4366)
- `[telemetry.dev]` konfiqurasiya hissəsinin adını `[dev_telemetry]` (#4377) olaraq dəyişdirin
- `Action` və `Filter` qeyri-generik növləri hazırlayın (#4375)
- qurucu nümunəsi ilə hadisə filtrləmə API-ni təkmilləşdirin (#3068)
- müxtəlif hadisə filtri API-lərini birləşdirin, səlis qurucu API təqdim edin
- `FilterBox` adını `EventFilterBox` olaraq dəyişdirin
- `TriggeringFilterBox` adını `TriggeringEventFilterBox` olaraq dəyişdirin
- filtrin adının yaxşılaşdırılması, məs. `AccountFilter` -> `AccountEventFilter`
- konfiqurasiyanı RFC konfiqurasiyasına uyğun olaraq yenidən yazın (#4239)
- versiyalı strukturların daxili strukturunu ictimai API-dən gizlədin (#3887)
- çox uğursuz görünüş dəyişikliklərindən sonra müvəqqəti olaraq proqnozlaşdırıla bilən sifarişi tətbiq edin (#4263)
- `iroha_crypto` (#4181)-də konkret açar növlərindən istifadə edin
- normal mesajlardan bölünmüş görünüş dəyişiklikləri (#4115)
- `SignedTransaction`-i dəyişməz etmək (#4162)
- ixrac `iroha_config` vasitəsilə `iroha_client` (#4147)
- ixrac `iroha_crypto` vasitəsilə `iroha_client` (#4149)
- `iroha_client` vasitəsilə `data_model` ixracı (#4081)
- `openssl-sys` asılılığını `iroha_crypto`-dən çıxarın və konfiqurasiya edilə bilən tls arxa uclarını `iroha_client`-ə təqdim edin (#3422)
- baxımsız EOF `hyperledger/ursa`-i daxili həll `iroha_crypto` (#3422) ilə əvəz edin
- icraçı performansını optimallaşdırın (#4013)
- topologiyanın yenilənməsi (#3995)

### Sabit

- `Unregister<Domain>` (#4461)-də müvafiq tetikleyiciləri çıxarın
- qurumun qeydiyyatdan çıxarılması ilə bağlı rollardan icazələrin silinməsi (#4242)
- genesis əməliyyatının genesis pub açarı ilə imzalandığını təsdiqləyin (#4253)
- p2p-də cavab verməyən həmyaşıdlar üçün fasilə təqdim edin (#4267)
- genesis Domain və ya Hesabın qeydiyyata alınmasının qarşısını almaq (#4226)
- `ChaCha20Poly1305` (#4395) üçün `MinSize`
- `tokio-console` aktiv olduqda konsolu işə salın (#4377)
- hər bir elementi `\n` ilə ayırın və `dev-telemetry` fayl qeydləri üçün rekursiv ana qovluqlar yaradın
- imzasız hesab qeydiyyatının qarşısını almaq (#4212)
- açar cütlərinin yaradılması indi səhvsizdir (#4283)
- `X25519` düymələrini `Ed25519` (#4174) kimi kodlamağı dayandırın
- `no_std` (#4270)-də imza təsdiqini edin
- asinxron kontekstdə bloklama metodlarının çağırılması (#4211)
- təşkilatın qeydiyyatdan çıxarılması ilə bağlı əlaqəli tokenləri ləğv edin (#3962)
- Sumeragi işə salındıqda asinxron bloklama səhvi
- sabit `(get|set)_config` 401 HTTP (#4177)
- `musl` arxivator adı Docker (#4193)
- ağıllı müqavilə debug çapı (#4178)
- yenidən başladıqda topologiya yeniləməsi (#4164)
- yeni həmyaşıdın qeydiyyatı (#4142)
- zəncirdə proqnozlaşdırıla bilən iterasiya sırası (#4130)
- re-architect logger və dinamik konfiqurasiya (#4100)
- tetikleyici atomiklik (#4106)
- sorğu mağazası mesajı sifariş problemi (#4057)
- Norito istifadə edərək cavab verən son nöqtələr üçün `Content-Type: application/x-norito` təyin edin

### Silindi

- `logger.tokio_console_address` konfiqurasiya parametri (#4377)
- `NotificationEvent` (#4377)
- `Value` nömrə (#4305)
- İrohadan MST aqreqasiyası (#4229)
- ISI üçün klonlaşdırma və ağıllı müqavilələrdə sorğuların icrası (#4182)
- `bridge` və `dex` xüsusiyyətləri (#4152)
- yastı hadisələr (#3068)
- ifadələr (#4089)
- avtomatik yaradılan konfiqurasiya arayışı
- Qeydlərdə `warp` səs-küy (#4097)

### Təhlükəsizlik

- p2p-də pub açarının saxtalaşdırılmasının qarşısını almaq (#4065)
- OpenSSL-dən çıxan `secp256k1` imzalarının normallaşdırılmasını təmin edin (#4155)

## [2.0.0-pre-rc.20] - 2023-10-17

### Əlavə edilib- `Domain` sahibliyini köçürün
- `Domain` sahib icazələri
- `owned_by` sahəsini `Domain`-ə əlavə edin
- `iroha_client_cli`-də JSON5 kimi filtri təhlil edin (#3923)
- Serde qismən etiketlənmiş enumlarda Self tipindən istifadə üçün dəstək əlavə edin
- Standartlaşdırılmış blok API (#3884)
- `Fast` kura başlanğıc rejimini həyata keçirin
- iroha_swarm imtina başlığını əlavə edin
- WSV snapshotları üçün ilkin dəstək

### Sabit

- update_configs.sh (#3990)-da icraçının endirilməsini düzəldin
- devShell-də düzgün rustc
- Yanma `Trigger` reprititionlarını düzəldin
- `AssetDefinition` transferini düzəldin
- `Domain` üçün `RemoveKeyValue`-i düzəldin
- `Span::join` istifadəsini düzəldin
- Topologiya uyğunsuzluğu səhvini düzəldin (#3903)
- `apply_blocks` və `validate_blocks` müqayisəsini düzəldin
- Mağaza yolu ilə `mkdir -r`, kilid yolu deyil (#3908)
- Test_env.py-də dir varsa, uğursuz olmayın
- Doğrulama/avtorizasiya sənədini düzəldin (#3876)
- Sorğu tapmaq xətası üçün daha yaxşı səhv mesajı
- Dev docker tərtib etmək üçün genesis hesabının ictimai açarını əlavə edin
- İcazə nişanı yükünü JSON (#3855) kimi müqayisə edin
- `irrefutable_let_patterns`-i `#[model]` makrosunda düzəldin
- Genesisin istənilən ISI-ni yerinə yetirməsinə icazə verin (#3850)
- Yaradılış doğrulamasını düzəldin (#3844)
- 3 və ya daha az həmyaşıd üçün topologiyanı düzəldin
- tx_amounts histoqramının necə hesablandığını düzəldin.
- `genesis_transactions_are_validated()` test qırıqlığı
- Defolt validator generasiyası
- İroha zərif bağlanmasını düzəldin

### Refaktor- istifadə olunmamış asılılıqları silin (#3992)
- zərbədən asılılıqlar (#3981)
- Doğrulayıcının adını icraçıya dəyişdirin (#3976)
- `IsAssetDefinitionOwner` (#3979) silin
- Ağıllı müqavilə kodunu iş sahəsinə daxil edin (#3944)
- API və Telemetriya son nöqtələrini bir serverə birləşdirin
- ifadə lenini ictimai API-dən nüvəyə köçürün (#3949)
- Rol axtarışında klonlamadan çəkinin
- Rollar üçün diapazon sorğuları
- Hesab rollarını `WSV`-ə köçürün
- ISI adını *Box-dan *Expr-ə dəyişdirin (#3930)
- Versiyalaşdırılmış konteynerlərdən "Versiyalaşdırılmış" prefiksi silin (#3913)
- `commit_topology`-i blok yükünə köçürün (#3916)
- `telemetry_future` makrosunu sinxronizasiya 2.0-a köçürün
- ISI sərhədlərində İdentifikasiya ilə qeydiyyata alınıb (#3925)
- `derive(HasOrigin)`-ə əsas generik dəstəyi əlavə edin
- Klipi xoşbəxt etmək üçün Emitent API sənədlərini təmizləyin
- Derive(HasOrigin) makrosu üçün testlər əlavə edin, derive(IdEqOrdHash)-da təkrarı azaldın, stabil olaraq səhv hesabatını düzəldin
- Adlandırmanı təkmilləşdirin, təkrarlanan .filter_xəritələrini sadələşdirin və əldə etmək (Filter) istisna olmaqla, lazımsız .
- PartallyTaggedSerialize/Deserialize istifadə edin sevgilim
- Darling (IdEqOrdHash) istifadə edin, testlər əlavə edin
- Darling (Filtr) istifadə edin
- Syn 2.0 istifadə etmək üçün iroha_data_model_derive-i yeniləyin
- İmza yoxlama vəziyyəti vahidi testlərini əlavə edin
- Yalnız sabit imza yoxlama şərtlərinə icazə verin
- ConstBytes-i istənilən const ardıcıllığını saxlayan ConstVec-ə ümumiləşdirin
- Dəyişməyən bayt dəyərləri üçün daha səmərəli təmsildən istifadə edin
- Hazırlanmış wsv-ni anlıq görüntüdə saxlayın
- `SnapshotMaker` aktyoru əlavə edin
- proc makrolarında təhlilin sənəd məhdudiyyəti
- şərhləri təmizləyin
- lib.rs-ə atributları təhlil etmək üçün ümumi test yardım proqramını çıxarın
- parse_display istifadə edin və Attr -> Attrs adlarını yeniləyin
- ffi args funksiyasında model uyğunluğundan istifadə etməyə icazə verin
- getset attrs təhlilində təkrarı azaldır
- Emitter::into_token_stream adını Emitent::finish_token_stream olaraq dəyişdirin
- Getset tokenlərini təhlil etmək üçün parse_display istifadə edin
- Yazı xətalarını düzəldin və səhv mesajlarını yaxşılaşdırın
- iroha_ffi_derive: atributları təhlil etmək üçün darling istifadə edin və syn 2.0 istifadə edin
- iroha_ffi_derive: proc-makro-xətanı manyhow ilə əvəz edin
- Kura kilidi fayl kodunu sadələşdirin
- bütün rəqəmli dəyərləri sətir hərfi kimi seriallaşdırın
- Kagami (#3841) ayırın
- `scripts/test-env.sh`-i yenidən yazın
- Ağıllı müqavilə və tetikleyici giriş nöqtələrini fərqləndirin
- `data_model/src/block.rs`-də Elide `.cloned()`
- syn 2.0 istifadə etmək üçün `iroha_schema_derive`-i yeniləyin

## [2.0.0-pre-rc.19] - 14-08-2023

### Əlavə edilib- hyperledger#3309 Təkmilləşdirmək üçün Bump IVM iş vaxtı
- hyperledger#3383 Kompilyasiya zamanı soket ünvanlarını təhlil etmək üçün makronu tətbiq edin
- hyperledger#2398 Sorğu filtrləri üçün inteqrasiya testləri əlavə edin
- `InternalError`-ə faktiki səhv mesajını daxil edin
- Standart alət zənciri kimi `nightly-2023-06-25` istifadəsi
- hyperledger#3692 Validator miqrasiyası
- [DSL internship] hyperledger # 3688: Proc makro kimi əsas arifmetikanı həyata keçirin
- hyperledger#3371 Validatorların artıq smart-müqavilə kimi baxılmamasını təmin etmək üçün `entrypoint` bölünmüş validator
- qəzadan sonra Iroha qovşağını tez bir zamanda gətirməyə imkan verən hyperledger#3651 WSV snapshots
- hyperledger#3752 `MockValidator`-i bütün əməliyyatları qəbul edən `Initial` validator ilə əvəz edin
- hyperledger#3276 Iroha nodeunun əsas jurnalına müəyyən edilmiş sətri qeyd edən `Log` adlı müvəqqəti təlimat əlavə edin
- hyperledger#3641 İcazə tokeninin faydalı yükünü insanlar tərəfindən oxuna bilən edin
- hyperledger#3324 `iroha_client_cli` ilə əlaqəli `burn` yoxlamaları və refaktorinq əlavə edin
- hyperledger#3781 Genesis əməliyyatlarını təsdiqləyin
- hyperledger#2885 Tətiklər üçün istifadə edilə bilən və edilə bilməyən hadisələri fərqləndirin
- hyperledger#2245 `Nix` əsaslı iroha node binar quruluşu `AppImage`

### Sabit

- hyperledger#3613 Səhv imzalanmış əməliyyatların qəbul edilməsinə icazə verə bilən reqressiya
- Yanlış Konfiqurasiya topologiyasını erkən rədd edin
- hyperledger#3445 Reqressiyanı düzəldin və `/configuration` son nöqtəsində `POST`-i yenidən işləyin
- hyperledger#3654 `iroha2` `glibc` əsaslı `Dockerfiles`-i yerləşdirmək üçün düzəldin
- hyperledger#3451 Apple silikon mac-larında `docker` quruluşunu düzəldin
- hyperledger#3741 `kagami validator`-də `tempfile` xətasını düzəldin
- hyperledger#3758 Fərdi qutuların tikilə bilmədiyi, lakin iş sahəsinin bir hissəsi kimi tikilə biləcəyi reqressiyanı düzəldin
- hyperledger#3777 Rol qeydiyyatında yamaq boşluğu təsdiqlənmir
- hyperledger#3805 `SIGTERM` qəbul etdikdən sonra Iroha-in bağlanmamasını düzəldin

### Digər

- hyperledger#3648 `docker-compose.*.yml` yoxlamasını CI proseslərinə daxil edin
- `len()` təlimatını `iroha_data_model`-dən `iroha_core`-ə köçürün
- hyperledger#3672 törəmə makrolarında `HashMap`-i `FxHashMap` ilə əvəz edin
- hyperledger#3374 Səhv sənəd şərhlərini və `fmt::Display` tətbiqini birləşdirin
- hyperledger#3289 Layihə boyu Rust 1.70 iş sahəsi miraslarından istifadə edin
- hyperledger#3654 `GNU libc <https://www.gnu.org/software/libc/>`_ üzərində iroha2 qurmaq üçün `Dockerfiles` əlavə edin
- Proc-makroslar üçün `syn` 2.0, `manyhow` və `darling` təqdim edin
- hyperledger#3802 Unicode `kagami crypto` toxumu

## [2.0.0-pre-rc.18]

### Əlavə edilib

- hyperledger#3468: Server tərəfi kursoru, sorğunun gecikməsi üçün əsas müsbət təsir göstərməli olan tənbəlliklə qiymətləndirilmiş təkrar səhifələşdirməyə imkan verir.
- hyperledger#3624: Ümumi təyinatlı icazə nişanları; konkret olaraq
  - İcazə tokenləri istənilən struktura malik ola bilər
  - Token strukturu `iroha_schema`-də özünü təsvir edir və JSON sətri kimi seriallaşdırılır
  - Token dəyəri `Norito` kodludur
  - bu dəyişiklik nəticəsində icazə nişanı adlandırma konvensiyası `snake_case`-dən `UpeerCamelCase`-ə köçürüldü
- hyperledger#3615 Təsdiqdən sonra wv-i qoruyun

### Sabit- hyperledger#3627 Tranzaksiya atomikliyi indi `WorlStateView`-in klonlanması ilə tətbiq edilir
- hyperledger#3195 Rədd edilmiş genezis əməliyyatını qəbul edərkən panik davranışını genişləndirin
- hyperledger#3042 Səhv sorğu mesajını düzəldin
- hyperledger#3352 Nəzarət axını və məlumat mesajını ayrı-ayrı kanallara bölün
- hyperledger#3543 Metriklərin dəqiqliyini təkmilləşdirin

## 2.0.0-pre-rc.17

### Əlavə edilib

- hyperledger#3330 `NumericValue` seriyadan çıxarılmasını genişləndirin
- hyperledger#2622 `u128`/`i128` FFI-də dəstək
- hyperledger#3088 DoS-in qarşısını almaq üçün növbənin azaldılmasını tətbiq edin
- hyperledger#2373 `kagami swarm file` və `kagami swarm dir` `docker-compose` faylları yaratmaq üçün əmr variantları
- hyperledger#3597 İcazə Token Analizi (Iroha tərəfi)
- hyperledger#3353 `eyre`-i `block.rs`-dan xəta şərtlərini sadalamaq və güclü şəkildə yazılmış xətalardan istifadə etməklə çıxarın
- hyperledger#3318 Tranzaksiyaların emal qaydasını qorumaq üçün rədd edilmiş və qəbul edilmiş əməliyyatları bloklara daxil edin

### Sabit

- hyperledger#3075 Etibarsız əməliyyatların işlənməsinin qarşısını almaq üçün `genesis.json`-də etibarsız əməliyyatda panik
- hyperledger#3461 Defolt konfiqurasiyada defolt dəyərlərin düzgün idarə edilməsi
- hyperledger#3548 `IntoSchema` şəffaf atributunu düzəldin
- hyperledger#3552 Validator yolunun sxem təqdimatını düzəldin
- hyperledger#3546 Zaman tetikleyicilerinin ilişib qalmasını aradan qaldırın
- hyperledger#3162 Blok axın sorğularında 0 hündürlüyünü qadağan edin
- Konfiqurasiya makrosunun ilkin testi
- hyperledger#3592 `release`-də yenilənən konfiqurasiya faylları üçün düzəliş
- hyperledger#3246 `Set B validators <https://github.com/hyperledger-iroha/iroha/blob/main/docs/source/iroha_2_whitepaper.md#2-system-architecture>`_ daxil etməyin `fault <https://en.wikipedia.org/wiki/Byzantine_fault>`_ olmadan
- hyperledger#3570 Müştəri tərəfi sətir sorğu xətalarını düzgün göstərin
- hyperledger#3596 `iroha_client_cli` blokları/hadisələri göstərir
- hyperledger#3473 `kagami validator`-i iroha repozitorunun kök qovluğundan kənarda işləyin

### Digər

- hyperledger#3063 `wsv`-də hündürlüyü bloklamaq üçün `hash` əməliyyatının xəritəsi
- `Value`-də güclü şəkildə yazılmış `HashOf<T>`

## [2.0.0-pre-rc.16]

### Əlavə edilib

- hyperledger#2373 `kagami swarm` `docker-compose.yml` yaratmaq üçün alt komanda
- hyperledger#3525 Əməliyyat API-sini standartlaşdırın
- hyperledger#3376 Iroha Client CLI `pytest <https://docs.pytest.org/en/7.4.x/>`_ avtomatlaşdırma çərçivəsini əlavə edin
- hyperledger#3516 `LoadedExecutable`-də orijinal blob hashını saxla

### Sabit

- hyperledger#3462 `burn` aktiv əmrini `client_cli`-ə əlavə edin
- hyperledger#3233 Refaktor xəta növləri
- hyperledger#3330 `partially-tagged <https://serde.rs/enum-representations.html>`_ `enums` üçün `serde::de::Deserialize`-i əl ilə tətbiq etməklə reqressiyanı düzəldin
- hyperledger#3487 Çatışmayan növləri sxemə qaytarın
- hyperledger#3444 Diskriminantı sxemə qaytarın
- hyperledger#3496 `SocketAddr` sahə təhlilini düzəldin
- hyperledger#3498 Yumşaq çəngəl aşkarlamasını düzəldin
- hyperledger#3396 Blok törədilmiş hadisəni yaymazdan əvvəl bloku `kura`-də saxlayın

### Digər

- hyperledger#2817 `WorldStateView`-dən daxili dəyişkənliyi aradan qaldırın
- hyperledger#3363 Genesis API refaktoru
- Mövcud refaktor və topologiya üçün yeni testlərlə əlavə edin
- Test əhatə dairəsi üçün `Codecov <https://about.codecov.io/>`_-dən `Coveralls <https://coveralls.io/>`_-ə keçin
- hyperledger#3533 Sxemdə `Bool` adını `bool` olaraq dəyişdirin

## [2.0.0-pre-rc.15]

### Əlavə edilib- hyperledger#3231 Monolit validator
- hyperledger#3015 FFI-də niş optimallaşdırılması üçün dəstək
- hyperledger#2547 `AssetDefinition`-ə loqo əlavə edin
- hyperledger#3274 `kagami`-ə misallar yaradan alt komanda əlavə edin (LTS-də əks olunur)
- hiperledger#3415 `Nix <https://nixos.wiki/wiki/Flakes>`_ lopa
- hyperledger#3412 Tranzaksiya qeybətini ayrıca aktyora köçürün
- hyperledger#3435 `Expression` ziyarətçisini təqdim edin
- hyperledger#3168 Genesis validatorunu ayrıca fayl kimi təqdim edin
- hyperledger#3454 LTS-i əksər Docker əməliyyatları və sənədləri üçün defolt edin
- hyperledger#3090 Blockchain-dən `sumeragi`-ə zəncirvari parametrləri yaymaq

### Sabit

- hyperledger#3330 `u128` yarpaqları ilə etiketlənməmiş enum de-serializasiyasını düzəldin (RC14-ə ötürülür)
- hyperledger#2581 loglarda səs-küyü azaldıb
- hyperledger#3360 `tx/s` etalonunu düzəldin
- hyperledger#3393 `actors`-də kommunikasiya kilidini qırın
- hyperledger#3402 `nightly` quruluşunu düzəldin
- hyperledger#3411 Həmyaşıdların eyni vaxtda əlaqəsini düzgün idarə edin
- hyperledger#3440 Köçürmə zamanı aktiv çevrilmələrini ləğv edin, bunun əvəzinə smart-kontraktlar tərəfindən idarə olunur
- hyperledger#3408: `public_keys_cannot_be_burned_to_nothing` testini düzəldin

### Digər

- hyperledger#3362 `tokio` aktyorlarına köçürün
- hyperledger#3349 `EvaluateOnHost`-i ağıllı müqavilələrdən silin
- hyperledger#1786 Soket ünvanları üçün `iroha` yerli növləri əlavə edin
- IVM önbelleğini söndürün
- IVM önbelleğini yenidən aktivləşdirin
- İcazə təsdiqləyicisinin adını validator olaraq dəyişdirin
- hyperledger#3388 `model!`-i modul səviyyəli atribut makrosuna çevirin
- hyperledger#3370 `hash`-i onaltılıq sətir kimi sıralayın
- `maximum_transactions_in_block`-i `queue`-dən `sumeragi` konfiqurasiyasına köçürün
- `AssetDefinitionEntry` növünü ləğv edin və silin
- `configs/client_cli` adını `configs/client` olaraq dəyişdirin
- `MAINTAINERS.md`-i yeniləyin

## [2.0.0-pre-rc.14]

### Əlavə edilib

- hyperledger#3127 data modeli `structs` defolt olaraq qeyri-şəffafdır
- hyperledger#3122 həzm funksiyasını saxlamaq üçün `Algorithm`-dən istifadə edin (icma töhfəçisi)
- hyperledger#3153 `iroha_client_cli` çıxışı maşın oxuna bilər
- hyperledger#3105 `AssetDefinition` üçün `Transfer` tətbiq edin
- hyperledger#3010 `Transaction` boru kəməri hadisəsi əlavə edildi

### Sabit

- hyperledger # 3113 qeyri-sabit şəbəkə testlərinin yenidən nəzərdən keçirilməsi
- hyperledger#3129 `Parameter` seriyalılaşdırmanı düzəldin
- hyperledger#3141 `Hash` üçün `IntoSchema`-i əl ilə həyata keçirin
- hyperledger#3155 Testlərdə çaxnaşma çəngəlini düzəldin, çıxılmaz vəziyyətin qarşısını alın
- hyperledger#3166 Performansı yaxşılaşdıraraq boş rejimdə dəyişikliyə baxmayın
- hyperledger#2123 Multihash-dan PublicKey de/serializasiyasına qayıdın
- hyperledger#3132 NewParameter validator əlavə edin
- hyperledger#3249 Blok hashlarını qismən və tam versiyalara bölün
- hyperledger#3031 Çatışmayan konfiqurasiya parametrlərinin UI/UX-ni düzəldin
- hyperledger#3247 `sumeragi`-dən xəta inyeksiyası silindi.

### Digər

- Saxta uğursuzluqları düzəltmək üçün çatışmayan `#[cfg(debug_assertions)]` əlavə edin
- hyperledger#2133 Ağ kağıza daha yaxın olmaq üçün topologiyanı yenidən yazın
- `iroha_core`-dən `iroha_client` asılılığını silin
- hyperledger#2943 `HasOrigin` əldə edin
- hyperledger#3232 İş sahəsinin metadatasını paylaşın
- hyperledger#3254 Refaktor `commit_block()` və `replace_top_block()`
- Stabil defolt ayırıcı işləyicidən istifadə edin
- hyperledger#3183 `docker-compose.yml` fayllarının adını dəyişin
- Təkmilləşdirilmiş `Multihash` ekran formatı
- hyperledger#3268 Qlobal unikal element identifikatorları
- Yeni PR şablonu

## [2.0.0-pre-rc.13]

### Əlavə edilib- hyperledger#2399 Parametrləri ISI kimi konfiqurasiya edin.
- hyperledger#3119 `dropped_messages` metrikasını əlavə edin.
- hyperledger#3094 `n` həmyaşıdları ilə şəbəkə yaradın.
- hyperledger#3082 `Created` hadisəsində tam məlumatları təqdim edin.
- hyperledger#3021 Qeyri-şəffaf göstərici idxalı.
- hyperledger#2794 FFI-də açıq diskriminantlarla Sahəsiz nömrələri rədd edin.
- hyperledger#2922 Defolt genezise `Grant<Role>` əlavə edin.
- hyperledger#2922 `NewRole` json deserializasiyasında `inner` sahəsini buraxın.
- hyperledger#2922 json seriyasından çıxarmada `object(_id)` buraxın.
- hyperledger#2922 json deserializasiyasında `Id`-i buraxın.
- hyperledger#2922 json seriyalıləşdirmədə `Identifiable`-i buraxın.
- hyperledger#2963 Metriklərə `queue_size` əlavə edin.
- hyperledger#3027 Kür üçün kilid faylını tətbiq edin.
- hyperledger#2813 Kagami standart peer konfiqurasiyasını yaradır.
- hyperledger#3019 JSON5 dəstəyi.
- hyperledger#2231 FFI sarmalayıcı API yaradın.
- hyperledger#2999 Blok imzalarını toplayın.
- hyperledger#2995 Yumşaq çəngəl aşkarlanması.
- hyperledger#2905 `NumericValue`-i dəstəkləmək üçün arifmetik əməliyyatları genişləndirin
- hyperledger#2868 İroha versiyasını buraxın və qeydlərdə hash edin.
- hyperledger#2096 Aktivin ümumi məbləği üçün sorğu.
- hyperledger#2899 'client_cli'-ə çoxlu göstərişlər alt əmri əlavə edin
- hyperledger#2247 Veb-soket rabitə səs-küyünü çıxarın.
- hyperledger#2889 `iroha_client`-ə blok axını dəstəyi əlavə edin
- hyperledger#2280 Rol verildikdə/ləğv edildikdə icazə hadisələri yaradın.
- hyperledger#2797 Hadisələri zənginləşdirin.
- hyperledger#2725 `submit_transaction_blocking`-ə vaxt aşımını yenidən daxil edin
- hyperledger#2712 Konfiqurasiya təklifləri.
- hyperledger#2491 FFi-də enum dəstəyi.
- hyperledger#2775 Sintetik genezdə müxtəlif açarlar yaradın.
- hyperledger#2627 Konfiqurasiyanın yekunlaşdırılması, proxy giriş nöqtəsi, sənəd sənədləri.
- hyperledger#2765 `kagami`-də sintetik genezisi yaradın
- hyperledger#2698 `iroha_client`-də aydın olmayan səhv mesajını düzəldin
- hyperledger#2689 İcazə nişanı tərifi parametrləri əlavə edin.
- hyperledger#2502 GIT hash-i saxla.
- hyperledger#2672 `ipv4Addr`, `ipv6Addr` variantını və predikatları əlavə edin.
- hyperledger#2626 `Combine` əldə edin, `config` makrolarını bölün.
- hyperledger#2586 `Builder` və proxy strukturları üçün `LoadFromEnv`.
- hyperledger#2611 Ümumi qeyri-şəffaf strukturlar üçün `TryFromReprC` və `IntoFfi` əldə edin.
- hyperledger#2587 `Configurable`-i iki əlamətə bölün. #2587: `Configurable`-i iki əlamətə bölün
- hyperledger#2488 `ffi_export`-də əlamət impls üçün dəstək əlavə edin
- hyperledger#2553 Aktiv sorğularına çeşidləmə əlavə edin.
- hyperledger#2407 Parametrləşdirici tetikler.
- hyperledger#2536 FFI müştəriləri üçün `ffi_import` təqdim edin.
- hyperledger#2338 `cargo-all-features` alətləri əlavə edin.
- hyperledger#2564 Kagami alət alqoritmi seçimləri.
- hyperledger#2490 Müstəqil funksiyalar üçün ffi_export tətbiq edin.
- hyperledger#1891 Tətik icrasını təsdiq edin.
- hyperledger#1988 İdentifikasiya edilə bilən, Eq, Hash, Ord üçün makrolar əldə edin.
- hyperledger#2434 FFI bindgen kitabxanası.
- hyperledger#2073 Blokçeyndəki növlər üçün String əvəzinə ConstString-ə üstünlük verin.
- hyperledger#1889 Domen əhatəli triggerləri əlavə edin.
- hyperledger#2098 Başlıq sorğularını bloklayın. # 2098: blok başlıq sorğuları əlavə edin
- hyperledger#2467 iroha_client_cli-ə hesab qrant alt əmri əlavə edin.
- hyperledger#2301 Sorğu zamanı əməliyyatın blok hashını əlavə edin.
 - hyperledger#2454 Norito dekoder alətinə qurma skripti əlavə edin.
- hyperledger#2061 Filtrlər üçün makro əldə edin.- hyperledger#2228 Smartcontracts sorğu xətasına İcazəsiz variant əlavə edin.
- hyperledger#2395 Genesis tətbiq edilə bilməzsə, panik əlavə edin.
- hyperledger#2000 Boş adlara icazə verməyin. # 2000: Boş adlara icazə verməyin
 - hyperledger#2127 Norito kodek tərəfindən deşifrə edilmiş bütün məlumatların istehlak edildiyinə əmin olmaq üçün ağlı başında olma yoxlanışı əlavə edin.
- hyperledger#2360 `genesis.json`-i yenidən isteğe bağlı edin.
- hyperledger#2053 Şəxsi blokçeyndə qalan bütün sorğulara testlər əlavə edin.
- hyperledger#2381 `Role` qeydiyyatını birləşdirin.
- hyperledger#2053 Şəxsi blokçeynində aktivlə bağlı sorğulara testlər əlavə edin.
- hyperledger#2053 "private_blockchain"-ə testlər əlavə edin
- hyperledger#2302 'FindTriggersByDomainId' qaralama sorğusunu əlavə edin.
- hyperledger#1998 Sorğulara filtrlər əlavə edin.
- hyperledger#2276 Cari Blok hashini BlockHeaderValue daxil edin.
- hyperledger#2161 İdarə id və paylaşılan FFI fns.
- tutacaq id əlavə edin və paylaşılan xüsusiyyətlərin FFI ekvivalentlərini tətbiq edin (Clone, Eq, Ord)
- hyperledger#1638 `configuration` sənəd alt ağacını qaytarın.
- hyperledger#2132 `endpointN` proc makro əlavə edin.
- hyperledger#2257 Revoke<Role> RoleRevoked hadisəsini yayır.
- hyperledger#2125 FindAssetDefinitionById sorğu əlavə edin.
- hyperledger#1926 Siqnalla işləmə və zərif bağlanma əlavə edin.
- hyperledger#2161 `data_model` üçün FFI funksiyaları yaradır
- hyperledger#1149 Blok fayllarının sayı hər kataloq üçün 1000000-dən çox deyil.
- hyperledger#1413 API versiyasının son nöqtəsini əlavə edin.
- hyperledger#2103 bloklar və əməliyyatlar üçün sorğuları dəstəkləyir. `FindAllTransactions` sorğu əlavə edin
- hyperledger#2186 `BigQuantity` və `Fixed` üçün transfer ISI əlavə edin.
- hyperledger#2056 `AssetValueType` `enum` üçün törəmə prok makro qutusu əlavə edin.
- hyperledger#2100 Aktiv olan bütün hesabları tapmaq üçün sorğu əlavə edin.
- hyperledger#2179 Tətik icrasını optimallaşdırın.
- hyperledger#1883 Daxili konfiqurasiya fayllarını silin.
- hyperledger#2105 müştəridə sorğu xətalarını idarə edir.
- hyperledger#2050 Rola bağlı sorğular əlavə edin.
- hyperledger#1572 İxtisaslaşdırılmış icazə nişanları.
- hyperledger#2121 Klaviatura cütlüyünün qurulduqda etibarlı olub olmadığını yoxlayın.
 - hyperledger#2003 Norito Dekoder alətini təqdim edin.
- hyperledger#1952 Optimallaşdırma üçün standart olaraq TPS etalonunu əlavə edin.
- hyperledger#2040 Tranzaksiya icra limiti ilə inteqrasiya testi əlavə edin.
- hyperledger#1890 Orillion istifadə hallarına əsaslanan inteqrasiya testlərini təqdim edin.
- hyperledger#2048 Alətlər silsiləsi faylı əlavə edin.
- hyperledger#2100 Aktiv olan bütün hesabları tapmaq üçün sorğu əlavə edin.
- hyperledger#2179 Tətik icrasını optimallaşdırın.
- hyperledger#1883 Daxili konfiqurasiya fayllarını silin.
- hyperledger#2004 `isize` və `usize`-in `IntoSchema` olmasını qadağan edin.
- hyperledger#2105 müştəridə sorğu xətalarını idarə edir.
- hyperledger#2050 Rola bağlı sorğular əlavə edin.
- hyperledger#1572 İxtisaslaşdırılmış icazə nişanları.
- hyperledger#2121 Klaviatura cütlüyünün qurulduqda etibarlı olub olmadığını yoxlayın.
 - hyperledger#2003 Norito Dekoder alətini təqdim edin.
- hyperledger#1952 Optimallaşdırma üçün standart olaraq TPS etalonunu əlavə edin.
- hyperledger#2040 Tranzaksiya icra limiti ilə inteqrasiya testi əlavə edin.
- hyperledger#1890 Orillion istifadə hallarına əsaslanan inteqrasiya testlərini təqdim edin.
- hyperledger#2048 Alətlər silsiləsi faylı əlavə edin.
- hyperledger#2037 Əvvəlcədən Təhlükəli Tətikləri Təqdim edin.
- hyperledger#1621 Zəng tetikleri ilə təqdim edin.
- hyperledger#1970 Əlavə sxem son nöqtəsi əlavə edin.
- hyperledger#1620 Zamana əsaslanan triggerləri təqdim edin.
- hyperledger#1918 `client` üçün əsas autentifikasiyanı həyata keçirin
- hyperledger#1726 Buraxılış PR iş prosesini həyata keçirin.
- hyperledger#1815 Sorğu cavablarını daha çox tipli strukturlaşdırın.- hyperledger#1928 `gitchangelog` istifadə edərək dəyişiklik jurnalının yaradılmasını həyata keçirir
- hyperledger#1902 Bare metal 4-peer quraşdırma skripti.

  Docker-compose tələb etməyən və Iroha sazlama quruluşundan istifadə edən setup_test_env.sh versiyası əlavə edildi.
- hyperledger#1619 Hadisəyə əsaslanan triggerləri təqdim edin.
- hyperledger#1195 Veb-soket bağlantısını təmiz şəkildə bağlayın.
- hyperledger#1606 Domen strukturunda domen loqosuna ipfs linki əlavə edin.
- hyperledger#1754 Kür müfəttişi CLI əlavə edin.
- hyperledger#1790 Stack-əsaslı vektorlardan istifadə etməklə performansı təkmilləşdirin.
- hyperledger#1805 Panik səhvləri üçün isteğe bağlı terminal rəngləri.
- hiperledger#1749 `no_std`, `data_model`
- hyperledger#1179 Revoke-permission-or-rol təlimatını əlavə edin.
- hyperledger#1782 iroha_crypto no_std uyğun olsun.
- hyperledger#1172 Təlimat hadisələrini həyata keçirin.
- hyperledger#1734 Boşluqları istisna etmək üçün `Name` doğrulayın.
- hyperledger#1144 metadata yuvasını əlavə edin.
- #1210 Blok axını (server tərəfi).
- hyperledger#1331 Daha çox `Prometheus` ölçülərini həyata keçirin.
- hyperledger#1689 Xüsusiyyət asılılıqlarını düzəldin. # 1261: Yük qabığını əlavə edin.
- hyperledger#1675 versiyalı elementlər üçün sarğı strukturu əvəzinə tipdən istifadə edin.
- hyperledger#1643 Testlərdə həmyaşıdların genezisi həyata keçirməsini gözləyin.
- hiperledger#1678 `try_allocate`
- hyperledger#1216 Prometheus son nöqtəsini əlavə edin. # 1216: ölçülərin son nöqtəsinin ilkin tətbiqi.
- hyperledger#1238 Run-time log səviyyəli yeniləmələr. Əsas `connection` giriş nöqtəsinə əsaslanan yenidən yükləmə yaradıldı.
- hyperledger#1652 PR başlığının formatlaşdırılması.
- `Status`-ə qoşulmuş həmyaşıdların sayını əlavə edin

  - "Əlaqəli həmyaşıdların sayı ilə əlaqəli şeyləri silin" geri qaytarın

  Bu geri qaytarmalar b228b41dab3c035ce9973b6aa3b35d443c082544 edir.
  - Aydınlaşdırın `Peer` yalnız əl sıxdıqdan sonra həqiqi açıq açara malikdir
  - `DisconnectPeer` testsiz
  - Qeydiyyatdan çıxarılan həmyaşıdların icrasını həyata keçirin
  - `client_cli`-ə peer alt əmrini əlavə edin (qeydiyyatdan çıxarın).
  - Ünvanına görə qeydiyyatdan keçməmiş həmyaşıdın yenidən qoşulmasından imtina edin

  Həmyaşıdınız digər həmyaşıdını qeydiyyatdan çıxarıb əlaqəni kəsdikdən sonra,
  şəbəkəniz həmyaşıdının yenidən qoşulma sorğularını eşidəcək.
  Əvvəlcə bilə biləcəyiniz yeganə şey port nömrəsi ixtiyari olan ünvandır.
  Beləliklə, qeydiyyatdan keçməmiş həmyaşıdını port nömrəsindən başqa bir hissə ilə xatırlayın
  və oradan yenidən əlaqə qurmaqdan imtina edin
- `/status` son nöqtəsini xüsusi porta əlavə edin.

### Düzəlişlər- hyperledger#3129 `Parameter` seriyalılaşdırmanı aradan qaldırın.
- hyperledger#3109 Rol aqnostik mesajından sonra `sumeragi` yuxusunun qarşısını alın.
- hyperledger#3046 Iroha-in boş halda qəşəng başlaya biləcəyinə əmin olun
  `./storage`
- hyperledger#2599 Uşaq bağçası tüklərini çıxarın.
- hyperledger#3087 Görünüş dəyişikliyindən sonra Set B təsdiqləyicilərindən səs toplayın.
- hyperledger#3056 `tps-dev` benchmark asma vəziyyətini düzəldin.
- hyperledger#1170 Klonlama-wsv-stil yumşaq çəngəl idarəsini həyata keçirin.
- hyperledger#2456 Genesis blokunu limitsiz edin.
- hyperledger#3038 Multisigləri yenidən aktivləşdirin.
- hyperledger#2894 `LOG_FILE_PATH` env dəyişəninin seriyadan çıxarılmasını düzəldin.
- hyperledger#2803 İmza xətaları üçün düzgün status kodunu qaytarın.
- hyperledger#2963 `Queue` əməliyyatları düzgün şəkildə silin.
- hyperledger#0000 Vergen breaking CI.
- hyperledger#2165 Alət zəncirinin fidgetini çıxarın.
- hyperledger#2506 Blok təsdiqini düzəldin.
- hyperledger#3013 Yandırma validatorlarını düzgün zəncirləyin.
- hyperledger#2998 İstifadə edilməmiş Zəncir kodunu silin.
- hyperledger#2816 Bloklara giriş məsuliyyətini kuraya köçürün.
- hyperledger#2384 deşifrəni decode_all ilə əvəz edin.
- hyperledger#1967 DəyərAdını Ad ilə əvəz edin.
- hyperledger#2980 Blok dəyərinin ffi növünü düzəldin.
- hyperledger#2858 std əvəzinə parking_lot::Mutex təqdim edin.
- hyperledger#2850 `Fixed` seriyasının silinməsi/şifrinin açılmasını düzəldin
- hyperledger#2923 `AssetDefinition` etmədikdə `FindError` qaytarın
  var.
- hyperledger#0000 `panic_on_invalid_genesis.sh` düzəldin
- hyperledger#2880 Veb-soket bağlantısını düzgün bağlayın.
- hyperledger#2880 Blok axınını düzəldin.
- hyperledger#2804 `iroha_client_cli` əməliyyat bloklamasını təqdim edir.
- hyperledger#2819 Qeyri-vacib üzvləri WSV-dən çıxarın.
- İfadə serializasiyası rekursiya səhvini düzəldin.
- hyperledger#2834 stenoqram sintaksisini təkmilləşdirin.
- hyperledger#2379 blocks.txt-ə yeni Kür bloklarını atmaq imkanı əlavə edin.
- hyperledger#2758 Çeşidləmə strukturunu sxemə əlavə edin.
- CI.
- hyperledger#2548 Böyük genesis faylında xəbərdarlıq edin.
- hyperledger#2638 `whitepaper` yeniləyin və dəyişiklikləri təbliğ edin.
- hyperledger#2678 Mərhələ bölməsində testləri düzəldin.
- hyperledger#2678 Kürün güclə bağlanması zamanı testlərin dayandırılmasını düzəldin.
- hyperledger#2607 Daha sadəlik üçün sumeragi kodunun refaktoru və
  möhkəmlik düzəlişləri.
- hyperledger#2561 Konsensusa baxış dəyişikliklərini yenidən təqdim edin.
- hyperledger#2560 block_sync və peer disconnecting-də geri əlavə edin.
- hyperledger#2559 Sumeragi mövzu bağlanmasını əlavə edin.
- hyperledger#2558 WSV-ni kuradan yeniləməzdən əvvəl genezisi yoxlayın.
- hyperledger#2465 Sumeragi qovşağını tək yivli vəziyyət kimi yenidən həyata keçirin
  maşın.
- hyperledger#2449 Sumeragi Yenidən Qurulmanın ilkin icrası.
- hyperledger#2802 Konfiqurasiya üçün env yüklənməsini düzəldin.
- hyperledger#2787 Hər bir dinləyicini çaxnaşma zamanı bağlanması barədə xəbərdar edin.
- hyperledger#2764 Maksimum mesaj ölçüsü üzrə limiti silin.
- # 2571: Daha yaxşı Kura Müfəttişi UX.
- hyperledger#2703 Orillion dev env səhvlərini düzəldin.
- Şema/src-də sənəd şərhində yazı səhvini düzəldin.
- hyperledger#2716 Uptime-da müddəti ictimailəşdirin.
- hyperledger#2700 Docker şəkillərində `KURA_BLOCK_STORE_PATH` ixrac edin.
- hyperledger#0 `/iroha/rust-toolchain.toml`-i qurucudan çıxarın
  şəkil.
- hyperledger#0 `docker-compose-single.yml` düzəldin
- hyperledger#2554 `secp256k1` toxum 32-dən qısa olarsa, xətanı artırın
  bayt.
- hyperledger#0 Hər bir həmyaşıd üçün yaddaş ayırmaq üçün `test_env.sh` dəyişdirin.
- hyperledger#2457 Testlərdə kuranı zorla söndürün.
- hyperledger#2623 VariantCount üçün sənəd testini düzəldin.
- ui_fail testlərində gözlənilən xətanı yeniləyin.
- İcazə təsdiqləyicilərində səhv sənəd şərhini düzəldin.- hyperledger#2422 Konfiqurasiyanın son nöqtəsi cavabında şəxsi açarları gizlədin.
- hyperledger#2492: Hadisə ilə uyğun gələn icra olunmayan bütün tetikleyiciləri düzəldin.
- hyperledger#2504 Uğursuz tps benchmarkını düzəldin.
- hyperledger#2477 Rolların icazələri nəzərə alınmadıqda səhvi düzəldin.
- hyperledger#2416 macOS qolunda ləkələri düzəldin.
- hyperledger#2457 Çaxnaşma zamanı bağlanma ilə bağlı sınanma testlərini düzəldin.
  #2457: Panik konfiqurasiyasında bağlanma əlavə edin
- hyperledger#2473 təhlil rustc --versiya RUSTUP_TOOLCHAIN yerinə.
- hyperledger#1480 Panik zamanı söndürün. #1480: Panik zamanı proqramdan çıxmaq üçün panik çəngəl əlavə edin
- hyperledger#2376 Sadələşdirilmiş Kura, async yoxdur, iki fayl.
- hyperledger#0000 Docker qurma xətası.
- hyperledger#1649 `spawn`-i `do_send`-dən silin
- hyperledger#2128 `MerkleTree` konstruksiyasını və iterasiyasını düzəldin.
- hyperledger#2137 Çoxprosesli kontekst üçün testlər hazırlayın.
- hyperledger#2227 Aktiv üçün Qeydiyyatı həyata keçirin və Qeydiyyatdan Çıxarın.
- hyperledger#2081 Rol verilməsi səhvini düzəldin.
- hyperledger#2358 Debuq profili ilə buraxılış əlavə edin.
- hyperledger#2294 oneshot.rs-ə flameqraf nəslini əlavə edin.
- hyperledger#2202 Sorğu cavabında ümumi sahəni düzəldin.
- hyperledger#2081 Rolu vermək üçün test vəziyyətini düzəldin.
- hyperledger#2017 Rolların qeydiyyatdan çıxarılmasını düzəldin.
- hyperledger#2303 Fix docker-compose' həmyaşıdları zərif şəkildə bağlanmır.
- hyperledger#2295 Qeydiyyatdan çıxma tetikleyici səhvini düzəldin.
- hyperledger#2282 təkmilləşdirilməsi FFI getset tətbiqindən irəli gəlir.
- hyperledger#1149 nocheckin kodunu silin.
- hyperledger#2232 Genesisdə çoxlu isi olduqda Iroha mənalı mesaj çap edin.
- hyperledger#2170 M1 maşınlarında docker konteynerində qurulmasını düzəldin.
- hyperledger#2215 `cargo build` üçün 2022-04-20-i gecəni isteğe bağlı edin
- hyperledger#1990 config.json olmadıqda env vars vasitəsilə həmyaşıd başlanğıcı aktivləşdirin.
- hyperledger#2081 Rol qeydiyyatını düzəldin.
- hyperledger#1640 config.json və genesis.json yaradın.
- hyperledger#1716 f=0 halları ilə konsensus uğursuzluğunu düzəldin.
- hyperledger#1845 Satılmayan aktivlər yalnız bir dəfə zərb oluna bilər.
- hyperledger#2005 WebSocket axınını bağlamayan `Client::listen_for_events()`-ni düzəldin.
- hyperledger#1623 RawGenesisBlockBuilder yaradın.
- hyperledger#1917 easy_from_str_impl makrosunu əlavə edin.
- hyperledger#1990 config.json olmadıqda env vars vasitəsilə həmyaşıd başlanğıcı aktivləşdirin.
- hyperledger#2081 Rol qeydiyyatını düzəldin.
- hyperledger#1640 config.json və genesis.json yaradın.
- hyperledger#1716 f=0 halları ilə konsensus uğursuzluğunu düzəldin.
- hyperledger#1845 Satılmayan aktivlər yalnız bir dəfə zərb oluna bilər.
- hyperledger#2005 WebSocket axınını bağlamayan `Client::listen_for_events()`-ni düzəldin.
- hyperledger#1623 RawGenesisBlockBuilder yaradın.
- hyperledger#1917 easy_from_str_impl makrosunu əlavə edin.
- hyperledger#1922 crypto_cli-ni alətlərə köçürün.
- hyperledger#1969 `roles` funksiyasını standart xüsusiyyətlər dəstinin bir hissəsi edin.
- hyperledger#2013 Düzəliş CLI args.
- hyperledger#1897 serializasiyadan istifadə/size silin.
- hyperledger#1955 `:`-i `web_login` daxilində keçmək imkanını düzəldin
- hyperledger#1943 Sorğu xətalarını sxemə əlavə edin.
- hyperledger#1939 `iroha_config_derive` üçün uyğun xüsusiyyətlər.
- hyperledger#1908 telemetriya təhlili skripti üçün sıfır dəyər işlənməsini düzəldin.
- hyperledger#0000 Doc-testi açıq şəkildə nəzərə alınmayan edin.
- hyperledger#1848 Açıq açarların heç bir yerə yandırılmasının qarşısını alın.
- hyperledger # 1811 etibarlı həmyaşıd açarlarını silmək üçün testlər və yoxlamalar əlavə etdi.
- hyperledger#1821 MerkleTree və VersionedValidBlock üçün IntoSchema əlavə edin, HashOf və SignatureOf sxemlərini düzəldin.- hyperledger#1819 Təsdiqləmə zamanı xəta hesabatından geriyə nəzarəti silin.
- hyperledger#1774 doğrulama uğursuzluqlarının dəqiq səbəbini qeyd edin.
- hyperledger#1714 PeerId-i yalnız açarla müqayisə edin.
- hyperledger#1788 `Value` yaddaş izlərini azaldın.
- hyperledger#1804 HashOf, SignatureOf üçün sxem yaradılmasını düzəldin, heç bir sxemin əskik olmadığından əmin olmaq üçün test əlavə edin.
- hyperledger#1802 Giriş oxunuşunun təkmilləşdirilməsi.
  - hadisələr jurnalı iz səviyyəsinə köçürüldü
  - ctx qeyddən silindi
  - terminal rəngləri isteğe bağlıdır (fayllara daha yaxşı giriş çıxışı üçün)
- hyperledger # 1783 Sabit torii benchmark.
- hyperledger#1772 #1764-dən sonra düzəldin.
- hyperledger#1755 #1743, #1725 üçün kiçik düzəlişlər.
  - #1743 `Domain` struktur dəyişikliyinə uyğun olaraq JSON-ları düzəldin
- hyperledger#1751 Konsensus düzəlişləri. #1715: Yüksək yükü idarə etmək üçün konsensus düzəlişləri (#1746)
  - Dəyişikliklərlə bağlı düzəlişlərə baxın
  - Xüsusi əməliyyat hashlərindən asılı olmayaraq edilən dəyişiklik sübutlarına baxın
  - Azaldılmış mesaj ötürülməsi
  - Dərhal mesaj göndərmək əvəzinə baxış dəyişikliyi səslərini toplayın (şəbəkənin dayanıqlığını artırır)
  - Sumeragi-də Actor çərçivəsini tam istifadə edin (tapşırıq kürü yerinə mesajları özünüzə planlaşdırın)
  - Sumeragi ilə testlər üçün nasazlığı yaxşılaşdırır
  - Test kodunu istehsal koduna yaxınlaşdırır
  - Həddindən artıq mürəkkəb sarğıları çıxarır
  - Sumeragi test kodunda aktyor Kontekstindən istifadə etməyə imkan verir
- hyperledger#1734 Yeni Domain validasiyasına uyğunlaşmaq üçün genezisi yeniləyin.
- hyperledger#1742 `core` təlimatlarında qaytarılan konkret xətalar.
- hyperledger#1404 Təsdiq olunduğunu yoxlayın.
- hyperledger#1636 `trusted_peers.json` və `structopt`-ni silin
  # 1636: `trusted_peers.json` silin.
- hyperledger#1706 Topologiya yeniləməsi ilə `max_faults` yeniləməsi.
- hyperledger#1698 Sabit açıq açarlar, sənədlər və səhv mesajları.
- zərb məsələləri (1593 və 1405) buraxılış 1405

### Refaktor- Sumeragi əsas döngəsindən funksiyaları çıxarın.
- Yeni tipə `ProofChain` refaktoru.
- `Mutex`-i `Metrics`-dən çıxarın
- adt_const_generics gecə funksiyasını silin.
- hyperledger#3039 Multisigs üçün gözləmə buferini təqdim edin.
- Sumeragini sadələşdirin.
- hyperledger#3053 Kırpılmış lintləri düzəldin.
- hyperledger#2506 Blokların yoxlanılması ilə bağlı daha çox test əlavə edin.
- Kürdə `BlockStoreTrait` çıxarın.
- `nightly-2022-12-22` üçün lintsləri yeniləyin
- hyperledger#3022 `transaction_cache`-də `Option`-i silin
- hyperledger#3008 `Hash`-ə niş dəyəri əlavə edin
- Lintləri 1.65-ə yeniləyin.
- Əhatə dairəsini artırmaq üçün kiçik testlər əlavə edin.
- `FaultInjection`-dən ölü kodu çıxarın
- Sumeraglardan p2p-ə daha az zəng edin.
- hyperledger#2675 Vec ayırmadan element adlarını/idlərini təsdiq edin.
- hyperledger#2974 Tam təsdiq etmədən blokun saxtalaşdırılmasının qarşısını alın.
- kombinatorlarda daha səmərəli `NonEmpty`.
- hyperledger#2955 Bloku BlockSigned mesajından silin.
- hyperledger#1868 Təsdiqlənmiş əməliyyatların göndərilməsinin qarşısını alın
  həmyaşıdları arasında.
- hyperledger#2458 Ümumi kombinator API tətbiq edin.
- Gitignore-a yaddaş qovluğu əlavə edin.
- hyperledger#2909 Növbəti üçün Hardcode portları.
- hyperledger#2747 `LoadFromEnv` API dəyişdirin.
- Konfiqurasiya uğursuzluğu ilə bağlı səhv mesajlarını təkmilləşdirin.
- `genesis.json`-ə əlavə nümunələr əlavə edin
- `rc9` buraxılışından əvvəl istifadə olunmamış asılılıqları silin.
- Yeni Sumeragi üzərində lintingi yekunlaşdırın.
- Əsas döngədə alt prosedurları çıxarın.
- hyperledger#2774 `kagami` genesis generasiya rejimini bayraqdan dəyişdirin
  alt komanda.
- hyperledger#2478 `SignedTransaction` əlavə edin
- hyperledger#2649 `byteorder` qutusunu `Kura`-dən çıxarın
- `DEFAULT_BLOCK_STORE_PATH` adını `./blocks`-dən `./storage`-ə dəyişdirin
- hyperledger#2650 Iroha submodullarını bağlamaq üçün `ThreadHandler` əlavə edin.
- hyperledger#2482 `Account` icazə nişanlarını `Wsv`-də saxlayın
- 1.62-ə yeni lints əlavə edin.
- `p2p` səhv mesajlarını təkmilləşdirin.
- hyperledger#2001 `EvaluatesTo` statik növün yoxlanılması.
- hyperledger#2052 İcazə nişanlarını təriflə qeyd oluna bilən edin.
  # 2052: PermissionTokenDefinition tətbiq edin
- Bütün funksiya birləşmələrinin işləməsini təmin edin.
- hyperledger#2468 İcazə təsdiqləyicilərindən debug supertraitini silin.
- hyperledger#2419 Açıq `drop`-ləri silin.
- hyperledger#2253 `Registrable` xüsusiyyətini `data_model`-ə əlavə edin
- Məlumat hadisələri üçün `Identifiable` əvəzinə `Origin` tətbiq edin.
- hyperledger#2369 Refaktor icazəsi təsdiqləyiciləri.
- hyperledger#2307 `events_sender`-i `WorldStateView`-də qeyri-opsional edin.
- hyperledger#1985 `Name` strukturunun ölçüsünü azaldın.
- Daha çox `const fn` əlavə edin.
- `default_permissions()` istifadə edərək inteqrasiya testlərini aparın
- private_blockchain-ə icazə nişanı sarğıları əlavə edin.
- hyperledger#2292 `WorldTrait`-i silin, `IsAllowedBoxed`-dən generikləri silin
- hyperledger#2204 Aktivlə əlaqəli əməliyyatları ümumiləşdirin.
- hyperledger#2233 `impl`-i `Display` və `Debug` üçün `derive` ilə əvəz edin.
- Müəyyən edilə bilən struktur təkmilləşdirmələri.
- hyperledger#2323 Kura başlanğıc səhv mesajını gücləndirin.
- hyperledger#2238 Testlər üçün peer builder əlavə edin.
- hyperledger#2011 Daha çox təsviri konfiqurasiya parametrləri.
- hyperledger#1896 `produce_event` tətbiqini sadələşdirin.
- `QueryError` ətrafında refaktor.
- `TriggerSet`-i `data_model`-ə köçürün.
- hyperledger#2145 refaktor müştərinin `WebSocket` tərəfi, təmiz məlumat məntiqini çıxarın.
- `ValueMarker` xüsusiyyətini çıxarın.
- hyperledger#2149 `prelude`-də `Mintable` və `MintabilityError`-i ifşa edin
- hyperledger#2144 müştərinin http iş prosesini yenidən dizayn edin, daxili api-ni ifşa edin.- `clap`-ə keçin.
- `iroha_gen` binar, birləşdirici sənədlər, schema_bin yaradın.
- hyperledger#2109 `integration::events::pipeline` testini stabil edin.
- hyperledger#1982 `iroha_crypto` strukturlarına girişi əhatə edir.
- `AssetDefinition` qurucusunu əlavə edin.
- API-dən lazımsız `&mut` silin.
- verilənlər modeli strukturlarına girişi əhatə edir.
- hyperledger#2144 müştərinin http iş prosesini yenidən dizayn edin, daxili api-ni ifşa edin.
- `clap`-ə keçin.
- `iroha_gen` binar, birləşdirici sənədlər, schema_bin yaradın.
- hyperledger#2109 `integration::events::pipeline` testini stabil edin.
- hyperledger#1982 `iroha_crypto` strukturlarına girişi əhatə edir.
- `AssetDefinition` qurucusunu əlavə edin.
- API-dən lazımsız `&mut` silin.
- verilənlər modeli strukturlarına girişi əhatə edir.
- Əsas, `sumeragi`, nümunə funksiyaları, `torii`
- hyperledger#1903 hadisə emissiyasını `modify_*` metodlarına köçürün.
- `data_model` lib.rs faylını bölün.
- Növbəyə ws istinad əlavə edin.
- hyperledger#1210 Split hadisə axını.
  - Tranzaksiya ilə əlaqəli funksionallığı data_model/transaction moduluna köçürün
- hyperledger#1725 Torii-də qlobal vəziyyəti silin.
  - `add_state macro_rules` tətbiq edin və `ToriiState`-i çıxarın
- Linter səhvini düzəldin.
- hyperledger#1661 `Cargo.toml` təmizlənməsi.
  - Yük asılılıqlarını sıralayın
- hyperledger#1650 `data_model`-i səliqəyə salın
  - Dünyanı wsv-ə köçürün, rollar xüsusiyyətini düzəldin, CommittedBlock üçün IntoSchema əldə edin
- `json` fayllarının və readme-nin təşkili. Şablonla uyğunlaşmaq üçün Readme-ni yeniləyin.
- 1529: strukturlaşdırılmış giriş.
  - Refactor log mesajları
- `iroha_p2p`
  - P2p özəlləşdirmə əlavə edin.

### Sənədləşmə

- Iroha Client CLI readme-ni yeniləyin.
- Dərslik hissələrini yeniləyin.
- API spesifikasiyasına 'sort_by_metadata_key' əlavə edin.
- Sənədlərə keçidləri yeniləyin.
- Təlimatı aktivlə əlaqəli sənədlərlə genişləndirin.
- Köhnəlmiş sənəd fayllarını silin.
- Durğu işarələrini nəzərdən keçirin.
- Bəzi sənədləri təlimat deposuna köçürün.
- Səhnələşdirmə filialı üçün qabarıqlıq hesabatı.
- Pre-rc.7 üçün dəyişiklik jurnalını yaradın.
- İyulun 30-na olan ləkəlilik hesabatı.
- Bump versiyaları.
- Test qırıqlığını yeniləyin.
- hyperledger#2499 client_cli xəta mesajlarını düzəldin.
- hyperledger#2344 2.0.0-pre-rc.5-lts üçün CHANGELOG yaradın.
- Dərsliyə keçidlər əlavə edin.
- Git hooks haqqında məlumatı yeniləyin.
- qırıqlıq testinin yazılması.
- hyperledger#2193 Iroha müştəri sənədlərini yeniləyin.
- hyperledger#2193 Yeniləmə Iroha CLI sənədləri.
- hyperledger#2193 Makro qutu üçün README-i yeniləyin.
 - hyperledger#2193 Yeniləmə Norito Dekoder Aləti sənədləri.
- hyperledger#2193 Kagami sənədlərini yeniləyin.
- hyperledger#2193 Benchmark sənədlərini yeniləyin.
- hyperledger#2192 Töhfə verən təlimatları nəzərdən keçirin.
- Qırılmış koddaxili istinadları düzəldin.
- hyperledger#1280 Sənəd Iroha ölçüləri.
- hyperledger#2119 Iroha-i Docker konteynerinə yenidən yükləmək üçün təlimat əlavə edin.
- hyperledger#2181 README-i nəzərdən keçirin.
- hyperledger#2113 Cargo.toml fayllarında sənəd xüsusiyyətləri.
- hyperledger#2177 Gitchangelog çıxışını təmizləyin.
- hyperledger#1991 Readme əlavə edin Kür inspektoru.
- hyperledger#2119 Iroha-i Docker konteynerinə yenidən yükləmək üçün təlimat əlavə edin.
- hyperledger#2181 README-i nəzərdən keçirin.
- hyperledger#2113 Cargo.toml fayllarında sənəd xüsusiyyətləri.
- hyperledger#2177 Gitchangelog çıxışını təmizləyin.
- hyperledger#1991 Readme əlavə edin Kür inspektoru.
- ən son dəyişiklik jurnalını yaradın.
- Dəyişikliklər jurnalını yaradın.
- Köhnəlmiş README fayllarını yeniləyin.
- `api_spec.md`-ə çatışmayan sənədlər əlavə edildi.

### CI/CD dəyişiklikləri- Özünə ev sahibliyi edən daha beş idmançı əlavə edin.
- Soramitsu reyestrinə müntəzəm şəkil etiketi əlavə edin.
- Libgit2-sys 0.5.0 üçün həll yolu. 0.4.4-ə qayıt.
- Arxa əsaslı təsvirdən istifadə etməyə çalışın.
- Yalnız gecəlik yeni konteynerdə işləmək üçün iş axınlarını yeniləyin.
- İkili giriş nöqtələrini əhatə dairəsindən çıxarın.
- İnkişaf testlərini Equinix-in özünün idarə etdiyi qaçışçılara keçirin.
- hyperledger#2865 `scripts/check.sh`-dən tmp faylının istifadəsini silin
- hyperledger#2781 Əhatə dairəsi ofsetləri əlavə edin.
- Yavaş inteqrasiya testlərini söndürün.
- Əsas təsviri docker keşi ilə əvəz edin.
- hyperledger#2781 codecov commit ana funksiyasını əlavə edin.
- İşləri github qaçışçılarına köçürün.
- hyperledger#2778 Müştəri konfiqurasiyasının yoxlanılması.
- hyperledger#2732 İroha2 əsaslı şəkilləri yeniləmək və əlavə etmək üçün şərtlər əlavə edin
  PR etiketləri.
- Gecə görüntü quruluşunu düzəldin.
- `docker/build-push-action` ilə `buildx` səhvini düzəldin
- İşləməyən `tj-actions/changed-files` üçün ilk yardım
- #2662-dən sonra şəkillərin ardıcıl dərcini aktivləşdirin.
- Liman reyestrini əlavə edin.
- Avtomatik etiket `api-changes` və `config-changes`
- Şəkildə hash edin, yenidən alətlər faylı, UI izolyasiyası,
  sxem izləmə.
- Nəşretmə iş axınlarını ardıcıl edin və №2427-i tamamlayın.
- hyperledger#2309: CI-də sənəd testlərini yenidən aktivləşdirin.
- hyperledger#2165 Codecov quraşdırmasını silin.
- Cari istifadəçilərlə münaqişələrin qarşısını almaq üçün yeni konteynerə keçin.
 - hyperledger#2158 `parity_scale_codec` və digər asılılıqları təkmilləşdirin. (Norito kodek)
- Quraşdırmanı düzəldin.
- hyperledger#2461 iroha2 CI-ni təkmilləşdirin.
- `syn`-i yeniləyin.
- əhatə dairəsini yeni iş axınına köçürün.
- reverse docker login ver.
- `archlinux:base-devel` versiya spesifikasiyasını silin
- Dockerfiles və Codecov hesabatlarının təkrar istifadəsi və paralelliyini yeniləyin.
- Dəyişikliklər jurnalını yaradın.
- `cargo deny` faylı əlavə edin.
- `iroha2`-dən kopyalanan iş axını ilə `iroha2-lts` filialı əlavə edin
- hyperledger#2393 Docker əsas təsvirin versiyasını sındırın.
- hyperledger#1658 Sənəd yoxlamasını əlavə edin.
- Versiya qutularının qabarması və istifadə olunmamış asılılıqları çıxarın.
- Lazımsız əhatə dairəsi hesabatını çıxarın.
- hyperledger#2222 Testləri əhatə edib-etməməsinə görə bölün.
- hyperledger#2153 Fix #2154.
- Versiya qutuları bütün qabar.
- Yerləşdirmə boru kəmərini düzəldin.
- hyperledger#2153 Əhatə dairəsini düzəldin.
- Genesis yoxlaması əlavə edin və sənədləri yeniləyin.
- Pas, kif və gecəni müvafiq olaraq 1.60, 1.2.0 və 1.62-yə çırpın.
- load-rs tetikleyicileri.
- hyperledger#2153 Fix #2154.
- Versiya qutuları bütün qabar.
- Yerləşdirmə boru kəmərini düzəldin.
- hyperledger#2153 Əhatə dairəsini düzəldin.
- Genesis yoxlaması əlavə edin və sənədləri yeniləyin.
- Pas, küf və gecəni müvafiq olaraq 1,60, 1,2,0 və 1,62-yə çırpın.
- load-rs tetikleyicileri.
- load-rs: iş axını tetikleyicilerini buraxın.
- Təkan iş axını düzəldin.
- Defolt funksiyalara telemetriya əlavə edin.
- əsas iş axını təkan üçün müvafiq etiket əlavə edin.
- uğursuz testləri düzəldin.
- hyperledger#1657 Şəkili pas 1.57 üçün yeniləyin. # 1630: Özünə ev sahibliyi edən qaçışçılara qayıdın.
- CI təkmilləşdirilməsi.
- `lld` istifadə etmək üçün əhatə dairəsi dəyişdirildi.
- CI asılılığının düzəldilməsi.
- CI seqmentasiyası təkmilləşdirilməsi.
- CI-də sabit Rust versiyasından istifadə edir.
- Docker nəşrini düzəldin və iroha2-dev təkan CI. Əhatə dairəsini və dəzgahı PR-a köçürün
- CI docker testində lazımsız tam Iroha quruluşunu silin.

  Iroha quruluşu indi docker görüntüsünün özündə edildiyi üçün yararsız oldu. Beləliklə, CI yalnız testlərdə istifadə olunan müştəri cli qurur.
- CI boru kəmərində iroha2 filialı üçün dəstək əlavə edin.
  - uzun testlər yalnız iroha2-də PR-də keçdi
  - docker şəkillərini yalnız iroha2-dən dərc edin
- Əlavə CI keşləri.

### Veb Assambleyası


### Versiya zərbələri- Pre-rc.13 versiyası.
- Pre-rc.11 versiyası.
- RC.9 versiyası.
- RC.8 versiyası.
- Versiyaları RC7-yə yeniləyin.
- Buraxılışdan əvvəl hazırlıqlar.
- Kalıp 1.0-ı yeniləyin.
- Bump asılılıqları.
- api_spec.md-ni yeniləyin: sorğu/cavab orqanlarını düzəldin.
- Pas versiyasını 1.56.0-a yeniləyin.
- Töhfə bələdçisini yeniləyin.
- Yeni API və URL formatına uyğunlaşdırmaq üçün README.md və `iroha/config.json`-i yeniləyin.
- Docker nəşr hədəfini hyperledger/iroha2 #1453-ə yeniləyin.
- İş axınını əsasla uyğunlaşdırmaq üçün yeniləyir.
- API spesifikasiyasını yeniləyin və sağlamlıq son nöqtəsini düzəldin.
- Rust yeniləməsi 1.54.
- Sənədlər(iroha_crypto): `Signature` sənədlərini yeniləyin və `verify` arxlarını uyğunlaşdırın
- Ursa versiyası 0.3.5-dən 0.3.6-a qədər yüksəlir.
- İş axınını yeni qaçışçılara yeniləyin.
- Keşləmə və daha sürətli ci qurmaları üçün docker faylını yeniləyin.
- Libssl versiyasını yeniləyin.
- Docker fayllarını və async-std-ni yeniləyin.
- Yenilənmiş klipi düzəldin.
- Aktiv strukturunu yeniləyir.
  - Aktivdə açar-dəyər təlimatlarına dəstək
  - Enum kimi aktiv növləri
  - ISI aktivində daşqın zəifliyi
- Bələdçiyə töhfə verən yeniləmələr.
- Köhnəlmiş libi yeniləyin.
- Ağ kağızı yeniləyin və linting problemlərini həll edin.
- Cucumber_rust lib-i yeniləyin.
- Açar yaratmaq üçün README yeniləmələri.
- Github Actions iş axınlarını yeniləyin.
- Github Actions iş axınlarını yeniləyin.
- Tələblər.txt faylını yeniləyin.
- common.yaml-ı yeniləyin.
- Saradan sənəd yeniləmələri.
- Təlimat məntiqini yeniləyin.
- Ağ kağızı yeniləyin.
- Şəbəkə funksiyalarının təsvirini yeniləyir.
- Şərhlər əsasında ağ kağızı yeniləyin.
- WSV yeniləməsinin ayrılması və Scale-ə köçməsi.
- Gitignore-u yeniləyin.
- WP-də kuranın bir qədər təsvirini yeniləyin.
- Ağ kağızda kura haqqında təsviri yeniləyin.

### Sxem

- hyperledger#2114 Sxemlərdə çeşidlənmiş kolleksiyaları dəstəkləyir.
- hyperledger#2108 Səhifə əlavə edin.
- hyperledger#2114 Sxemlərdə çeşidlənmiş kolleksiyaları dəstəkləyir.
- hyperledger#2108 Səhifə əlavə edin.
- Sxem, versiya və makro no_std uyğun olsun.
- Sxemdə imzaları düzəldin.
- Sxemdə `FixedPoint`-in dəyişdirilmiş təmsili.
- Sxem introspeksiyasına `RawGenesisBlock` əlavə edildi.
- IR-115 sxemini yaratmaq üçün obyekt modelləri dəyişdirildi.

### Testlər

- hyperledger#2544 Dərslik testləri.
- hyperledger#2272 'FindAssetDefinitionById' sorğusu üçün testlər əlavə edin.
- `roles` inteqrasiya testlərini əlavə edin.
- Ui test formatını standartlaşdırın, sandıqlar əldə etmək üçün əldə edilən ui testlərini köçürün.
- Sınaq testləri düzəldin (fyuçers sıralanmamış səhv).
- DSL qutusu çıxarıldı və testlər `data_model`-ə köçürüldü
- Qeyri-sabit şəbəkə testlərinin etibarlı kod üçün keçdiyinə əmin olun.
- iroha_p2p-ə testlər əlavə edildi.
- Test uğursuz olmadıqca testlərdə qeydləri ələ keçirir.
- Testlər üçün sorğu əlavə edin və nadir hallarda pozulan testləri düzəldin.
- Paralel quraşdırmanı sınaqdan keçirir.
- iroha init və iroha_client testlərindən kökü çıxarın.
- Testləri düzəldin və ci-yə yoxlamalar əlavə edin.
- Benchmark testləri zamanı `tx` doğrulama səhvlərini düzəldin.
- hyperledger#860: Iroha Sorğular və testlər.
- Iroha xüsusi ISI bələdçisi və Xiyar testləri.
- No-std müştəri üçün testlər əlavə edin.
- Körpü qeydiyyatı dəyişiklikləri və testləri.
- Şəbəkə istehza ilə konsensus testləri.
- Testlərin icrası üçün temp dir-dən istifadə.
- Benches müsbət halları test edir.
- Testlərlə ilkin Merkle Tree funksionallığı.
- Sabit testlər və World State View işə salınması.

### Digər- Parametrləşdirməni əlamətlərə köçürün və FFI IR növlərini çıxarın.
- Həmkarlar ittifaqları üçün dəstək əlavə edin, `non_robust_ref_mut` təqdim edin * conststring FFI çevrilməsini həyata keçirin.
- IdOrdEqHash-i təkmilləşdirin.
- FilterOpt::BySome-ni (de-)serializasiyadan çıxarın.
- Şəffaf olun.
- ContextValue şəffaf olun.
- İfadə edin::Raw etiketi isteğe bağlıdır.
- Bəzi təlimatlar üçün şəffaflıq əlavə edin.
- RoleId seriyasını təkmilləşdirin (de-)
- Validator ::Id-nin seriyalaşdırılmasını təkmilləşdirin (de-)
- PermissionTokenId seriyasını təkmilləşdirin (de-)
- TriggerId seriyasını təkmilləşdirin (de-)
- Aktivin (-Tərif) İdlərinin seriyalaşdırılmasını təkmilləşdirin.
- AccountId seriyasını təkmilləşdirin (de-)
- Ipfs və DomainId-in serializasiyasını təkmilləşdirin (de-)
- Müştəri konfiqurasiyasından logger konfiqurasiyasını çıxarın.
- FFI-də şəffaf strukturlar üçün dəstək əlavə edin.
- Refactor &Option<T> to Variant<&T>
- Kəsmə xəbərdarlıqlarını düzəldin.
- `Find` xəta təsvirində daha çox təfərrüat əlavə edin.
- `PartialOrd` və `Ord` tətbiqlərini düzəldin.
- `cargo fmt` əvəzinə `rustfmt` istifadə edin
- `roles` xüsusiyyətini silin.
- `cargo fmt` əvəzinə `rustfmt` istifadə edin
- İş dirəyini dev docker nümunələri ilə həcm kimi paylaşın.
- Execute-də Fərqlə əlaqəli növü silin.
- Multival qaytarılması əvəzinə xüsusi kodlaşdırmadan istifadə edin.
- Serde_json-u iroha_crypto asılılığı kimi çıxarın.
- Versiya atributunda yalnız məlum sahələrə icazə verin.
- Son nöqtələr üçün müxtəlif portları aydınlaşdırın.
- `Io` törəməni çıxarın.
- Açar_cütlərinin ilkin sənədləri.
- Özünə ev sahibliyi edən idmançılara qayıdın.
- Kodda yeni klip lintslərini düzəldin.
- i1i1-i baxıcılardan çıxarın.
- Aktyor doc və kiçik düzəlişlər əlavə edin.
- Ən son blokları itələmək əvəzinə sorğu.
- 7 həmyaşıdın hər biri üçün sınaqdan keçirilmiş əməliyyat statusu hadisələri.
- `join_all` əvəzinə `FuturesUnordered`
- GitHub Runners-ə keçin.
- /sorğu son nöqtəsi üçün VersionedQueryResult vs QueryResult istifadə edin.
- Telemetriyanı yenidən birləşdirin.
- Dependabot konfiqurasiyasını düzəldin.
- Signoff daxil etmək üçün commit-msg git hook əlavə edin.
- Təkan boru kəmərini düzəldin.
- Bağlı robotu təkmilləşdirin.
- Növbə təkanında gələcək zaman damğasını aşkar edin.
- hyperledger#1197: Kür səhvləri idarə edir.
- Qeydiyyatdan çıxma təlimatı əlavə edin.
- Tranzaksiyaları fərqləndirmək üçün isteğe bağlı olmayan əlavə edin. Bağlayın # 1493.
- Lazımsız `sudo` silindi.
- Domenlər üçün metadata.
- `create-docker` iş prosesində təsadüfi sıçrayışları düzəldin.
- Arızalı boru kəmərinin təklif etdiyi kimi `buildx` əlavə edildi.
- hyperledger#1454: Xüsusi status kodu və göstərişlər ilə sorğu xətası cavabını düzəldin.
- hyperledger#1533: tranzaksiyanı hash əsasında tapın.
- `configure` son nöqtəsini düzəldin.
- Boolean-əsaslı aktivlərin zərf olunma yoxlanışı əlavə edin.
- Tipli kripto primitivlərin əlavə edilməsi və tip təhlükəsiz kriptoqrafiyaya miqrasiya.
- Giriş təkmilləşdirmələri.
- hyperledger#1458: `mailbox` kimi konfiqurasiya etmək üçün aktyor kanalının ölçüsünü əlavə edin.
- hyperledger#1451: `faulty_peers = 0` və `trusted peers count > 1` olduqda səhv konfiqurasiya haqqında xəbərdarlıq əlavə edin
- Xüsusi blok hash əldə etmək üçün işləyici əlavə edin.
- FindTransactionByHash yeni sorğu əlavə edildi.
- hyperledger#1185: Kassaların adını və yolunu dəyişdirin.
- Qeydləri və ümumi təkmilləşdirmələri düzəldin.
- hyperledger#1150: 1000 bloku hər fayla qruplaşdırın
- Növbə stres testi.
- Giriş səviyyəsinin düzəldilməsi.
- Müştəri kitabxanasına başlıq spesifikasiyası əlavə edin.
- Növbə çaxnaşma uğursuzluq aradan qaldırılması.
- Düzəltmə növbəsi.
- Dockerfile buraxılış quruluşunu düzəldin.
- Https müştəri düzəlişi.
- Sürətləndirmə ci.
- 1. İroha_crypto istisna olmaqla, bütün ursa asılılıqları silindi.
- Müddətləri çıxararkən daşmanı düzəldin.
- Müştəridə sahələri ictimailəşdirin.
- Iroha2-ni hər gecə Dockerhub-a itələyin.
- http status kodlarını düzəldin.
- iroha_error-u thiserror, eyre və color-eyre ilə əvəz edin.
- Növbəni çarpaz biri ilə əvəz edin.- Bəzi faydasız lint ehtiyatlarını çıxarın.
- Aktiv tərifləri üçün metadata təqdim edir.
- Test_şəbəkə qutusundan arqumentlərin çıxarılması.
- Lazımsız asılılıqları aradan qaldırın.
- iroha_client_cli:: hadisələrini düzəldin.
- hyperledger#1382: Köhnə şəbəkə tətbiqini silin.
- hyperledger#1169: Aktivlər üçün əlavə dəqiqlik.
- Həmyaşıd başlanğıcda təkmilləşdirmələr:
  - Genesis açıq açarını yalnız env-dən yükləməyə imkan verir
  - konfiqurasiya, genesis və etibarlı_peers yolu indi cli parametrlərində göstərilə bilər
- hyperledger#1134: Iroha P2P inteqrasiyası.
- Sorğunun son nöqtəsini GET əvəzinə POST olaraq dəyişdirin.
- Aktyorda on_start-ı sinxron şəkildə yerinə yetirin.
- Çarpmağa köç.
- Broker səhvlərinin düzəldilməsi ilə yenidən işləmək.
- "Birdən çox broker düzəlişlərini təqdim edir" öhdəliyini geri qaytarın (9c148c33826067585b5868d297dcdd17c0efe246)
- Çoxlu broker düzəlişlərini təqdim edir:
  - Aktyor dayanacağında brokerin abunəliyini dayandırın
  - Eyni aktyor növündən birdən çox abunəni dəstəkləyin (əvvəllər TODO idi)
  - Brokerin həmişə özünü aktyor identifikatoru kimi qoyduğu səhvi düzəldin.
- Broker səhvi (test vitrini).
- Məlumat modeli üçün törəmələr əlavə edin.
- Torii-dən rwlock çıxarın.
- OOB Sorğu İcazə Yoxlamaları.
- hyperledger#1272: Həmyaşıdların saylarının həyata keçirilməsi,
- Təlimat daxilində sorğu icazələri üçün rekursiv yoxlama.
- Dayanacaq aktyorlarını planlaşdırın.
- hyperledger#1165: Həmyaşıdların saylarının həyata keçirilməsi.
- Torii son nöqtəsində hesabla sorğu icazələrini yoxlayın.
- Sistem ölçülərində CPU və yaddaş istifadəsini ifşa edən silindi.
 - WS mesajları üçün JSON-u Norito ilə əvəz edin.
- Görünüş dəyişikliklərinin sübutunu saxlayın.
- hyperledger#1168: Tranzaksiya imza yoxlaması şərtini keçmirsə, qeyd əlavə edildi.
- Sabit kiçik problemlər, əlavə qoşulma dinləmək kodu.
- Şəbəkə topologiyası qurucusunu təqdim edin.
- Iroha üçün P2P şəbəkəsini tətbiq edin.
- Blok ölçüsü ölçüsünü əlavə edir.
- PermissionValidator xüsusiyyətinin adı IsAllowed olaraq dəyişdirildi. və müvafiq digər ad dəyişiklikləri
- API spesifikasiyalı veb yuva düzəlişləri.
- Docker görüntüsündən lazımsız asılılıqları aradan qaldırır.
- Fmt Crate import_granularity istifadə edir.
- Ümumi İcazə Validatorunu təqdim edir.
- Aktyor çərçivəsinə keçin.
- Broker dizaynını dəyişdirin və aktyorlara bəzi funksionallıq əlavə edin.
- Codecov status yoxlamalarını konfiqurasiya edir.
- Grcov ilə mənbə əsaslı əhatə dairəsindən istifadə edir.
- Sabit çoxlu qurma-arg formatı və aralıq tikinti konteynerləri üçün yenidən elan edilmiş ARG.
- SubscriptionAccepted mesajını təqdim edir.
- Əməliyyatdan sonra sıfır dəyərli aktivləri hesablardan silin.
- Sabit docker qurma arqumentləri formatı.
- Uşaq bloku tapılmadıqda səhv mesajı düzəldildi.
- Qurmaq üçün təchiz edilmiş OpenSSL əlavə edildi, pkg-konfiqurasiya asılılığını düzəldir.
- Dockerhub və əhatə dairəsi fərqi üçün repozitoriya adını düzəldin.
- TrustedPeers yüklənə bilmədiyi halda aydın xəta mətni və fayl adı əlavə edildi.
- Mətn obyektləri sənədlərdəki keçidlərə dəyişdirildi.
- Docker nəşrində səhv istifadəçi adı sirrini düzəldin.
- Ağ kağızda kiçik yazı səhvini düzəldin.
- Daha yaxşı fayl strukturu üçün mod.rs istifadə etməyə imkan verir.
- main.rs-ni ayrı qutuya köçürün və ictimai blokçeyn üçün icazələr verin.
- Müştəri cli daxilində sorğu əlavə edin.
- Cli üçün clap-dan structopts-a köçürün.
- Telemetriyanı qeyri-sabit şəbəkə testi ilə məhdudlaşdırın.
- Xüsusiyyətləri smartcontracts moduluna köçürün.
- Sed -i "s/world_state_view/wsv/g"
- Ağıllı müqavilələri ayrı modula köçürün.
- Iroha şəbəkə məzmun uzunluğu səhvi düzəldib.
- Aktyor identifikatoru üçün yerli tapşırıq yaddaşı əlavə edir. Kilidin aşkarlanması üçün faydalıdır.
- CI-ə kilidin aşkarlanması testini əlavə edin
- Introspect makro əlavə edin.
- İş axını adlarını, həmçinin formatlaşdırma düzəlişlərini aydınlaşdırır
- Sorğu api-nin dəyişdirilməsi.
- Async-std-dən tokioya miqrasiya.
- ci-yə telemetriya təhlili əlavə edin.- İroha üçün fyuçers telemetriyası əlavə edin.
- Hər bir async funksiyasına iroha fyuçersləri əlavə edin.
- Anketlərin sayının müşahidə oluna bilməsi üçün iroha fyuçersləri əlavə edin.
- README-ə əl ilə yerləşdirmə və konfiqurasiya əlavə edildi.
- Reportyor düzəlişi.
- Alma Mesajı makrosunu əlavə edin.
- Sadə aktyor çərçivəsi əlavə edin.
- Dependabot konfiqurasiyası əlavə edin.
- Gözəl panik və səhv müxbirləri əlavə edin.
- Rust versiyasının 1.52.1-ə köçürülməsi və müvafiq düzəlişlər.
- Ayrı-ayrı mövzularda CPU intensiv tapşırıqlarını bloklayan kürü.
- Crates.io saytından unikal_port və yük lintlərindən istifadə edin.
- Kilidsiz WSV üçün düzəliş:
  - API-də lazımsız Dashmap və kilidləri silir
  - yaradılmış blokların həddindən artıq sayı ilə səhvləri düzəldir (rədd edilən əməliyyatlar qeydə alınmadı)
  - Səhvlərin tam səbəbini göstərir
- Telemetriya abunəçisi əlavə edin.
- Rollar və icazələr üçün sorğular.
- Blokları kuradan wsv-ə köçürün.
- WSV daxilində kilidsiz məlumat strukturlarına keçin.
- Şəbəkə fasiləsinin düzəldilməsi.
- Sağlamlığın son nöqtəsini düzəldin.
- Rolları təqdim edir.
- Dev filialından təkan docker şəkilləri əlavə edin.
- Daha aqressiv linting əlavə edin və koddan panikləri çıxarın.
- Təlimatlar üçün Execute xüsusiyyətinin yenidən işlənməsi.
- Köhnə kodu iroha_config-dən silin.
- IR-1060 bütün mövcud icazələr üçün Qrant yoxlamalarını əlavə edir.
- iroha_network üçün ulimit və vaxt aşımını düzəldin.
- Ci fasilə testi düzəlişi.
- Tərifləri çıxarıldıqda bütün aktivləri silin.
- Aktiv əlavə edərkən wsv panikasını düzəldin.
- Kanallar üçün Arc və Rwlock-u çıxarın.
- Iroha şəbəkə düzəlişi.
- İcazə Təsdiqləyiciləri çeklərdə istinadlardan istifadə edirlər.
- Qrant Təlimatı.
- NewAccount, Domain və AssetDefinition IR-1036 üçün sətir uzunluğu məhdudiyyətləri və id-lərin təsdiqi üçün əlavə konfiqurasiya.
- İzləmə libi ilə jurnalı əvəz edin.
- Sənədləri yoxlayın və dbg makrosunu rədd edin.
- Verilən icazələri təqdim edir.
- iroha_config qutusu əlavə edin.
- Bütün daxil olan birləşmə sorğularını təsdiqləmək üçün kod sahibi kimi @alerdenisov əlavə edin.
- Konsensus zamanı əməliyyat ölçüsünün yoxlanılmasının düzəldilməsi.
- Async-std yeniləməsini geri qaytarın.
- Bəzi sabitləri 2 IR-1035 gücü ilə əvəz edin.
- IR-1024 əməliyyat tarixçəsini əldə etmək üçün sorğu əlavə edin.
- İcazə təsdiqləyicilərinin saxlanması və yenidən qurulması üçün icazələrin təsdiqini əlavə edin.
- Hesabın qeydiyyatı üçün NewAccount əlavə edin.
- Aktiv tərifi üçün növlər əlavə edin.
- Konfiqurasiya edilə bilən metadata məhdudiyyətlərini təqdim edir.
- Tranzaksiya metadatasını təqdim edir.
- Sorğulara ifadələr əlavə edin.
- Lints.toml əlavə edin və xəbərdarlıqları düzəldin.
- Config.json-dan etibarlı_peers-i ayırın.
- Telegram-da Iroha 2 icmasına URL-də yazı səhvini düzəldin.
- Kəsmə xəbərdarlıqlarını düzəldin.
- Hesab üçün açar-dəyər metadata dəstəyini təqdim edir.
- Blokların versiyalarını əlavə edin.
- Təkrarlamaları düzəldin.
- mul,div,mod,raise_to ifadələri əlavə edin.
- Versiyalaşdırma üçün into_v* əlavə edin.
- Səhv makrosu ilə Xəta::msg əvəz edin.
- iroha_http_server-i yenidən yazın və səhvləri yenidən işləyin.
 - Norito versiyasını 2-yə yüksəldir.
- Whitepaper versiyasının təsviri.
- Yanlış səhifələmə. Səhifələndirmənin səhvlər səbəbindən lazımsız ola biləcəyi, bunun əvəzinə boş kolleksiyaları qaytarmadığı halları düzəldin.
- Sadalamalar üçün törəmə (Xəta) əlavə edin.
- Gecə versiyasını düzəldin.
- iroha_error qutusu əlavə edin.
- Versiyalaşdırılmış mesajlar.
- Konteyner versiyaları üçün primitivləri təqdim edir.
- Benchmarkları düzəldin.
- Səhifə əlavə edin.
- Varint kodlaşdırma deşifrəsini əlavə edin.
- Sorğunun vaxt damgasını u128-ə dəyişin.
- Boru kəməri hadisələri üçün RejectionReason nömrə əlavə edin.
- Genesis fayllarından köhnəlmiş xətləri silir. Təyinat əvvəlki öhdəliklərdə ISI reyestrindən çıxarılıb.
- İSİ-lərin qeydiyyatını və qeydiyyatdan çıxarılmasını sadələşdirir.
- 4 peer şəbəkəsində göndərilməmiş öhdəliyin vaxt aşımını düzəldin.
- Dəyişiklik görünüşündə topologiya qarışdırılır.- FromVariant əldə etmək makrosu üçün digər konteynerlər əlavə edin.
- Müştəri cli üçün MST dəstəyi əlavə edin.
- FromVariant makro və təmizləmə kod bazasını əlavə edin.
- Kod sahiblərinə i1i1 əlavə edin.
- Qeybət əməliyyatları.
- Təlimatlar və ifadələr üçün uzunluq əlavə edin.
- Vaxtı bloklamaq və vaxt parametrlərini yerinə yetirmək üçün sənədlər əlavə edin.
- Verify and Accept xüsusiyyətləri TryFrom ilə əvəz olundu.
- Yalnız minimum sayda həmyaşıdları gözləməyi tətbiq edin.
- Api-ni iroha2-java ilə sınamaq üçün github əməliyyatı əlavə edin.
- Docker-compose-single.yml üçün genesis əlavə edin.
- Hesab üçün defolt imza yoxlama şərti.
- Çox sayda imza sahibi olan hesab üçün test əlavə edin.
- MST üçün müştəri API dəstəyi əlavə edin.
- Docker-də qurun.
- Docker-in tərtibinə genesis əlavə edin.
- Şərti MST-ni təqdim edin.
- wait_for_active_peers impl əlavə edin.
- iroha_http_server-də isahc müştəri üçün test əlavə edin.
- Müştəri API spesifikasiyası.
- İfadələrdə sorğunun icrası.
- İfadələri və İSİ-ləri birləşdirir.
- ISI üçün ifadələr.
- Hesab konfiqurasiya göstəricilərini düzəldin.
- Müştəri üçün hesab konfiqurasiyası əlavə edin.
- `submit_blocking`-i düzəldin.
- Boru kəməri hadisələri göndərilir.
- Iroha müştəri veb yuvası bağlantısı.
- Boru kəməri və məlumat hadisələri üçün hadisələrin ayrılması.
- İcazələr üçün inteqrasiya testi.
- Yanma və nanə üçün icazə çekləri əlavə edin.
- ISI icazəsini qeydiyyatdan çıxarın.
- Dünya struktur PR üçün etalonları düzəldin.
- Dünya quruluşunu təqdim edin.
- Genesis blokunun yükləmə komponentini həyata keçirin.
- Genesis hesabını təqdim edin.
- İcazələrin təsdiqləyici qurucusunu təqdim edin.
- Github Actions ilə Iroha2 PR-lərinə etiketlər əlavə edin.
- İcazələr Çərçivəsini təqdim edin.
- Queue tx tx nömrə limiti və Iroha başlanğıc düzəlişləri.
- Hash-i struktura sarın.
- Günlük səviyyəsini yaxşılaşdırın:
  - Konsensusa məlumat səviyyəsi qeydləri əlavə edin.
  - Şəbəkə rabitəsi qeydlərini iz səviyyəsi kimi qeyd edin.
  - WSV-dən blok vektorunu çıxarın, çünki bu, dublikatdır və bütün blokçeyni qeydlərdə göstərdi.
  - Məlumat jurnalının səviyyəsini standart olaraq təyin edin.
- Doğrulama üçün dəyişkən WSV istinadlarını çıxarın.
- Heim versiyası artımı.
- Konfiqurasiyaya standart etibarlı həmyaşıdları əlavə edin.
- Müştəri API miqrasiyası http.
- CLI-ə köçürmə isi əlavə edin.
- Iroha Peer ilə əlaqəli Təlimatların konfiqurasiyası.
- Çatışmayan ISI icra üsullarının və testlərinin tətbiqi.
- Url sorğu parametrlərinin təhlili
- `HttpResponse::ok()`, `HttpResponse::upgrade_required(..)` əlavə edin
- Köhnə Təlimat və Sorğu modellərinin Iroha DSL yanaşması ilə dəyişdirilməsi.
- BLS imza dəstəyi əlavə edin.
- Http server qutusunu təqdim edin.
- Symlink ilə yamaqlanmış libssl.so.1.0.0.
- Əməliyyat üçün hesab imzasını yoxlayır.
- Refaktor əməliyyatı mərhələləri.
- İlkin domenlərin təkmilləşdirilməsi.
- DSL prototipini tətbiq edin.
- Torii Benchmarks-ı təkmilləşdirin: meyarlarda girişi söndürün, müvəffəqiyyət nisbəti təsdiqini əlavə edin.
- Sınaq əhatə dairəsini yaxşılaşdırın: `tarpaulin`-i `grcov` ilə əvəz edin, `codecov.io`-də test əhatəsi hesabatını dərc edin.
- RTD mövzusunu düzəldin.
- İroha sublayihələri üçün çatdırılma artefaktları.
- `SignedQueryRequest` təqdim edin.
- İmza yoxlaması ilə səhvi düzəldin.
- Geri qaytarma əməliyyatlarına dəstək.
- Yaradılmış açar cütünü json kimi çap edin.
- `Secp256k1` açar cütünü dəstəkləyin.
- Müxtəlif kripto alqoritmləri üçün ilkin dəstək.
- DEX Xüsusiyyətləri.
- Sərt kodlanmış konfiqurasiya yolunu cli param ilə əvəz edin.
- Dəzgah ustası iş axınının düzəldilməsi.
- Docker hadisə bağlantısı testi.
- Iroha Monitor Bələdçisi və CLI.
- Hadisələr cli təkmilləşdirilməsi.
- Hadisələr filtri.
- Hadisə əlaqələri.
- Əsas iş prosesində düzəldin.
- iroha2 üçün Rtd.
- Blok əməliyyatları üçün Merkle ağacı kök hash.
- Docker mərkəzinə nəşr.
- Maintenance Connect üçün CLI funksionallığı.
- Maintenance Connect üçün CLI funksionallığı.
- Makro daxil etmək üçün Eprintln.- Təkmilləşdirmələri qeyd edin.
- IR-802 Blok statuslarına abunə.
- Əməliyyatların və blokların göndərilməsi hadisələri.
- Sumeragi mesaj idarəsini mesaja köçürür.
- Ümumi Bağlantı Mexanizmi.
- No-std müştəri üçün Iroha domen obyektlərini çıxarın.
- TTL əməliyyatları.
- Blok konfiqurasiyası üzrə maksimum əməliyyatlar.
- Etibarsız blokların hashlərini saxlayın.
- Blokları qruplarla sinxronlaşdırın.
- Qoşulma funksiyasının konfiqurasiyası.
- Iroha funksionallığına qoşulun.
- Doğrulama düzəlişlərini bloklayın.
- Blok sinxronizasiyası: diaqramlar.
- Iroha funksionallığına qoşulun.
- Körpü: müştəriləri çıxarın.
- Blok sinxronizasiyası.
- AddPeer ISI.
- Təlimatların adının dəyişdirilməsi üçün əmrlər.
- Sadə ölçülərin son nöqtəsi.
- Körpü: qeydiyyatdan keçmiş körpüləri və xarici aktivləri əldə edin.
- Docker boru kəmərində test tərtib edir.
- Kifayət qədər səs yoxdur Sumeragi testi.
- Blok zəncirləmə.
- Körpü: xarici köçürmələrin əl ilə idarə edilməsi.
- Sadə Baxım son nöqtəsi.
- Serde-json-a miqrasiya.
- ISI-ni ləğv edin.
- Körpü müştəriləri, AddSignatory ISI və CanAddSignatory icazəsi əlavə edin.
- Sumeragi: b dəstindəki həmyaşıdlar TODO düzəlişləri ilə əlaqədardır.
- Sumeragi-də imzalamadan əvvəl bloku təsdiqləyir.
- Xarici aktivləri birləşdirin.
- Sumeragi mesajlarında imzanın yoxlanılması.
- İkili aktiv mağazası.
- PublicKey ləqəbini növlə əvəz edin.
- Nəşr üçün qutuları hazırlayın.
- NetworkTopology daxilində minimum səs məntiqi.
- TransactionReceipt təsdiqləmə refaktorinqi.
- OnWorldStateViewChange tetikleyici dəyişikliyi: Təlimat əvəzinə IrohaQuery.
- NetworkTopology-də inisializasiyadan ayrı tikinti.
- Iroha hadisələri ilə bağlı Iroha Xüsusi Təlimatlar əlavə edin.
- Blok yaratma müddətinin uzadılması.
- Lüğət və Iroha Modul sənədlərini necə əlavə etmək olar.
- Sərt kodlu körpü modelini mənşəli Iroha modeli ilə əvəz edin.
- NetworkTopology strukturunu təqdim edin.
- Təlimatlardan çevrilmə ilə İcazə obyekti əlavə edin.
- Sumeragi Mesaj modulunda mesajlar.
- Kür üçün Genesis Block funksionallığı.
- Iroha qutuları üçün README faylları əlavə edin.
- Bridge və RegisterBridge ISI.
- Iroha ilə ilkin iş dinləyiciləri dəyişir.
- OOB ISI-yə icazə çeklərinin vurulması.
- Docker çoxlu həmyaşıdları düzəldin.
- Peer to peer docker nümunəsi.
- Tranzaksiya Qəbzinin idarə edilməsi.
- Iroha İcazələri.
- Dex üçün modul və körpülər üçün qutular.
- Bir neçə həmyaşıdları ilə aktivlərin yaradılması ilə inteqrasiya testini düzəldin.
- Aktiv modelinin EC-S-də yenidən tətbiqi.
- Taymout rəftarına əməl edin.
- Blok başlığı.
- Domen obyektləri üçün ISI ilə əlaqəli üsullar.
- Kür rejiminin sayılması və Güvənli Həmyaşıdların konfiqurasiyası.
- Sənədləşdirmə qaydası.
- CommittedBlock əlavə edin.
- `sumeragi`-dən ayrılan kura.
- Blok yaratmazdan əvvəl əməliyyatların boş olmadığını yoxlayın.
- Iroha Xüsusi Təlimatlarını yenidən həyata keçirin.
- Əməliyyatlar və blok keçidləri üçün meyarlar.
- Əməliyyatların həyat dövrü və yenidən işlənmiş dövlətlər.
- Həyat dövrünü və vəziyyətləri bloklayır.
- Doğrulama səhvini düzəldin, block_build_time_ms konfiqurasiya parametri ilə sinxronlaşdırılmış `sumeragi` dövrə dövrü.
- `sumeragi` modulu daxilində Sumeragi alqoritminin inkapsulyasiyası.
- Kanallar vasitəsilə həyata keçirilən Iroha şəbəkə qutusu üçün istehza modulu.
- Async-std API-yə köçürmə.
- Şəbəkə istehza xüsusiyyəti.
- Asinxron əlaqəli kodu təmizləyin.
- Tranzaksiyaların işlənməsi döngəsində performansın optimallaşdırılması.
- Açar cütlərinin yaradılması Iroha başlanğıcından çıxarılıb.
- Docker qablaşdırması Iroha icra edilə bilər.- Sumeragi əsas ssenarisini təqdim edin.
- Iroha CLI müştəri.
- Dəzgah qrupunun icrasından sonra irohanın düşməsi.
- `sumeragi`-i inteqrasiya edin.
- `sort_peers` tətbiqini əvvəlki blok hash ilə toxumlanmış rand qarışdırmaqla dəyişdirin.
- Peer modulunda Mesaj sarğısını çıxarın.
- `torii::uri` və `iroha_network` daxilində şəbəkə ilə bağlı məlumatları əhatə edin.
- Sərt kodla işləmək əvəzinə həyata keçirilən Peer təlimatını əlavə edin.
- Etibarlı həmyaşıdlar siyahısı vasitəsilə həmyaşıdlarla ünsiyyət.
- Torii daxilində idarə olunan şəbəkə sorğularının inkapsulyasiyası.
- Kriptomodul daxilində kripto məntiqinin inkapsulyasiyası.
- Vaxt damğası və faydalı yük kimi əvvəlki blok hash ilə blok işarəsi.
- Kripto funksiyaları modulun üstünə yerləşdirilir və İmzaya daxil edilmiş ursa imzalayıcı ilə işləyir.
- Sumeragi ilkin.
- Saxlamadan əvvəl dünya dövlət görünüşü klonunda əməliyyat təlimatlarının təsdiqlənməsi.
- Tranzaksiya qəbuluna dair imzaları yoxlayın.
- Seriyadan çıxarma sorğusunda səhvi düzəldin.
- Iroha imzasının həyata keçirilməsi.
- Blockchain obyekti kod bazasını təmizləmək üçün silindi.
- Transactions API-də dəyişikliklər: daha yaxşı yaratmaq və sorğularla işləmək.
- Əməliyyatın boş vektoru ilə bloklar yarada biləcək səhvi düzəldin
- Forward gözlənilən əməliyyatlar.
 - U128 Norito kodlu TCP paketində çatışmayan bayt olan səhvi düzəldin.
- Metodların izlənməsi üçün atribut makroları.
- P2p modulu.
- Torii və müştəridə iroha_network-dən istifadə.
- Yeni ISI məlumatı əlavə edin.
- Şəbəkə vəziyyəti üçün xüsusi tipli ləqəb.
- Box<dyn Error> String ilə əvəz olundu.
- Şəbəkə vəziyyətinə qulaq asın.
- Əməliyyatlar üçün ilkin doğrulama məntiqi.
- Iroha_şəbəkə qutusu.
- Io, IntoContract və IntoQuery xüsusiyyətləri üçün makro əldə edin.
- Iroha-müştəri üçün sorğuların icrası.
- Əmrlərin ISI müqavilələrinə çevrilməsi.
- Şərti multisig üçün təklif olunan dizaynı əlavə edin.
- Karqo iş sahələrinə köçmə.
- Modulların miqrasiyası.
- Ətraf mühit dəyişənləri vasitəsilə xarici konfiqurasiya.
- Torii üçün sorğuların idarə edilməsini alın və qoyun.
- Github ci korreksiyası.
- Kargo-make testdən sonra blokları təmizləyir.
- Kataloqu bloklarla təmizləmək funksiyası ilə `test_helper_fns` modulunu təqdim edin.
- Merkle ağacı vasitəsilə doğrulama həyata keçirin.
- İstifadə edilməmiş törəməni çıxarın.
- Async/gözləyin təbliğ edin və gözlənilməyən `wsv::put`-ni düzəldin.
- `futures` qutusundan birləşmədən istifadə edin.
- Paralel mağaza icrasını həyata keçirin: diskə yazmaq və WSV-ni yeniləmək paralel olaraq baş verir.
- Serializasiya üçün mülkiyyət əvəzinə istinadlardan istifadə edin.
- Fayllardan kodun çıxarılması.
- Ursa::blake2 istifadə edin.
- Töhfə bələdçisində mod.rs haqqında qayda.
- Hash 32 bayt.
- Blake2 hash.
- Disk bloklamaq üçün istinadları qəbul edir.
- Əmrlər modulunun və İlkin Merkle Ağacının refaktorinqi.
- Refactored modul strukturu.
- Düzgün formatlaşdırma.
- Read_all-a sənəd şərhləri əlavə edin.
- `read_all` tətbiq edin, yaddaş testlərini yenidən təşkil edin və asinxron funksiyaları olan testləri asinxron testlərə çevirin.
- Lazımsız dəyişkən ələ keçirməni çıxarın.
- Problemi nəzərdən keçirin, klipi düzəldin.
- Tire çıxarın.
- Format yoxlanışı əlavə edin.
- Token əlavə edin.
- Github əməliyyatları üçün rust.yml yaradın.
- Disk saxlama prototipini təqdim edin.
- Aktiv testi və funksionallığı transferi.
- Strukturlara standart başlatıcı əlavə edin.
- MSTCache strukturunun adını dəyişdirin.
- Unudulmuş borc əlavə edin.
- İroha2 kodunun ilkin konturları.
- İlkin Kura API.
- Bəzi əsas faylları əlavə edin və həmçinin iroha v2 üçün vizyonu əks etdirən sənədin ilk layihəsini buraxın.
- Əsas iroha v2 filialı.

## [1.5.0] - 2022-04-08

### CI/CD dəyişiklikləri
- Jenkinsfile və JenkinsCI-ni silin.

### Əlavə edilib- Burrow üçün RocksDB saxlama tətbiqini əlavə edin.
- Bloom-filtr ilə trafikin optimallaşdırılmasını təqdim edin
- `batches_cache`-də `OS` modulunda yerləşmək üçün `MST` modul şəbəkəsini yeniləyin.
- Trafikin optimallaşdırılmasını təklif edin.

### Sənədləşmə

- Quraşdırmanı düzəldin. DB fərqləri, miqrasiya təcrübəsi, sağlamlıq yoxlanışının son nöqtəsi, iroha-swarm aləti haqqında məlumat əlavə edin.

### Digər

- Sənədin qurulması üçün tələb düzəlişi.
- Qalan kritik təqib elementini vurğulamaq üçün buraxılış sənədlərini kəsin.
- "Docker təsvirinin mövcud olub olmadığını yoxlayın" düzəldin / bütün skip_testing qurun.
- /bütün skip_testing qurun.
- /keçirmə_testini qurun; Və daha çox sənədlər.
- `.github/_README.md` əlavə edin.
- `.packer`-i çıxarın.
- Test parametrindəki dəyişiklikləri silin.
- Test mərhələsini keçmək üçün yeni parametrdən istifadə edin.
- İş prosesinə əlavə edin.
- Repozitor göndərişini silin.
- Repozitor göndərişini əlavə edin.
- Testçilər üçün parametr əlavə edin.
- `proposal_delay` fasiləsini çıxarın.

## [1.4.0] - 31-01-2022

### Əlavə edilib

- Sinxronizasiya node vəziyyəti əlavə edin
- RocksDB üçün ölçülər əlavə edir
- Http və ölçülər vasitəsilə sağlamlıq yoxlaması interfeysləri əlavə edin.

### Düzəlişlər

- Iroha v1.4-rc.2-də sütun ailələrini düzəldin
- Iroha v1.4-rc.1-də 10 bitlik çiçəkləmə filtri əlavə edin

### Sənədləşmə

- Quraşdırma məlumatlarının siyahısına zip və pkg-config əlavə edin.
- Readmeni yeniləyin: status yaratmaq, bələdçi qurmaq və s. üçün pozulmuş keçidləri düzəldin.
- Konfiqurasiya və Docker Metriklərini düzəldin.

### Digər

- GHA docker etiketini yeniləyin.
- g++11 ilə tərtib edərkən Iroha 1 kompilyasiya xətalarını düzəldin.
- `max_rounds_delay`-i `proposal_creation_timeout` ilə əvəz edin.
- Köhnə DB əlaqə parametrlərini silmək üçün nümunə konfiqurasiya faylını yeniləyin.