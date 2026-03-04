---
lang: az
direction: ltr
source: docs/source/crypto/sm_program.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 08e2e1e4a54390d9142d6788aad2385e93282a33423b9fc7f3418e3633f3f86a
source_last_modified: "2026-01-23T23:46:10.134857+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Hyperledger Iroha v2 üçün SM2/SM3/SM4 aktivləşdirmə memarlığı qısası.

# SM Proqramı Memarlığı Qısacası

## Məqsəd
Çin milli kriptoqrafiyasının (SM2/SM3/SM4) Iroha v2 yığını üzərindən tətbiqi üçün texniki planı, təchizat zəncirinin vəziyyətini və risk sərhədlərini müəyyənləşdirin, eyni zamanda deterministik icra və audit qabiliyyətini qoruyun.

## Əhatə dairəsi
- **Kritik konsensus yolları:** `iroha_crypto`, `iroha`, `irohad`, IVM host, Kotodama intrinsics.
- **Müştəri SDK-ları və alətlər:** Rust CLI, Kagami, Python/JS/Swift SDK, genesis utilitləri.
- **Konfiqurasiya və seriallaşdırma:** `iroha_config` düymələri, Norito data modeli teqləri, manifest idarəsi, multicodec yeniləmələri.
- **Sınaq və uyğunluq:** Vahid/əmlak/birlikdə işləmə dəstləri, Wycheproof qoşqular, performans profili, ixrac/tənzimləyici təlimat. *(Status: RustCrypto ilə dəstəklənən SM yığını birləşdirildi; isteğe bağlı `sm_proptest` fuzz dəsti və genişləndirilmiş CI üçün OpenSSL paritet qoşqu əlçatandır.)*

Əhatə dairəsi xaricində: PQ alqoritmləri, konsensus yollarında deterministik olmayan host sürətləndirilməsi; wasm/`no_std` qurucuları təqaüdə çıxdı.

## Alqoritm Daxiletmələri və Təqdim olunanlar
| Artefakt | Sahibi | Vaxtı | Qeydlər |
|----------|-------|-----|-------|
| SM alqoritm xüsusiyyətləri dizaynı (`SM-P0`) | Kripto WG | 2025-02 | Xüsusiyyət keçidi, asılılıq auditi, risk reyestri. |
| Core Rust inteqrasiyası (`SM-P1`) | Kripto WG / Məlumat Modeli | 2025-03 | RustCrypto əsaslı doğrulama/hash/AEAD köməkçiləri, Norito uzantıları, qurğular. |
| İmzalanma + VM sistem zəngləri (`SM-P2`) | IVM Əsas / SDK Proqramı | 2025-04 | Deterministik imzalama sarğıları, sistemlər, Kotodama əhatə dairəsi. |
| İsteğe bağlı provayder və əməliyyatların aktivləşdirilməsi (`SM-P3`) | Platforma Əməliyyatları / Performans WG | 2025-06 | OpenSSL/Tongsuo backend, ARM intrinsics, telemetriya, sənədlər. |

