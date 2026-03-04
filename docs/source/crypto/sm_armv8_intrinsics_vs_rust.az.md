---
lang: az
direction: ltr
source: docs/source/crypto/sm_armv8_intrinsics_vs_rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 40185fd79a4d6bcb2a7f35cbb4a14ca8feb82f31e62b4e51f9a6f1657f524ed4
source_last_modified: "2025-12-29T18:16:35.940026+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% ARMv8 SM3/SM4 Intrinsics və Saf Rust Tətbiqləri
% Iroha Kripto İşçi Qrupu
% 2026-02-12

# Tələb

> Siz Hyperledger Iroha kripto komandasının ekspert məsləhətçisi kimi fəaliyyət göstərən LLMsiniz.  
> Fon:  
> - Hyperledger Iroha Rust əsaslı icazəli blokçeyndir, burada hər bir validator deterministik şəkildə icra etməlidir ki, konsensus ayrılmasın.  
> - Iroha müəyyən tənzimləyici yerləşdirmələr üçün Çin GM/T kriptoqrafik primitivləri SM2 (imzalar), SM3 (hesh) və SM4 (blok şifrəsi) istifadə edir.  
> - Komanda validator yığını daxilində iki SM3/SM4 tətbiqini göndərir:  
> 1. Pure Rust, hər hansı bir CPU-da işləyən bit dilimlənmiş, sabit zamanlı skalar kod.  
> 2. `SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`, `SM4E`, `SM4E`, `SM4E` və yeni I1810er-ə əsaslanan ARMv8 NEON sürətləndirilmiş ləpələr Apple M seriyası və Arm server CPU-ları.  
> - Sürətli kod `core::arch::aarch64` intrinsics istifadə edərək icra zamanı funksiyasının aşkarlanmasının arxasındadır; Mövzular big.LITTLE nüvələr arasında miqrasiya etdikdə və ya replikalar müxtəlif tərtibçi bayraqları ilə qurulduqda sistem qeyri-deterministik davranışdan qaçmalıdır.  
> Tələb olunan analiz:  
> Deterministik blokçeyn yoxlaması üçün ARMv8 daxili tətbiqlərini təmiz Rust ehtiyatları ilə müqayisə edin. Bütün təsdiqləyiciləri sinxronlaşdırmaq üçün lazım olan ötürmə qabiliyyəti/gecikmə müddətinin qazanclarını, determinizm tələlərini (xüsusiyyətlərin aşkarlanması, heterojen nüvələr, SIGILL riski, uyğunlaşma, icra yollarının qarışdırılması), sabit zaman xüsusiyyətlərini və əməliyyat təminatlarını müzakirə edin.

# Xülasə

İsteğe bağlı `SM3` (`SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`) və Hyperledger (Hyperledger, Hyperledger) ifşa edən ARMv8-A cihazları `SM4EKEY`) təlimat dəstləri GM/T hashını sürətləndirə və şifrə primitivlərini əhəmiyyətli dərəcədə bloklaya bilər. Bununla belə, deterministik blokçeyn icrası xüsusiyyət aşkarlanması, geri qaytarma pariteti və daimi davranış üzərində ciddi nəzarət tələb edir. Aşağıdakı təlimat iki icra strategiyasının necə müqayisə edildiyini və Iroha yığınının nəyi tətbiq etməli olduğunu əhatə edir.

# İcra Müqayisəsi| Aspekt | ARMv8 Intrinsics (AArch64 Inline ASM/`core::arch::aarch64`) | Saf Rust (bit dilimlənmiş / masasız) |
|--------|-------------------------------------------------------------|------------------------------------------------|
| Məhsuldarlıq | Apple M-seriyası və Neoverse V1-də hər nüvə üçün 3–5 × daha sürətli SM3 hashing və 8 × daha sürətli SM4 ECB/CTR; yaddaşa bağlı olduqda daha daralır. | Skaler ALU ilə bağlanmış və fırlanan ilkin ötürmə qabiliyyəti; bəzən `aarch64` SHA genişlənmələrindən (tərtibçinin avtomatik vektorlaşdırılması vasitəsilə) faydalanır, lakin adətən NEON-dan oxşar 3–8× boşluqla geri qalır. |
| Gecikmə | M2-də intrinsiklərlə tək bloklu gecikmə ~30-40ns; sistem zənglərində qısa mesaj hashing və kiçik blok şifrələməsi uyğun gəlir. | blok başına 90-120ns; Rəqabətli qalmaq, təlimat önbelleğinin təzyiqini artırmaq üçün açılmağı tələb edə bilər. |
| Kod ölçüsü | İkili kod yolları (intrinsics + skaler) və iş vaxtı qadağasını tələb edir; `cfg(target_feature)` istifadə edərkən daxili yol yığcam. | Tək yol; əl cədvəli cədvəllərinə görə bir qədər böyükdür, lakin keçid məntiqi yoxdur. |
| Determinizm | Deterministik nəticə əldə etmək üçün iş vaxtı göndərilməsini kilidləməlidir, çarpaz cizgi funksiyalarının yoxlanılması yarışlarından qaçınmalı və heterojen nüvələr fərqli olarsa (məsələn, big.LITTLE) CPU yaxınlığını təyin etməlidir. | Varsayılan olaraq deterministik; iş vaxtı xüsusiyyətinin aşkarlanması yoxdur. |
| Daimi duruş | Avadanlıq bölməsi əsas dövrələr üçün sabit vaxtdır, lakin sarğı arxaya düşəndə ​​və ya masaları qarışdırarkən gizli asılı seçimdən qaçmalıdır. | Rustda tam idarə olunur; düzgün kodlaşdırıldıqda tikinti (bit dilimləmə) ilə təmin edilən sabit vaxt. |
| Daşınma | `aarch64` + əlavə funksiyalar tələb edir; x86_64 və RISC-V avtomatik olaraq geri düşür. | Hər yerdə işləyir; performans kompilyatorun optimallaşdırılmasından asılıdır. |