## Seçilmiş Kitabxanalar
- **İlkin:** RustCrypto qutuları (`sm2`, `sm3`, `sm4`) `rfc6979` funksiyası aktivdir və SM3 deterministik qeyri-müəyyənliklərə bağlıdır.
- **Könüllü FFI:** Sertifikatlaşdırılmış yığınlar və ya aparat mühərrikləri tələb edən yerləşdirmələr üçün OpenSSL 3.x provayder API və ya Tongsuo; xüsusiyyət qapalı və konsensus ikili sistemlərində defolt olaraq qeyri-aktivdir.### Əsas Kitabxana İnteqrasiya Vəziyyəti
- `iroha_crypto::sm` vahid `sm` funksiyası altında SM3 hashing, SM2 doğrulama və SM4 GCM/CCM köməkçilərini ifşa edir, SDK-lar üçün deterministik RFC6979 imzalama yolları ilə `Sm2PrivateKey`.【crates/iroha_crypto/src/sm.rs:1049】【crates/iroha_crypto/src/sm.rs:1128】【crates/iroha_crypto/src/sm.rs:1236】
- Norito/Norito-JSON teqləri və multikodek köməkçiləri SM2 açıq açarlarını/imzalarını və SM3/SM4 faydalı yüklərini əhatə edir, beləliklə təlimatlar müəyyən bir şəkildə seriyalaşdırılır hostlar.【crates/iroha_data_model/src/isi/registry.rs:407】【crates/iroha_data_model/tests/sm_norito_roundtrip.rs:12】
- Məlum cavab paketləri RustCrypto inteqrasiyasını (`sm3_sm4_vectors.rs`, `sm2_negative_vectors.rs`) təsdiqləyir və qovşaqlar ilə imzalamağa davam edərkən doğrulamanı deterministik saxlayaraq, CI-nin `sm` funksiya işlərinin bir hissəsi kimi işləyir. Ed25519.【crates/iroha_crypto/tests/sm3_sm4_vectors.rs:15】【crates/iroha_crypto/tests/sm2_negative_vectors.rs:1】
- Könüllü `sm` funksiyanın qurulmasının yoxlanılması: `cargo check -p iroha_crypto --features sm --locked` (soyuq 7,9s / isti 0,23s) və `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` (1,0s) hər ikisi uğur qazanır; funksiyanın işə salınması 11 qutu əlavə edir (`base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, Hyperledger, Hyperledger, Hyperledger, `ghash` `sm2`, `sm3`, `sm4`, `sm4-gcm`). Tapıntılar `docs/source/crypto/sm_rustcrypto_spike.md`-də qeydə alınıb.【docs/source/crypto/sm_rustcrypto_spike.md:1】
- BouncyCastle/GmSSL neqativ yoxlama qurğuları `crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json` altında yaşayır, kanonik uğursuzluq hallarının (r=0, s=0, fərqləndirici şəxsiyyət uyğunsuzluğu, dəyişdirilmiş açıq açar) geniş şəkildə yerləşdirilənlərlə uyğun qalmasını təmin edir. provayderlər.【crates/iroha_crypto/tests/sm2_negative_vectors.rs:1】【crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json:1】
- `sm-ffi-openssl` indi təchiz edilmiş OpenSSL 3.x alətlər silsiləsini (`openssl` sandıq `vendored` xüsusiyyəti) tərtib edir, buna görə də sistem LibreSSL/OpenSSL olmadıqda belə, ilkin baxış qurur və sınaqlar həmişə müasir SM qabiliyyətli provayderi hədəf alır. alqoritmlər.【crates/iroha_crypto/Cargo.toml:59】
- `sm_accel` indi iş vaxtı AArch64 NEON-u aşkarlayır və `crypto.sm_intrinsics` konfiqurasiya düyməsinə riayət etməklə x86_64/RISC-V göndərişi vasitəsilə SM3/SM4 qarmaqlarını keçir. (`auto`/`force-enable`/`force-disable`). Vektor arxa ucları olmadıqda dispetçer hələ də skalyar RustCrypto yolu ilə hərəkət edir, beləliklə dəzgahlar və siyasət keçidləri hostlar arasında ardıcıl şəkildə davranır.【crates/iroha_crypto/src/sm.rs:702】【crates/iroha_crypto/src/sm.rs:733】

### Norito Sxem və Məlumat Modeli Səthləri| Norito növü / istehlakçı | Nümayəndəlik | Məhdudiyyətlər və qeydlər |
|---------------------------------|----------------|---------------------|
| `Sm3Digest` (`iroha_crypto::Sm3Digest`) | Çılpaq: 32 bayt blob · JSON: böyük hex sətir (`"4F4D..."`) | Kanonik Norito dəzgah sarğısı `[u8; 32]`. JSON/Çılpaq deşifrləmə uzunluqları rədd edir ≠32. Gediş-gəlişlər `sm_norito_roundtrip::sm3_digest_norito_roundtrip` tərəfindən əhatə olunur. |
| `Sm2PublicKey` / `Sm2Signature` | Multicodec prefiksli bloblar (`0x1306` müvəqqəti) | Açıq açarlar sıxılmamış SEC1 nöqtələrini kodlayır; imzalar DER təhlil qoruyucuları ilə `(r∥s)` (hər biri 32 bayt). |
| `Sm4Key` | Çılpaq: 16 bayt blob | Kotodama/CLI təsirinə məruz qalan sarğı sıfırlanır. JSON serializasiyası qəsdən buraxılıb; operatorlar açarları bloblar (kontraktlar) və ya CLI hex (`--key-hex`) vasitəsilə ötürməlidirlər. |
| `sm4_gcm_seal/open` operandları | 4 blob dəsti: `(key, nonce, aad, payload)` | Açar = 16 bayt; nonce = 12 bayt; teq uzunluğu 16 baytda müəyyən edilmişdir. Qaytarır `(ciphertext, tag)`; Kotodama/CLI həm hex, həm də base64 köməkçiləri buraxır.【crates/ivm/tests/sm_syscalls.rs:728】 |
| `sm4_ccm_seal/open` operandları | `r14`-də 4 blob (açar, qeyri, aad, faydalı yük) + etiket uzunluğu | 7-13 bayt deyil; teq uzunluğu ∈ {4,6,8,10,12,14,16}. `sm` xüsusiyyəti `sm-ccm` bayrağının arxasında CCM-ni ifşa edir. |
| Kotodama intrinsics (`sm::hash`, `sm::seal_gcm`, `sm::open_gcm`, …) | Yuxarıdakı SCALLs üçün xəritə | Daxiletmənin doğrulanması host qaydalarını əks etdirir; qüsurlu ölçülər `ExecutionError::Type` artırır. |

İstifadə sürətli istinad:
- **Müqavilələrdə/testlərdə SM3 hashing:** `Sm3Digest::hash(b"...")` (Rust) və ya Kotodama `sm::hash(input_blob)`. JSON 64 hex simvol gözləyir.
- **CLI vasitəsilə SM4 AEAD:** `iroha tools crypto sm4 gcm-seal --key-hex <32 hex> --nonce-hex <24 hex> --plaintext-hex …` hex/base64 şifrəli mətn+teq cütlərini verir. `gcm-open` uyğunluğu ilə şifrəni açın.
- **Multicodec sətirləri:** SM2 açıq açarları/imzaları `PublicKey::from_str`/`Signature::from_bytes` tərəfindən qəbul edilən çoxbazalı sətirdən/imzaya təhlil edir, Norito manifestləri və hesab identifikatorlarını SM imzalayanları daşımağa imkan verir.

Məlumat modeli istehlakçıları SM4 açarları və teqləri keçici bloblar kimi qəbul etməlidirlər; zəncirdə heç vaxt xam açarları saxlamayın. Audit tələb olunduqda müqavilələr yalnız şifrəli mətn/teq çıxışlarını və ya əldə edilmiş həzmləri (məsələn, açarın SM3-ü) saxlamalıdır.

### Təchizat Zənciri və Lisenziyalaşdırma
| Komponent | Lisenziya | Azaldılması |
|-----------|---------|-----------|
| `sm2`, `sm3`, `sm4` | Apache-2.0 / MIT | Yuxarı öhdəliyi izləyin, tədarükçünün kilidləmə buraxılışları tələb olunarsa, validator GA imzalamazdan əvvəl üçüncü tərəf auditini planlaşdırın. |
| `rfc6979` | Apache-2.0 / MIT | Artıq digər alqoritmlərdə istifadə olunur; SM3 digest ilə deterministik `k` bağlanmasını təsdiq edin. |
| Könüllü OpenSSL/Tongsuo | Apache-2.0 / BSD-stil | `sm-ffi-openssl` xüsusiyyətinin arxasında saxlayın, operatorun açıq şəkildə qoşulmasını və qablaşdırma yoxlama siyahısını tələb edin. |### Xüsusiyyət Bayraqları və Sahiblik
| Səthi | Defolt | Baxıcı | Qeydlər |
|---------|---------|------------|-------|
| `iroha_crypto/sm-core`, `sm-ccm`, `sm` | Off | Kripto WG | RustCrypto SM primitivlərini aktivləşdirir; `sm` təsdiqlənmiş şifrələmə tələb edən müştərilər üçün CCM köməkçilərini birləşdirir. |
| `ivm/sm` | Off | IVM Əsas Komanda | SM sistem zənglərini (`sm3_hash`, `sm2_verify`, `sm4_gcm_*`, `sm4_ccm_*`) qurur. Host qapısı `crypto.allowed_signing`-dən (`sm2`-in olması) irəli gəlir. |
| `iroha_crypto/sm_proptest` | Off | QA / Kripto WG | Səhv formalaşdırılmış imzaları/teqləri əhatə edən mülkiyyət testi kəməri. Yalnız genişləndirilmiş CI-də aktivləşdirilib. |
| `crypto.allowed_signing` + `default_hash` | `["ed25519"]`, `blake2b-256` | Config WG / Operators WG | `sm2` plus `sm3-256` hash-in mövcudluğu SM sistem zənglərini/imzalarını təmin edir; `sm2`-in silinməsi yalnız doğrulama rejiminə qayıdır. |
| Könüllü `sm-ffi-openssl` (öncədən baxış) | Off | Platforma Əməliyyatları | OpenSSL/Tongsuo provayder inteqrasiyası üçün yer tutucu xüsusiyyəti; sertifikatlaşdırma və qablaşdırma SOP-ları yerə düşənə qədər əlil olaraq qalır. |

Şəbəkə siyasəti indi `network.require_sm_handshake_match` və
`network.require_sm_openssl_preview_match` (hər ikisi standart olaraq `true`). İstənilən bayrağın təmizlənməsi icazə verir
Yalnız Ed25519 müşahidəçilərinin SM-i aktivləşdirən validatorlara qoşulduğu qarışıq yerləşdirmələr; uyğunsuzluqlardır
`WARN`-də daxil edilmişdir, lakin konsensus qovşaqları təsadüfi hadisələrin qarşısını almaq üçün defoltları aktiv saxlamalıdır.
SM-dən xəbərdar olan və SM-dən əlil olan həmyaşıdlar arasında fərq.
CLI bu keçidləri `iroha_cli app sorafs handshake yeniləməsi vasitəsilə üzə çıxarır
--allow-sm-handshake-usmatch` and `--allow-sm-openssl-preview-usmatch`, or the matching `--tələb edir-*`
ciddi icranı bərpa etmək üçün bayraqlar.#### OpenSSL/Tongsuo önizləməsi (`sm-ffi-openssl`)
- **Əhatə dairəsi.** OpenSSL işləmə vaxtının mövcudluğunu təsdiqləyən və daxil olmaqda qalarkən OpenSSL tərəfindən dəstəklənən SM3 heşinqini, SM2 yoxlamasını və SM4-GCM şifrələməsini/şifrəsini açan yalnız önizləmə təminatçısını (`OpenSslProvider`) qurur. Konsensus ikili faylları RustCrypto yolundan istifadə etməyə davam etməlidir; FFI backend, kənar yoxlama/imzalama pilotları üçün ciddi şəkildə seçilir.
- **İlkin şərtlər yaradın.** `cargo build -p iroha_crypto --features "sm sm-ffi-openssl"` ilə tərtib edin və OpenSSL/Tongsuo 3.0+ (SM2/SM3/SM4 dəstəyi ilə `libcrypto`) ilə alətlər silsiləsi bağlantılarını təmin edin. Statik əlaqə tövsiyə edilmir; operator tərəfindən idarə olunan dinamik kitabxanalara üstünlük verin.
- **Tərtibatçının tüstü testi.** `cargo check -p iroha_crypto --features "sm sm-ffi-openssl"` və ardınca `cargo test -p iroha_crypto --features "sm sm-ffi-openssl" --test sm_openssl_smoke -- --nocapture`-i yerinə yetirmək üçün `scripts/sm_openssl_smoke.sh`-i işə salın; OpenSSL ≥3 inkişaf başlıqları mövcud olmadıqda (və ya `pkg-config` çatışmayanda) köməkçi avtomatik olaraq atlanır və tüstü çıxışını səthə çıxarır ki, tərtibatçılar SM2 yoxlamasının Rust tətbiqinə keçib-keçmədiyini görə bilsinlər.
- **Rust iskele.** `openssl_sm` modulu indi SM3 hashing, SM2 yoxlaması (ZA prehash + SM2 ECDSA) və SM4 GCM şifrələmə/şifrəni OpenSSL vasitəsilə ilkin baxış keçidlərini və etibarsız açar/nonce/teq uzunluqlarını əhatə edən strukturlaşdırılmış xətalarla istiqamətləndirir; SM4 CCM əlavə FFI şimləri yerə enənə qədər yalnız Pasdan qorunur.
- **Ötürmə davranışı.** OpenSSL ≥3.0 başlıqları və ya kitabxanaları olmadıqda, tüstü testi ötürmə bannerini çap edir (`-- --nocapture` vasitəsilə), lakin yenə də uğurla çıxır ki, CI ətraf mühit boşluqlarını həqiqi reqressiyalardan ayıra bilsin.
- **Runtime qoruyucuları.** OpenSSL önizləməsi defolt olaraq qeyri-aktivdir; FFI yolundan istifadə etməyə cəhd etməzdən əvvəl onu konfiqurasiya (`crypto.enable_sm_openssl_preview` / `OpenSslProvider::set_preview_enabled(true)`) vasitəsilə aktivləşdirin. Provayder məzun olana qədər istehsal klasterlərini yalnız doğrulama rejimində saxlayın (`sm2`-i `allowed_signing`-dən buraxın), deterministik RustCrypto ehtiyatına etibar edin və imza pilotlarını təcrid olunmuş mühitlərlə məhdudlaşdırın.
- **Qablaşdırma yoxlama siyahısı.** Yerləşdirmə manifestlərində provayder versiyasını, quraşdırma yolunu və bütövlük hashlərini sənədləşdirin. Operatorlar təsdiqlənmiş OpenSSL/Tongsuo quruluşunu quraşdıran quraşdırma skriptlərini təqdim etməli, onu ƏS-in etibar mağazasında qeydiyyatdan keçirməli (lazım olduqda) və yeniləmələri texniki xidmət pəncərələrinin arxasına qoymalıdır.
- **Növbəti addımlar.** Gələcək mərhələlər deterministik SM4 CCM FFI bağlamaları, CI tüstü işləri (bax: `ci/check_sm_openssl_stub.sh`) və telemetriya əlavə edir. `roadmap.md`-də SM-P3.1.x altında tərəqqi izləyin.

#### Kod Sahibliyi Snapshot
- **Kripto WG:** `iroha_crypto`, SM qurğuları, uyğunluq sənədləri.
- **IVM Əsas:** sistem zəngi tətbiqləri, Kotodama intrinsics, host qapısı.
- **Config WG:** קופיגורצית `crypto.allowed_signing`/`default_hash`, ולידציית מניפסט, חיהבוט.
- **SDK Proqramı:** CLI/Kagami/SDK-lar üzrə SM-dən xəbərdar alətlər, paylaşılan qurğular.
- **Platforma Əməliyyatları və Performans WG:** sürətləndirici qarmaqlar, telemetriya, operatorun aktivləşdirilməsi.

## Konfiqurasiya Miqrasiya KitabıYalnız Ed25519 şəbəkələrindən SM-i aktivləşdirən yerləşdirmələrə keçən operatorlar
mərhələli prosesi izləyin
[`sm_config_migration.md`](sm_config_migration.md). Bələdçi quruluşu əhatə edir
doğrulama, `iroha_config` qatlama (`defaults` → `user` → `actual`), genezis
`kagami` ləğvetmələri (məsələn, `kagami genesis generate --allowed-signing sm2 --default-hash sm3-256`), uçuşdan əvvəl doğrulama və geri qaytarma vasitəsilə regenerasiya
konfiqurasiya snapshots və manifestləri arasında ardıcıl qalmaq belə planlaşdırma
donanma.

## Deterministik Siyasət
- SDK-larda bütün SM2 imzalama yolları və isteğe bağlı host imzalanması üçün RFC6979-dan əldə edilmiş qeyri-məhdudiyyətləri tətbiq edin; yoxlayıcılar yalnız kanonik r∥s kodlaşdırmalarını qəbul edir.
- İdarəetmə təyyarəsi əlaqəsi (axın) Ed25519 olaraq qalır; İdarəetmə genişlənməni təsdiqləmədiyi halda, SM2 data-plan imzaları ilə məhdudlaşır.
- İntrinsics (ARM SM3/SM4) iş vaxtı xüsusiyyətlərinin aşkarlanması və proqram təminatının geri qaytarılması ilə deterministik yoxlama/hesh əməliyyatları ilə məhdudlaşır.

## Norito & Kodlaşdırma Planı
1. `iroha_data_model`-də alqoritm nömrələrini `Sm2PublicKey`, `Sm2Signature`, `Sm3Digest`, `Sm4Key` ilə genişləndirin.
2. DER qeyri-müəyyənliklərinin qarşısını almaq üçün SM2 imzalarını böyük-endian sabit enli `r∥s` massivləri (32+32 bayt) kimi sıralayın; dönüşümlər adapterlərdə idarə olunur. *(Tamamlandı: `Sm2Signature` köməkçilərində tətbiq olundu; Norito/JSON gediş-gəliş yerində.)*
3. Multiformatlardan istifadə edirsinizsə, multikodek identifikatorlarını (`sm3-256`, `sm2-pub`, `sm4-key`) qeydiyyatdan keçirin, qurğuları və sənədləri yeniləyin. *(İrəliləyiş: `sm2-pub` müvəqqəti kodu `0x1306` indi törəmə açarlarla təsdiqlənib; SM3/SM4 kodları yekun təyinatı gözləyir, `sm_known_answers.toml` vasitəsilə izlənilir.)*
4. Gediş-dönüşləri və səhv formalaşdırılmış kodlaşdırmaların (qısa/uzun r və ya s, etibarsız əyri parametrləri) rədd edilməsini əhatə edən Norito qızıl testlərini yeniləyin.## Host və VM İnteqrasiya Planı (SM-2)
1. Mövcud GOST hash şimini əks etdirən host tərəfi `sm3_hash` sistem çağırışını həyata keçirin; `Sm3Digest::hash`-i təkrar istifadə edin və deterministik xəta yollarını ifşa edin. *(Eniş: host Blob TLV-ni qaytarır; `DefaultHost` tətbiqi və `sm_syscalls.rs` reqressiyasına baxın.)*
2. VM sistem zəngi cədvəlini kanonik r∥ imzalarını qəbul edən, fərqləndirici identifikatorları təsdiqləyən və uğursuzluqları deterministik qayıdış kodları ilə xəritələyən `sm2_verify` ilə genişləndirin. *(Tamamlandı: host + Kotodama intrinsics qaytarılması `1/0`; reqressiya dəsti indi kəsilmiş imzaları, düzgün tərtib edilmiş açıq açarları, blob olmayan TLV-ləri və UTF-8/boş/uyğunsuz Hyperledger faydalı yükləri əhatə edir*)
3. `sm4_gcm_seal`/`sm4_gcm_open` (və istəyə görə CCM) sistem çağırışlarını aydın olmayan/teq ölçüləri (RFC 8998) ilə təmin edin. *(Tamamlandı: GCM sabit 12 baytlıq boşluqlardan + 16 baytlıq teqlərdən istifadə edir; CCM, `r14` vasitəsilə idarə olunan {4,6,8,10,12,14,16} teq uzunluğuna malik 7-13 bayt nonsları dəstəkləyir; Kotodama və Kotodama və I080101 kimi açıqlayır `sm::seal_ccm/open_ccm`.) Tərtibatçının kitabçasında təkrar istifadə etmədən siyasəti sənədləşdirin.*
4. Tel Kotodama tüstü müqavilələri və müsbət və mənfi halları əhatə edən IVM inteqrasiya testləri (dəyişdirilmiş etiketlər, səhv formalaşdırılmış imzalar, dəstəklənməyən alqoritmlər). *(SM3/SM2/SM4 üçün host reqressiyalarını əks etdirən `crates/ivm/tests/kotodama_sm_syscalls.rs` vasitəsilə həyata keçirilib.)*
5. Sistem çağırışına icazə verilən siyahıları, siyasətləri və ABI sənədlərini (`crates/ivm/docs/syscalls.md`) yeniləyin və yeni qeydləri əlavə etdikdən sonra hash edilmiş manifestləri yeniləyin.

### Host və VM İnteqrasiya Statusu
- DefaultHost, CoreHost və WsvHost SM3/SM2/SM4 sistem zənglərini ifşa edir və onları `sm_enabled`-də bağlayır, iş vaxtı bayrağı olduqda `PermissionDenied` qaytarır false.【crates/ivm/src/host.rs:915】【crates/ivm/src/core_host.rs:833】【crates/ivm/src/mock_wsv.rs:2307】
- `crypto.allowed_signing` darvazası boru kəməri/icraçı/dövlət vasitəsilə ötürülür ki, istehsal qovşaqları konfiqurasiya vasitəsilə deterministik şəkildə seçilsin; `sm2`-in əlavə edilməsi SM köməkçisinin əlçatanlığını dəyişdirir.`【crates/iroha_core/src/smartcontracts/ivm/host.rs:170】【crates/iroha_core/src/state.rs:7673】【crator/8:iro】【crates/iro3
- SM3 hashing, SM2 yoxlaması və SM4 GCM/CCM möhürü/açıq üçün reqressiya əhatə dairəsi həm aktiv, həm də qeyri-aktiv yolları (DefaultHost/CoreHost/WsvHost) həyata keçirir. axır.【crates/ivm/tests/sm_syscalls.rs:129】【crates/ivm/tests/sm_syscalls.rs:733】【crates/ivm/tests/sm_syscalls.rs:1036】

## Konfiqurasiya mövzuları
- `crypto.allowed_signing`, `crypto.default_hash`, `crypto.sm2_distid_default` və isteğe bağlı `crypto.enable_sm_openssl_preview`-i `iroha_config`-ə əlavə edin. Məlumat modeli funksiyasının santexnika qurğusunun kripto qutusunu əks etdirdiyinə əmin olun (`iroha_data_model` `sm` → `iroha_crypto/sm`-i ifşa edir).
- Təzahürlər/genez faylları icazə verilən alqoritmləri müəyyən etmək üçün qəbul siyasətlərinə tel konfiqurasiyası; idarəetmə təyyarəsi standart olaraq Ed25519 olaraq qalır.### CLI və SDK İşi (SM-3)
1. **Torii CLI** (`crates/iroha_cli`): SM2 açar gen/import/ixrac (məlumatlı), SM3 hashing köməkçiləri və SM4 AEAD şifrələmə/şifrəni açmaq əmrlərini əlavə edin. İnteraktiv göstərişləri və sənədləri yeniləyin.
2. **Yaradılış aləti** (`xtask`, `scripts/`): manifestlərə icazə verilən imzalama alqoritmlərini və defolt heşləri bəyan etməyə icazə verin, uyğun konfiqurasiya düymələri olmadan SM aktivləşdirilibsə, tez uğursuz olur. *(Tamamlandı: `RawGenesisTransaction` indi `crypto` blokunu `default_hash`/`allowed_signing`/`sm2_distid_default` ilə daşıyır; `ManifestCrypto::validate` və `ManifestCrypto::validate` və I102SMcon0ist parametrlərində reject defaults/genesis manifest snapshotı reklam edir.)*
3. **SDK səthləri**:
   - Pas (`iroha_client`): SM2 imzalama/doğrulama köməkçilərini, SM3 heşinqini, deterministik defoltlarla SM4 AEAD sarğılarını ifşa edin.
   - Python/JS/Swift: Rust API-ni əks etdirmək; dillər arası testlər üçün `sm_known_answers.toml`-də mərhələli qurğulardan təkrar istifadə edin.
4. CLI/SDK sürətli başlanğıclarında SM-i işə salmaq üçün operator iş prosesini sənədləşdirin və JSON/YAML konfiqurasiyalarının yeni alqoritm teqlərini qəbul etməsinə əmin olun.

#### CLI tərəqqisi
- `cargo run -p iroha_cli --features sm -- crypto sm2 keygen --distid CN12345678901234` indi `client.toml` fraqmenti (`public_key_config`, `private_key_hex`, Hyperledger) ilə birlikdə SM2 açar cütünü təsvir edən JSON faydalı yükü yayır. Komanda deterministik nəsil üçün `--seed-hex`-i qəbul edir və hostlar tərəfindən istifadə edilən RFC 6979 törəməsini əks etdirir.
- `cargo xtask sm-operator-snippet --distid CN12345678901234` eyni `sm2-key.json`/`client-sm2.toml` çıxışlarını bir addımda yazaraq, açar gen/ixrac axınını bükür. Avtomatlaşdırma üçün `jq` asılılığını aradan qaldıraraq faylları yönləndirmək və ya onları stdout-a axın etmək üçün `--json-out <path|->` / `--snippet-out <path|->` istifadə edin.
- `iroha_cli tools crypto sm2 import --private-key-hex <hex> [--distid ...]` mövcud materialdan eyni metadata əldə edir ki, operatorlar qəbuldan əvvəl fərqləndirici ID-ləri təsdiq edə bilsinlər.
- `iroha_cli tools crypto sm2 export --private-key-hex <hex> --emit-json` konfiqurasiya parçasını çap edir (`allowed_signing`/`sm2_distid_default` təlimatı daxil olmaqla) və isteğe bağlı olaraq skript üçün JSON açar inventarını yenidən buraxır.
- `iroha_cli tools crypto sm3 hash --data <string>` ixtiyari faydalı yükləri hash edir; `--data-hex` / `--file` ikili girişləri əhatə edir və komanda açıq alətlər üçün hex və base64 həzmlərini bildirir.
- `iroha_cli tools crypto sm4 gcm-seal --key-hex <KEY> --nonce-hex <NONCE> --plaintext-hex <PT>` (və `gcm-open`) host SM4-GCM köməkçilərini və səth `ciphertext_hex`/`tag_hex` və ya açıq mətn yüklərini əhatə edir. `sm4 ccm-seal` / `sm4 ccm-open` CCM üçün eyni UX-ni birdəfəlik uzunluq (7–13 bayt) və etiket uzunluğu (4,6,8,10,12,14,16) ilə təmin edir; hər iki əmr isteğe bağlı olaraq diskə xam baytlar buraxır.## Test Strategiyası
### Vahid/Məlum Cavab Testləri
- SM3 üçün GM/T 0004 & GB/T 32905 vektorları (məsələn, `"abc"`).
- SM4 üçün GM/T 0002 & RFC 8998 vektorları (blok + GCM/CCM).
- SM2 üçün GM/T 0003/GB/T 32918 nümunələri (Z-dəyəri, imza doğrulaması), o cümlədən `ALICE123@YAHOO.COM` ID-li Əlavə 1.
- Aralıq armatur quruluş faylı: `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`.
- Wycheproof mənşəli SM2 reqressiya dəsti (`crates/iroha_crypto/tests/sm2_wycheproof.rs`) indi bit çevirmə, mesaj dəyişdirmə və kəsilmiş imza neqativləri ilə deterministik qurğuları (Əlavə D, SDK toxumları) qatlayan 52 halda korpusa malikdir. Dezinfeksiya edilmiş JSON `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json`-də yaşayır və `sm2_fuzz.rs` onu birbaşa istehlak edir ki, həm xoşbəxt yol, həm də müdaxilə ssenariləri qeyri-səlis/mülk qaçışları arasında uyğunlaşdırılsın. 벡터들은 표준 곡선뿐만 아니라 Əlavə 영역도 다루며, 필요 시 내장 `Sm2PublicKey`Big 백업 루틴이 추적을 완료합니다.
- `cargo xtask sm-wycheproof-sync --input <wycheproof-sm2.json>` (və ya `--input-url <https://…>`) hər hansı yuxarı axın düşməsini (generator etiketi isteğe bağlıdır) deterministik şəkildə kəsir və `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json`-i yenidən yazır. C2SP rəsmi korpusu dərc edənə qədər çəngəlləri əl ilə endirin və köməkçi vasitəsilə qidalandırın; o, açarları, sayları və bayraqları normallaşdırır ki, rəyçilər fərqlər üzərində düşünə bilsinlər.
- `crates/iroha_data_model/tests/sm_norito_roundtrip.rs`-də təsdiqlənmiş SM2/SM3 Norito gediş-gəliş.
- `crates/ivm/tests/sm_syscalls.rs`-də SM3 host sisteminin reqressiyası (SM xüsusiyyəti).
- SM2 `crates/ivm/tests/sm_syscalls.rs`-də sistem çağırışının reqressiyasını yoxlayır (uğur + uğursuzluq halları).

### Mülkiyyət və Reqressiya Testləri
- Etibarsız əyriləri, qeyri-kanonik r/s-ləri rədd edən SM2 təklifi və qeyri-şəxslərin təkrar istifadə edilməsi. *(`crates/iroha_crypto/tests/sm2_fuzz.rs`-də mövcuddur, `sm_proptest`-in arxasındadır; `cargo test -p iroha_crypto --features "sm sm_proptest"` vasitəsilə aktivləşdirin.)*
- Müxtəlif rejimlər üçün uyğunlaşdırılmış Wycheproof SM4 vektorları (blok/AES-rejimi); SM2 əlavələri üçün yuxarı axını izləyin. `sm3_sm4_vectors.rs` indi həm GCM, həm də CCM üçün tag bit-flipləri, kəsilmiş teqləri və şifrəli mətnin dəyişdirilməsini həyata keçirir.

### Qarşılıqlı işləmə və Performans
- RustCrypto ↔ SM2 işarəsi/doğrulanması, SM3 həzmləri və SM4 ECB/GCM üçün OpenSSL/Tongsuo paritet dəsti `crates/iroha_crypto/tests/sm_cli_matrix.rs`-də yaşayır; onu `scripts/sm_interop_matrix.sh` ilə çağırın. CCM paritet vektorları indi `sm3_sm4_vectors.rs`-də işləyir; CLI matrix dəstəyi yuxarı axın CLI-lər CCM köməkçilərini ifşa etdikdən sonra izlənəcək.
- SM3 NEON köməkçisi indi Armv8 sıxılma/doldurma yolunu `sm_accel::is_sm3_enabled` (SM3/SM4-də əks olunan xüsusiyyət + env ləğvetmələri) vasitəsilə iş vaxtı qadağası ilə uçdan-uca idarə edir. Qızıl həzmlər (sıfır/`"abc"`/uzun blok + təsadüfi uzunluqlar) və məcburi söndürmə testləri skalyar RustCrypto backend ilə pariteti saxlayır və Criterion mikro dəzgahı (`crates/sm3_neon/benches/digest.rs`) A.Arch-da NEON6 ilə skalyar ötürmə qabiliyyətini çəkir.
- Ed25519/SHA-2 ilə SM2/SM3/SM4-ü müqayisə etmək və dözümlülük hədlərini təsdiqləmək üçün `scripts/gost_bench.sh`-ni əks etdirən mükəmməl qoşqu.#### Arm64 Baseline (yerli Apple Silicon; Kriteriya `sm_perf`, yenilənmiş 2025-12-05)
- `scripts/sm_perf.sh` indi Kriteriya dəstini idarə edir və `crates/iroha_crypto/benches/sm_perf_baseline.json`-ə qarşı medianı tətbiq edir (aarch64 macOS-da qeydə alınıb; defolt olaraq tolerantlıq 25%, əsas metadata host üçlüyü tutur). Yeni `--mode` bayrağı mühəndislərə skripti redaktə etmədən skaler və NEON vs `sm-neon-force` məlumat nöqtələrini tutmağa imkan verir; cari tutma paketi (xam JSON + ümumiləşdirilmiş xülasə) `artifacts/sm_perf/2026-03-lab/m3pro_native/` altında yaşayır və hər bir faydalı yükü `cpu_label="m3-pro-native"` ilə möhürləyir.
- Sürətləndirmə rejimləri indi müqayisə hədəfi kimi skalyar baza xəttini avtomatik seçir. `scripts/sm_perf.sh` yivləri `--compare-baseline/--compare-tolerance/--compare-label` - `sm_perf_check`, skalar arayışa qarşı hər etalon deltalar yayır və yavaşlama konfiqurasiya edilmiş həddi keçdikdə uğursuz olur. Başlanğıc səviyyəsindən hər bir meyar tolerantlıqları müqayisə qoruyucusunu idarə edir (SM3 Apple skalyar bazasında 12% ilə məhdudlaşır, halbuki SM3 müqayisə deltası indi çırpınmanın qarşısını almaq üçün skalyar istinada qarşı 70%-ə qədər imkan verir); Linux əsas xətləri eyni müqayisə xəritəsindən təkrar istifadə edir, çünki onlar `neoverse-proxy-macos` çəkilişindən ixrac olunur və medianlar fərqli olarsa, biz onları çılpaq metal Neoverse qaçışından sonra sərtləşdirəcəyik. Daha sərt sərhədləri (məsələn, `--compare-tolerance 0.20`) əldə edərkən `--compare-tolerance`-i açıq şəkildə keçin və alternativ istinad hostlarına şərh əlavə etmək üçün `--compare-label`-dən istifadə edin.
- CI istinad maşınında qeydə alınan əsas xətlər indi `crates/iroha_crypto/benches/sm_perf_baseline_aarch64_macos_scalar.json`, `sm_perf_baseline_aarch64_macos_auto.json` və `sm_perf_baseline_aarch64_macos_neon_force.json`-də yaşayır. Onları `scripts/sm_perf.sh --mode scalar --write-baseline`, `--mode auto --write-baseline` və ya `--mode neon-force --write-baseline` ilə yeniləyin (çəkmədən əvvəl `SM_PERF_CPU_LABEL` təyin edin) və yaradılan JSON-u işləmə qeydləri ilə birlikdə arxivləşdirin. Rəyçilər hər bir nümunəni yoxlaya bilməsi üçün ümumi köməkçi çıxışı (`artifacts/.../aggregated.json`) PR ilə birlikdə saxlayın. Linux/Neoverse bazaları indi `sm_perf_baseline_aarch64_unknown_linux_gnu_{mode}.json`-də göndərilir, `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/aggregated.json`-dən irəli gəlir (CPU etiketi `neoverse-proxy-macos`, SM3 aarch64 macOS/Linux üçün 0,70 tolerantlığı müqayisə edir); tolerantlıqları gücləndirmək üçün mövcud olduqda çılpaq metal Neoverse hostlarında təkrar işləyin.
- Baseline JSON faylları indi hər etalon üçün qoruyucuları bərkitmək üçün əlavə `tolerances` obyekti daşıya bilər. Misal:
  ```json
  {
    "benchmarks": { "...": 12.34 },
    "tolerances": {
      "sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt": 0.08,
      "sm3_vs_sha256_hash/sm3_hash": 0.12
    }
  }
  ```
  `sm_perf_check` siyahıda olmayan hər hansı meyarlar üçün qlobal CLI tolerantlığından istifadə edərkən bu fraksiya limitlərini (nümunədə 8% və 12%) tətbiq edir.
- Müqayisə mühafizəçiləri müqayisənin əsas xəttində də `compare_tolerances`-ə hörmət edə bilərlər. Birbaşa baza yoxlamaları üçün əsas `tolerances`-i ciddi saxlayarkən, skalyar arayışa (məsələn, `\"sm3_vs_sha256_hash/sm3_hash\": 0.70`) qarşı daha boş deltaya icazə vermək üçün bundan istifadə edin.- Yoxlanılan Apple Silicon bazaları indi beton qoruyucularla təchiz olunur: SM2/SM4 əməliyyatları variasiyadan asılı olaraq 12-20% sürüşməyə imkan verir, SM3/ChaCha müqayisələri isə 8-12% səviyyəsindədir. Skayar baza xəttinin `sm3` tolerantlığı indi 0,12-ə qədər sərtləşdirilib; `unknown_linux_gnu` faylları `neoverse-proxy-macos` ixracını eyni dözümlülük xəritəsi (SM3 ilə müqayisə 0.70) və metadata qeydləri ilə əks etdirir və onların çılpaq metal Neoverse təkrarı mövcud olana qədər Linux qapısı üçün göndərildiyini göstərir.
- SM2 imzalanması: əməliyyat başına 298µs (Ed25519: 32µs) ⇒ ~9,2× daha yavaş; doğrulama: 267µs (Ed25519: 41µs) ⇒ ~6,5× daha yavaş.
- SM3 hashing (4KiB faydalı yük): 11,2µs, 11,3µs-də SHA-256 ilə effektiv paritet (≈356MiB/s vs 353MiB/s).
- SM4-GCM möhürü/açıq (1KiB faydalı yük, 12 bayt olmayan): 15,5µs və 1,78µs-də ChaCha20-Poly1305 (≈64MiB/s qarşı 525MiB/s).
- Reproduktivlik üçün çəkilmiş etalon artefaktlar (`target/criterion/sm_perf*`); Linux bazaları `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/`-dən (CPU etiketi `neoverse-proxy-macos`, SM3 tolerantlığı 0,70 ilə müqayisə edir) qaynaqlanır və tolerantlığı gücləndirmək üçün laboratoriya vaxtı açıldıqdan sonra çılpaq metal Neoverse hostlarda (`SM-4c.1`) yenilənə bilər.

#### Çarpaz memarlıq ələ keçirmə siyahısı
- `scripts/sm_perf_capture_helper.sh` **hədəf maşında** işə salın (x86_64 iş stansiyası, Neoverse ARM server və s.). Çəkilişləri möhürləmək üçün `--cpu-label <host>`-i keçin və (matris rejimində işləyərkən) laboratoriya planlaşdırması üçün yaradılan planı/əmrləri əvvəlcədən doldurmaq üçün. Köməkçi rejimə uyğun əmrləri çap edir:
  1. Kriteriya dəstini düzgün xüsusiyyətlər dəsti ilə icra edin və
  2. medianları `crates/iroha_crypto/benches/sm_perf_baseline_${arch}_${os}_${mode}.json`-ə yazın.
- Əvvəlcə skalyar baza xəttini çəkin, sonra `auto` (və AArch64 platformalarında `neon-force`) üçün köməkçini yenidən işə salın. Rəyçilər JSON metadatasında host təfərrüatlarını izləyə bilməsi üçün mənalı `SM_PERF_CPU_LABEL` istifadə edin.
- Hər qaçışdan sonra xam `target/criterion/sm_perf*` kataloqunu arxivləşdirin və yaradılan əsaslarla birlikdə onu PR-a daxil edin. Ardıcıl iki qaçış stabilləşən kimi hər bir etalon tolerantlıqlarını sərtləşdirin (istinad formatı üçün `sm_perf_baseline_aarch64_macos_*.json`-ə baxın).
- Bu bölmədə medianları + tolerantlıqları qeyd edin və yeni arxitektura əhatə edildikdə `status.md`/`roadmap.md`-i yeniləyin. Linux əsas xətləri indi `neoverse-proxy-macos` çəkilişindən yoxlanılır (metadata aarch64-naməlum-linux-gnu qapısına ixracı qeyd edir); bu laboratoriya yuvaları mövcud olduqda, çılpaq metal Neoverse/x86_64 hostlarında izləmə olaraq təkrar işə salın.

#### ARMv8 SM3/SM4 intrinsics vs skaler yollar
`sm_accel` (bax: `crates/iroha_crypto/src/sm.rs:739`) NEON dəstəkli SM3/SM4 köməkçiləri üçün iş vaxtı göndərmə qatını təmin edir. Xüsusiyyət üç səviyyədə qorunur:| Layer | Nəzarət | Qeydlər |
|-------|---------|-------|
| Kompilyasiya vaxtı | `--features sm` (indi `sm-neon`-də avtomatik olaraq `aarch64`-də çəkilir) və ya `sm-neon-force` (testlər/bençmarklar) | NEON modullarını qurur və `sm3-neon`/`sm4-neon` əlaqələndirir. |
| Runtime avtomatik aşkarlama | `sm4_neon::is_supported()` | Yalnız AES/PMULL ekvivalentlərini ifşa edən CPU-lar üçün doğrudur (məsələn, Apple M-seriyası, Neoverse V1/N2). NEON və ya FEAT_SM4-ü maskalayan VM-lər yenidən skalyar koda düşür. |
| Operatorun ləğvi | `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`) | Başlanğıcda tətbiq edilən konfiqurasiyaya əsaslanan göndərmə; `force-enable`-dən yalnız etibarlı mühitlərdə profil yaratmaq üçün istifadə edin və skalyar ehtiyatları təsdiq edərkən `force-disable`-ə üstünlük verin. |

**Performans zərfi (Apple M3 Pro; medianlar `sm_perf_baseline_aarch64_macos_{mode}.json`-də qeydə alınıb):**

| Rejim | SM3 həzm (4KiB) | SM4-GCM möhürü (1KiB) | Qeydlər |
|------|-------------------|----------------------|-------|
| Skalyar | 11.6µs | 15.9µs | Deterministik RustCrypto yolu; hər yerdə istifadə olunur `sm` xüsusiyyəti tərtib edilir, lakin NEON mövcud deyil. |
| NEON avto | ~2,7 × skalar |-dən daha sürətli ~2,3× skaler |-dən daha sürətli Cari NEON ləpələri (SM-5a.2c) cədvəli bir anda dörd sözlə genişləndirir və ikili növbə fan-out istifadə edir; dəqiq medianlar hər hosta görə dəyişir, ona görə də əsas JSON metadatasına müraciət edin. |
| NEON qüvvəsi | NEON avtomatik güzgülər, lakin geri qaytarmağı tamamilə söndürür | NEON auto | ilə eyni `scripts/sm_perf.sh --mode neon-force` vasitəsilə həyata keçirilir; CI-ni hətta standart olaraq skalar rejimə keçirəcək hostlarda da dürüst saxlayır. |

**Determinizm və yerləşdirmə təlimatı**
- İntrinsiklər heç vaxt müşahidə olunan nəticələri dəyişməz—`sm_accel` sürətləndirilmiş yol əlçatmaz olduqda `None` qaytarır, beləliklə skalyar köməkçi işləyir. Buna görə də konsensus kodu yolları skalyar tətbiq düzgün olduğu müddətcə deterministik olaraq qalır.
- NEON yolunun istifadə edilib-edilməməsi ilə bağlı biznes məntiqini ** etməyin. Sürətlənməni sırf mükəmməl bir işarə kimi qəbul edin və statusu yalnız telemetriya vasitəsilə ifşa edin (məsələn, `sm_intrinsics_enabled` ölçmə cihazı).
- SM koduna toxunduqdan sonra həmişə `ci/check_sm_perf.sh` (və ya `make check-sm-perf`) işə salın ki, Kriteriya qoşqu hər bir baza JSON-a daxil edilmiş tolerantlıqlardan istifadə edərək həm skalar, həm də sürətləndirilmiş yolları təsdiq etsin.
- Müqayisə və ya sazlama zamanı kompilyasiya vaxtı bayraqları ilə müqayisədə `crypto.sm_intrinsics` konfiqurasiya düyməsinə üstünlük verin; `sm-neon-force` ilə təkrar kompilyasiya skalyar geri dönüşü tamamilə söndürür, `force-enable` isə sadəcə olaraq iş vaxtının aşkarlanmasını sıxışdırır.
- Seçilmiş siyasəti buraxılış qeydlərində sənədləşdirin: istehsal qurğuları `Auto`-də siyasəti tərk etməlidir, hər bir təsdiqləyiciyə eyni binar artefaktları paylaşarkən müstəqil olaraq aparat imkanlarını kəşf etməyə imkan verir.
- Eyni göndərmə və sınaq axınına hörmət etmədikcə, statik olaraq əlaqəli təchizatçı intrinsiklərini (məsələn, üçüncü tərəf SM4 kitabxanaları) qarışdıran ikili faylların göndərilməsindən çəkinin - əks halda əsas alətlərimiz tərəfindən mükəmməl reqressiyalar tutulmayacaq.#### x86_64 Rosetta bazası (Apple M3 Pro; çəkiliş 2025-12-01)
- Əsas xətlər `artifacts/sm_perf/2026-03-lab/m3pro_rosetta/` altında xam + yığılmış çəkilişlərlə `crates/iroha_crypto/benches/sm_perf_baseline_x86_64_macos_{scalar,auto,neon_force}.json` (cpu_label=`m3-pro-rosetta`) içərisində yaşayır.
- x86_64-də hər bir standart tolerans SM2 üçün 20%, Ed25519/SHA-256 üçün 15% və SM4/ChaCha üçün 12% olaraq təyin edilib. `scripts/sm_perf.sh` indi qeyri-AAarch64 hostlarda sürətlənmə müqayisəsi tolerantlığını 25%-ə defolt edir, beləliklə, Neoverse təkrar yayımlanana qədər paylaşılan `m3-pro-native` baza xətti üçün AArch64-də 5.25 boşluq buraxarkən skalyar-avtomatik sabit qalır.