# Runtime Dispatch Pitfalls

1. **Qeyri-deterministik xüsusiyyətin yoxlanılması**
   - Problem: `is_aarch64_feature_detected!("sm4")`-in heterojen böyük.LITTLE SoC-lərdə yoxlanması hər nüvəyə fərqli cavablar verə bilər və çarpaz iş oğurluğu bir blok daxilində yolları qarışdıra bilər.
   - Azaldılma: qovşağın işə salınması zamanı dəqiq bir dəfə aparat imkanlarını ələ keçirin, `OnceLock` vasitəsilə yayımlayın və VM və ya kripto qutularında sürətləndirilmiş ləpələri icra edərkən CPU yaxınlığı ilə cütləşdirin. Konsensus baxımından kritik işə başladıqdan sonra heç vaxt xüsusiyyət bayraqları üzərində şaxələnməyin.

2. **Replikalar üzrə qarışıq dəqiqlik**
   - Problem: müxtəlif kompilyatorlarla qurulmuş qovşaqlar daxili əlçatanlıq (`target_feature=+sm4` kompilyasiya vaxtını aktivləşdirmə və icra müddətinin aşkarlanması) ilə razılaşa bilməz. İcra müxtəlif kod yollarından keçirsə, mikro-arxitektura vaxtı güc əsaslı geri çəkilmələrə və ya sürət məhdudlaşdırıcılarına sıza bilər.
   - Azaldılması: açıq-aydın `RUSTFLAGS`/`CARGO_CFG_TARGET_FEATURE` ilə kanonik qurma profillərini paylayın, deterministik ehtiyat sifarişini tələb edin (məsələn, konfiqurasiya avadanlığı təmin etmədikcə skalara üstünlük verin) və təsdiq üçün manifestlərə konfiqurasiya heşini daxil edin.3. **Apple vs Linux-da təlimatın mövcudluğu**
   - Problem: Apple SM4 təlimatlarını yalnız ən yeni silikon və OS buraxılışlarında ifşa edir; Linux distrosları ixrac təsdiqini gözləyən ləpələri maskalamaq üçün yamaq edə bilər. Mühafizəsiz intrinsiklərə güvənmək SIGILL-lərə səbəb olur.
   - Azaldılma: `std::arch::is_aarch64_feature_detected!` vasitəsilə qapı, tüstü testlərində `SIGILL`-i tutun və çatışmayan intrinsikləri gözlənilən geriləmə (hələ deterministik) kimi müalicə edin.

4. **Paralel parçalama və yaddaş sifarişi**
   - Problem: sürətləndirilmiş ləpələr tez-tez hər iterasiyada bir neçə bloku emal edir; düzülməmiş girişləri olan NEON yüklərindən/mağazalarından istifadə Norito-serializə edilmiş buferlərlə qidalandıqda xətaya səbəb ola bilər və ya aydın düzülmə düzəlişləri tələb edə bilər.
   - Azaldılması: bloka uyğunlaşdırılmış ayırmaları (məsələn, `SM4_BLOCK_SIZE` `aligned_alloc` sarğıları vasitəsilə çoxaltmaq) saxlayın, sazlama quruluşlarında düzülməni təsdiqləyin və yanlış uyğunlaşdırıldıqda skalyar vəziyyətə qayıdın.