| Benchmark | Skalyar | Avto | Neon-Force | Avto vs Skalar | Neon vs Skalar | Neon vs Avto |
|----------|--------|------|------------|----------------|---------------|--------------|
| sm2_vs_ed25519_sign/ed25519_sign |    57.43 |  57.12 |      55.77 |          -0,53% |         -2,88% |        -2,36% |
| sm2_vs_ed25519_sign/sm2_sign |   572.76 | 568.71 |     557.83 |          -0,71% |         -2,61% |        -1,91% |
| sm2_vs_ed25519_verify/verify/ed25519 |    69.03 |  68.42 |      66.28 |          -0,88% |         -3,97% |        -3,12% |
| sm2_vs_ed25519_verify/verify/sm2 |   521.73 | 514.50 |     502.17 |          -1,38% |         -3,75% |        -2,40% |
| sm3_vs_sha256_hash/sha256_hash |    16.78 |  16.58 |      16.16 |          -1,19% |         -3,69% |        -2,52% |
| sm3_vs_sha256_hash/sm3_hash |    15.78 |  15.51 |      15.04 |          -1,71% |         -4,69% |        -3,03% |
| sm4_vs_chacha20poly1305_decrypt/chacha20poly1305_decrypt |     1.96 |   1.97 |       1.97 |           0,39% |          0,16% |        -0,23% |
| sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt |    16.26 |  16.38 |      16.26 |           0,72% |         -0,01% |        -0,72% |
| sm4_vs_chacha20poly1305_encrypt/chacha20poly1305_encrypt |     1.96 |   2.00 |       1.93 |           2,23% |         -1,14% |        -3,30% |
| sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt |    16.60 |  16.58 |      16.15 |          -0,10% |         -2,66% |        -2,57% |

#### x86_64 / aarch64 olmayan digər hədəflər
- Cari konstruksiyalar hələ də x86_64-də yalnız deterministik RustCrypto skalyar yolunu göndərir; `sm`-i aktiv saxlayın, lakin SM-4c.1b yerə enənə qədər xarici AVX2/VAES ləpələrini yeritməyin. İcra zamanı siyasəti ARM-i əks etdirir: defolt olaraq `Auto`, `crypto.sm_intrinsics`-ə hörmət edin və eyni telemetriya ölçü cihazlarını yerləşdirin.
- Linux/x86_64 ələ keçirmələri qeydə alınmalıdır; həmin aparatda köməkçidən yenidən istifadə edin və yuxarıdakı Rosetta əsas xətləri və tolerantlıq xəritəsi ilə yanaşı medianı `sm_perf_baseline_x86_64_unknown_linux_gnu_{mode}.json`-ə buraxın.**Ümumi tələlər**
1. **Virtuallaşdırılmış ARM nümunələri:** Bir çox bulud NEON-u ifşa edir, lakin `sm4_neon::is_supported()`-in yoxladığı SM4/AES genişləndirmələrini gizlədir. Həmin mühitlərdə skalyar yolu gözləyin və müvafiq olaraq mükəmməl əsas xətləri çəkin.
2. **Qismən ləğvetmələr:** Qaçışlar arasında davamlı `crypto.sm_intrinsics` dəyərlərinin qarışdırılması uyğunsuz perf oxunuşlarına gətirib çıxarır. Təcrübə biletində nəzərdə tutulan ləğvi sənədləşdirin və yeni əsas xətləri çəkməzdən əvvəl konfiqurasiyanı sıfırlayın.
3. **CI pariteti:** Bəzi macOS koşucuları NEON aktiv olduqda əks əsaslı perf seçməyə icazə vermir. `scripts/sm_perf_capture_helper.sh` çıxışlarını PR-lərə əlavə edin ki, rəyçilər hətta qaçışçı həmin sayğacları gizlətsə belə, sürətləndirilmiş yolun həyata keçirildiyini təsdiq etsinlər.
4. **Gələcək ISA variantları (SVE/SVE2):** Cari nüvələr NEON zolaq formalarını qəbul edir. SVE/SVE2-yə keçməzdən əvvəl `sm_accel::NeonPolicy`-i xüsusi variantla genişləndirin ki, biz CI, telemetriya və operator düymələrini uyğunlaşdıraq.