5. **Təlimat önbelleğinin zəhərlənməsi hücumları**
   - Problem: konsensus rəqibləri daha zəif nüvələrdə kiçik I-keş xətlərini qıran iş yükləri yarada, sürətləndirilmiş və skalyar yollar arasında gecikmə fərqini genişləndirə bilər.
   - Azaltma: planlaşdırmanı deterministik yığın ölçülərinə, gözlənilməz budaqlanmanın qarşısını almaq üçün yastıq döngələrinə uyğunlaşdırın və tolerantlıq pəncərələrində titrəmələrin qalmasını təmin etmək üçün mikrobench reqressiya testlərini daxil edin.

# Deterministik Yerləşdirmə Tövsiyələri

- **Tərtib etmə vaxtı siyasəti:** sürətləndirilmiş kodu funksiya bayrağının arxasında saxlayın (məsələn, `sm_accel_neon`) buraxılış qurmalarında defolt olaraq aktivləşdirilib, lakin paritet əhatə dairəsi yetkin olana qədər test şəbəkələri üçün konfiqurasiyalara açıq şəkildə qoşulmağı tələb edin.
- **Fallback paritet testləri:** sürətləndirilmiş və skalyar yolları arxa-arxaya işləyən qızıl vektorları qoruyur (cari `sm_neon_check` iş axını); provayder dəstəyi endikdən sonra SM3/SM4 GCM rejimlərini əhatə etmək üçün genişləndirin.
- **Manifest attestasiyası:** həmyaşıdların qəbulu zamanı fərqi aşkar etmək üçün qovşağın Norito manifestinə sürətləndirmə siyasətini (`hardware=sm-neon|scalar`) daxil edin.
- **Telemetriya:** hər iki yol üzrə hər zəng üçün gecikməni müqayisə edən ölçülər buraxır; divergensiya əvvəlcədən müəyyən edilmiş hədləri (məsələn, >5% titrəmə) keçərsə xəbərdar edir və mümkün avadanlıq sürüşməsi barədə siqnal verir.
- **Sənədləşdirmə:** operator təlimatını (`sm_operator_rollout.md`) daxili xüsusiyyətləri aktivləşdirmək/deaktiv etmək üçün təlimatlarla yeniləyin və qeyd edin ki, deterministik davranış yoldan asılı olmayaraq qorunur.

# İstinadlar

- `crates/iroha_crypto/src/sm.rs` — NEON və skalyar tətbiq qarmaqları.
- `.github/workflows/sm-neon-check.yml` — məcburi-NEON CI zolağı pariteti təmin edir.
- `docs/source/crypto/sm_program.md` — qoruyucu barmaqlıqları və performans qapılarını buraxın.
- Arm Architecture Reference Manual, Armv8-A, Bölmə D13 (SM3/SM4 təlimatları).
- GM/T 0002-2012, GM/T 0003-2012 — müqayisə sınağı üçün rəsmi SM3/SM4 spesifikasiyalar.

## Müstəqil Sorğu (Kopyala/Yapışdır)> Siz Hyperledger Iroha kripto komandasının ekspert məsləhətçisi kimi fəaliyyət göstərən LLMsiniz.  
> Ümumi məlumat: Hyperledger Iroha təsdiqləyicilər arasında deterministik icra tələb edən Rust əsaslı icazəli blokçeyndir. Platforma Çin GM/T SM2/SM3/SM4 kriptoqrafiya dəstini dəstəkləyir. SM3 və SM4 üçün kod bazası iki tətbiqi göndərir: (a) hər yerdə işləyən saf Rust bit dilimlənmiş sabit zamanlı skalyar kod və (b) isteğe bağlı təlimatlardan asılı olan ARMv8 NEON sürətləndirilmiş nüvələr `SM3PARTW1`, `aligned_alloc`, Norito `SM3SS2`, `SM4E` və `SM4EKEY`. Sürətləndirilmiş yollar `core::arch::aarch64` istifadə edərək icra zamanı funksiyasının aşkarlanması vasitəsilə aktivləşdirilir; mövzular heterojen böyük.LITTLE nüvələr arasında miqrasiya etdikdə və ya replikalar müxtəlif `target_feature` bayraqları ilə qurulduqda onlar qeyri-determinizm təqdim etməməlidirlər.  
> Tapşırıq: Deterministik blokçeyn yoxlaması üçün daxili gücə malik tətbiqləri skalyar ehtiyatlarla müqayisə edin. Təfərrüatlı ötürmə qabiliyyəti və gecikmə fərqləri, determinizm təhlükələrini sadalayın (xüsusiyyətlərin aşkarlanması, heterojen nüvələr, SIGILL davranışı, düzülmə, qarışıq icra yolları), sabit vaxt vəziyyətini şərh edin və bütün təsdiqləyici proqramlarda belə qalsa da, bütün təsdiqləyici proqramlarda qorunma tədbirləri (sınaq strategiyası, manifest/attestasiya sahələri, telemetriya, operator sənədləri) tövsiyə edin.