SM-5a/SM-4c.1 altında izlənilən fəaliyyət elementləri CI-nin hər bir yeni arxitektura üçün paritet sübutlarını əldə etməsini təmin edir və yol xəritəsi Neoverse/x86 əsas xətlər və NEON-vs-skalyar dözümlülüklər yaxınlaşana qədər 🈺-də qalır.

## Uyğunluq və Tənzimləyici Qeydlər

### Standartlar və Normativ İstinadlar
- **GM/T 0002-2012** (SM4), **GM/T 0003-2012** + **GB/T 32918 seriyası** (SM2), **GM/T 0004-2012** + **GB/T 32905/32907** (SM389) və qurğularımızın istehlak etdiyi təriflər, test vektorları və KDF bağlamaları.【docs/source/crypto/sm_vectors.md#L79】
- `docs/source/crypto/sm_compliance_brief.md` uyğunluq brifinqi mühəndislik, SRE və hüquq qrupları üçün sənədləşdirmə/ixrac öhdəlikləri ilə yanaşı bu standartları çarpaz bağlayır; GM/T kataloqu yenidən nəzərdən keçirildikdə həmin qısa məlumatı yeniləyin.

### Materik Çin Tənzimləmə İş axını
1. **Məhsulun təqdim edilməsi (开发备案):** Materik Çindən SM-i təmin edən ikili faylları göndərməzdən əvvəl artefakt manifestini, deterministik qurma addımlarını və asılılıq siyahısını əyalət kriptoqrafiya administrasiyasına təqdim edin. Sənədləşdirmə şablonları və uyğunluq yoxlama siyahısı `docs/source/crypto/sm_compliance_brief.md` və qoşmalar kataloqunda (`sm_product_filing_template.md`, `sm_sales_usage_filing_template.md`, `sm_export_statement_template.md`) mövcuddur.
2. **Satış/İstifadə sənədləri (销售/使用备案):** Quruda SM-i aktivləşdirən qovşaqları idarə edən operatorlar öz yerləşdirmə əhatə dairəsini, əsas idarəetmə mövqeyini və telemetriya planını qeydiyyatdan keçirməlidirlər. Sənəd verərkən imzalanmış manifestləri və `iroha_sm_*` metrik snapşotları əlavə edin.
3. **Akkreditasiya olunmuş sınaq:** Kritik infrastruktur operatorları sertifikatlaşdırılmış laboratoriya hesabatları tələb edə bilər. Aşağı axın auditorlarının kodu dəyişmədən vektorları təkrar istehsal edə bilməsi üçün təkrarlana bilən tikinti skriptləri, SBOM ixracları və Wycheproof/interop artefaktları (aşağıya baxın) təmin edin.
4. **Statusun izlənməsi:** Tamamlanmış sənədləri buraxılış biletində və `status.md`-də qeyd edin; çatışmayan sənədlər yalnız doğrulamadan imzalayan pilotlara qədər təşviqi bloklayır.### İxrac və Dağıtım Duruşu
- SM qabiliyyətli binar faylları **US EAR Kateqoriya 5           **** və **Aİ Qaydaları 2021/821 Əlavə 1 (5D002)** altında idarə olunan elementlər kimi qəbul edin. Mənbənin nəşri açıq mənbəli/ENC bölgüləri üçün uyğun olmağa davam edir, lakin embarqo edilmiş istiqamətlərə yenidən bölüşdürülməsi hələ də hüquqi yoxlama tələb edir.
- Buraxılış manifestləri ENC/TSU əsasına istinad edən ixrac bəyanatını toplamalı və FFI önizləməsi qablaşdırılıbsa, OpenSSL/Tongsuo qurma identifikatorlarını siyahıya almalıdır.
- Transsərhəd ötürmə problemlərindən qaçmaq üçün operatorlar quruda paylanmaya ehtiyac duyduqda region-yerli qablaşdırmaya (məsələn, materik güzgülərinə) üstünlük verin.

### Operator Sənədləri və Sübutları
- Bu memarlıq xülasəsini `docs/source/crypto/sm_operator_rollout.md`-də təqdim olunan yoxlama siyahısı və `docs/source/crypto/sm_compliance_brief.md`-də uyğunluq sənədləşdirmə təlimatı ilə birləşdirin.
- Genesis/operator sürətli başlanğıcını `docs/genesis.md`, `docs/genesis.he.md` və `docs/genesis.ja.md` arasında sinxronlaşdırın; SM2/SM3 CLI iş axını `crypto` manifestlərini əkmək üçün operatorla üz-üzə olan həqiqət mənbəyidir.
- Arxiv OpenSSL/Tongsuo mənşəyi, `scripts/sm_openssl_smoke.sh` çıxışı və hər buraxılış paketi ilə `scripts/sm_interop_matrix.sh` paritet qeydləri, beləliklə, uyğunluq və audit partnyorlarının deterministik artefaktları var.
- Proqram vəziyyətini aşkar saxlamaq üçün uyğunluq əhatə dairəsi dəyişdikdə (yeni yurisdiksiyalar, sənədlərin doldurulması və ya ixrac qərarları) `status.md`-i yeniləyin.
- `docs/source/release_dual_track_runbook.md`-də çəkilmiş mərhələli hazırlıq baxışlarına (`SM-RR1`–`SM-RR3`) əməl edin; Yalnız doğrulama, pilot və GA imzalama mərhələləri arasında təşviqat orada sadalanan artefaktları tələb edir.

## Qarşılıqlı Təriflər

### RustCrypto ↔ OpenSSL/Tongsuo Matrisi
1. OpenSSL/Tongsuo CLI-lərin mövcud olduğundan əmin olun (`IROHA_SM_CLI="openssl /opt/tongsuo/bin/openssl"` açıq alət seçiminə imkan verir).
2. `scripts/sm_interop_matrix.sh`-i işə salın; o, `cargo test -p iroha_crypto --test sm_cli_matrix --features sm`-i işə salır və SM2 işarələmə/doğrulama, SM3 həzm və SM4 ECB/GCM hərəkətlərini hər bir provayderə qarşı həyata keçirir, olmayan hər hansı CLI-ni atlayır.【scripts/sm_interop_matrix.sh#L1】
3. Nəticə olan `target/debug/deps/sm_cli_matrix*.log` fayllarını buraxılış artefaktları ilə arxivləşdirin.

### OpenSSL Önizləmə Dumanı (Qablaşdırma Qapısı)
1. OpenSSL ≥3.0 inkişaf başlıqlarını quraşdırın və `pkg-config`-in onları tapa biləcəyinə əmin olun.
2. `scripts/sm_openssl_smoke.sh` yerinə yetirin; köməkçi `cargo check`/`cargo test --test sm_openssl_smoke`-i işlədir, SM3 hashing, SM2 yoxlanışı və FFI backend vasitəsilə SM4-GCM gediş-gəlişini həyata keçirir (test qoşqu açıq şəkildə önizləməni təmin edir).【scripts/sm_openssl_smo
3. Hər hansı bir ötürmə uğursuzluğunu buraxma blokeri kimi qəbul edin; audit sübutu üçün konsol çıxışını əldə edin.

### Deterministik Quraşdırma Yeniləmə
- Hər uyğunluq təqdim etməzdən əvvəl SM qurğularını (`sm_vectors.md`, `fixtures/sm/…`) bərpa edin, sonra paritet matrisini və tüstü kəmərini yenidən işə salın ki, auditorlar sənədlərlə yanaşı təzə deterministik transkriptlər də alsınlar.## Xarici Auditə Hazırlıq
- `docs/source/crypto/sm_audit_brief.md` xarici baxış üçün kontekst, əhatə dairəsi, cədvəl və kontaktları paketləşdirir.
- Audit artefaktları `docs/source/crypto/attachments/` (OpenSSL tüstü jurnalı, yük ağacının şəkli, yük metadatasının ixracı, alət dəstinin mənşəyi) və `fuzz/sm_corpus_manifest.json` (mövcud reqressiya vektorlarından əldə edilən deterministik SM qeyri-səlis toxumları) altında yaşayır. macOS-da tüstü jurnalı hazırda buraxılmış qaçışı qeyd edir, çünki iş sahəsindən asılılıq dövrü `cargo check`-in qarşısını alır; Dövr olmadan qurulan Linux önizləmə arxa ucunu tam istifadə edəcək.
- RFQ göndərilməsindən əvvəl uyğunlaşma üçün 2026-01-30 tarixində Crypto WG, Platform Ops, Security və Docs/DevRel rəhbərlərinə tirajlandı.

### Audit Məşğulluğu Status

- **Bitlərin izi (CN kriptoqrafiya təcrübəsi)** — **2026-02-21** tarixində yerinə yetirilən İşin Bəyanatı, başlanğıc **2026-02-24**, sahə işi pəncərəsi **2026-02-24-2026-03-22**, yekun hesabat **4-2016**0-a qədərdir. Crypto WG rəhbəri və Təhlükəsizlik Mühəndisliyi əlaqəsi ilə həftəlik status yoxlama məntəqəsi hər çərşənbə günü saat 09:00UTC. Əlaqələr, çatdırılma materialları və sübut əlavələri üçün [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) baxın.
- **NCC Group APAC (fövqəladə vəziyyət yuvası)** — Əlavə tapıntılar və ya tənzimləyici sorğular ikinci rəyi tələb edərsə, 2026-cı ilin may pəncərəsi təqib/paralel baxış kimi qorunur. Nişan təfərrüatları və eskalasiya qarmaqları `sm_audit_brief.md`-də Bitlərin İzi girişi ilə yanaşı qeyd olunur.

## Risklər və azaldılması

Tam qeydiyyat: ətraflı məlumat üçün bax [`sm_risk_register.md`](sm_risk_register.md)
ehtimal/təsir qiymətləndirilməsi, monitorinq tetikleyicileri və imzalanma tarixçəsi. The
Aşağıdakı xülasə mühəndisliyi buraxmaq üçün ortaya çıxan başlıq maddələrini izləyir.
| Risk | Ciddilik | Sahibi | Azaldılması |
|------|----------|-------|------------|
| RustCrypto SM qutuları üçün xarici auditin olmaması | Yüksək | Kripto WG | Contract Trail of Bits/NCC Group, yalnız audit hesabatı qəbul edilənə qədər yoxlayın. |
| SDK-lar arasında deterministik olmayan reqressiyalar | Yüksək | SDK Proqram Rəhbərləri | SDK CI üzrə qurğuları paylaşın; kanonik r∥s kodlamasını tətbiq etmək; çarpaz SDK inteqrasiya testləri əlavə edin (SM-3c-də izlənilir). |
| İntrinsiklərdə ISA-ya xas səhvlər | Orta | Performans WG | Xüsusiyyət qapısının intrinsikləri, ARM-də CI əhatə dairəsini tələb edir, proqram təminatının geri qaytarılmasını təmin edir. `sm_perf.md`-də saxlanılan hardware doğrulama matrisi. |
| Uyğunluq qeyri-müəyyənliyi övladlığa götürməyi gecikdirir | Orta | Sənədlər və Hüquqi Əlaqə | GA-dan əvvəl uyğunluq haqqında qısa məlumat və operator yoxlama siyahısını (SM-6a/SM-6b) dərc edin; hüquqi məlumat toplamaq. Sənədləşdirmə yoxlama siyahısı `sm_compliance_brief.md`-də göndərildi. |
| Provayder yeniləmələri ilə FFI backend drift | Orta | Platforma Əməliyyatları | Provayder versiyalarını bağlayın, paritet testləri əlavə edin, qablaşdırma sabitləşənə qədər FFI backend-ə qoşulun (SM-P3). |## Açıq Suallar / İzləmələr
1. Rust-da SM alqoritmləri ilə təcrübəsi olan müstəqil audit tərəfdaşlarını seçin.
   - **Cavab (24-02-2026):** Bits-in CN kriptoqrafiya təcrübəsi ilkin audit SOW-a imza atdı (2026-02-24, çatdırılma tarixi 2026-04-15) və NCC Group APAC may fövqəladə hallar üçün vaxt ayırır, beləliklə tənzimləyicilər yenidən yoxlanılmadan ikinci yoxlama tələb edə bilər. Nişan sahəsi, kontaktlar və yoxlama siyahıları [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) daxilində yaşayır və `sm_audit_vendor_landscape.md`-də əks olunur.
2. Rəsmi Wycheproof SM2 verilənlər bazası üçün yuxarı axını izləməyə davam edin; iş sahəsi hazırda seçilmiş 52 iş dəstini (deterministik qurğular + sintez edilmiş dəyişdirmə qutuları) göndərir və onu `sm2_wycheproof.rs`/`sm2_fuzz.rs`-ə ötürür. Yuxarı JSON enişindən sonra korpusu `cargo xtask sm-wycheproof-sync` vasitəsilə yeniləyin.
   - Bouncy Castle və GmSSL mənfi vektor paketlərini izləyin; mövcud korpusu əlavə etmək üçün lisenziyalaşdırma təmizləndikdən sonra `sm2_fuzz.rs`-ə idxal edin.
3. SM qəbulunun monitorinqi üçün əsas telemetriyanı (ölçmələr, qeydlər) müəyyənləşdirin.
4. Kotodama/VM ifşası üçün SM4 AEAD defoltunun GCM və ya CCM olmasına qərar verin.
5. Əlavə Nümunə 1 üçün RustCrypto/OpenSSL paritetini izləyin (ID `ALICE123@YAHOO.COM`): nəşr edilmiş açıq açar və `(r, s)` üçün kitabxana dəstəyini təsdiqləyin ki, qurğular reqressiya testlərinə yüksəlsin.

## Fəaliyyət elementləri
- [x] Asılılıq auditini yekunlaşdırın və təhlükəsizlik izləyicisində ələ keçirin.
- [x] RustCrypto SM qutuları üçün audit partnyorunun əlaqəsini təsdiqləyin (SM-P0 təqibi). Trail of Bits (CN kriptoqrafiya təcrübəsi) `sm_audit_brief.md`-də qeydə alınmış başlanğıc/çatdırılma tarixləri ilə ilkin baxışa sahibdir və NCC Group APAC tənzimləyici və ya idarəetmə təqiblərini təmin etmək üçün 2026-cı ilin may ayında gözlənilməz hallar üçün boşluq saxladı.
- [x] SM4 CCM müdaxilə halları (SM-4a) üçün Wycheproof əhatə dairəsini genişləndirin.
- [x] SDK-lar və CI-yə naqil (SM-3c/SM-1b.1) arasında yerin kanonik SM2 imzalama qurğuları; `scripts/check_sm2_sdk_fixtures.py` tərəfindən qorunur (bax: `ci/check_sm2_sdk_fixtures.sh`).

## Uyğunluq Əlavəsi (Dövlət Kommersiya Kriptoqrafiyası)

- **Təsnifat:** SM2/SM3/SM4 Çinin *dövlət kommersiya kriptoqrafiyası* rejimi altında göndərilir (ÇXR Kriptoqrafiya Qanunu, Maddə 3). Bu alqoritmlərin Iroha proqram təminatında göndərilməsi layihəni əsas/ümumi (dövlət sirri) səviyyələrində yerləşdirmir, lakin ÇXR yerləşdirmələrində onlardan istifadə edən operatorlar kommersiya-kripto sənədləri və MLPS öhdəliklərinə əməl etməlidirlər.
- **Standartlar xətti:** İctimai sənədləri GM/T xüsusiyyətlərinin rəsmi GB/T çevrilmələri ilə uyğunlaşdırın:

| Alqoritm | GB/T arayışı | GM/T mənşəyi | Qeydlər |
|----------|----------------|-------------|-------|
| SM2 | GB/T32918 (bütün hissələr) | GM/T0003 | ECC rəqəmsal imza + açar mübadiləsi; Iroha əsas qovşaqlarda yoxlamanı və SDK-lara deterministik imzalamağı ifşa edir. |
| SM3 | GB/T32905 | GM/T0004 | 256-bit hash; skalyar və ARMv8 sürətləndirilmiş yollar üzrə deterministik hashing. |
| SM4 | GB/T32907 | GM/T0002 | 128 bitlik blok şifrəsi; Iroha GCM/CCM köməkçiləri təqdim edir və tətbiqlər arasında böyük-endian paritetini təmin edir. |- **Bacarıq manifesti:** Torii `/v1/node/capabilities` son nöqtəsi aşağıdakı JSON formasını reklam edir, beləliklə operatorlar və alətlər SM manifestini proqramlı şəkildə istehlak edə bilsin:

```json
{
  "supported_abi_versions": [1],
  "default_compile_target": 1,
  "data_model_version": 1,
  "crypto": {
    "sm": {
      "enabled": true,
      "default_hash": "sm3-256",
      "allowed_signing": ["ed25519"],
      "sm2_distid_default": "1234567812345678",
      "openssl_preview": false,
      "acceleration": {
        "scalar": true,
        "neon_sm3": false,
        "neon_sm4": false,
        "policy": "auto"
      }
    }
  }
}
```

CLI alt əmri `iroha runtime capabilities` uyğunluq sübutlarının toplanması üçün JSON reklamı ilə yanaşı bir sətirlik xülasə çap etdirərək eyni faydalı yükü yerli olaraq üzə çıxarır.

- **Sənədləşmənin nəticələri:** yuxarıdakı alqoritmləri/standartları müəyyən edən buraxılış qeydləri və SBOM-ları dərc edin və buraxılış artefaktları ilə birləşdirilmiş tam uyğunluq brifinqini (`sm_chinese_crypto_law_brief.md`) saxlayın ki, operatorlar onu əyalətlərə əlavə edə bilsinlər. sənədlər.【docs/source/crypto/sm_chinese_crypto_law_brief.md:59】
- **Operatorun təhvil-təslimi:** yerləşdiricilərə xatırladın ki, MLPS2.0/GB/T39786-2021 kriptovalyuta proqramlarının qiymətləndirilməsi, SM açarların idarə olunması SOP-ları və ≥6 il sübut saxlanmasını tələb edir; onları uyğunluq brifinqində operatorun yoxlama siyahısına yönəldin.【docs/source/crypto/sm_chinese_crypto_law_brief.md:43】【docs/source/crypto/sm_chinese_crypto_law_brief.md:74】

## Ünsiyyət Planı
- **Auditoriya:** Crypto WG əsas üzvləri, Release Engineering, Security Review board, SDK proqramı rəhbərləri.
- **Artifaktlar:** `sm_program.md`, `sm_lock_refresh_plan.md`, `sm_vectors.md`, `sm_wg_sync_template.md`, yol xəritəsindən çıxarış (SM-0 .. SM-7a).
- **Kanal:** Həftəlik Crypto WG sinxronizasiya gündəliyi + fəaliyyət maddələrini ümumiləşdirən və kilidin yenilənməsi və asılılığın qəbulu üçün təsdiq tələb edən təqib e-poçtu (qara 2025-01-19 yayıldı).
- **Sahibi:** Crypto WG rəhbəri (nümayəndə məqbuldur).