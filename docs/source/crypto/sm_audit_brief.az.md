---
lang: az
direction: ltr
source: docs/source/crypto/sm_audit_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9cda4648f0af7f89022e9d9f4ea243bc22685d9356927bbf1417c77b2057d872
source_last_modified: "2025-12-29T18:16:35.940439+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 Xarici Audit Qısacası
% Iroha Kripto İşçi Qrupu
% 2026-01-30

# Baxış

Bu qısa məlumat üçün tələb olunan mühəndislik və uyğunluq kontekstini birləşdirir
Iroha-in SM2/SM3/SM4 aktivləşdirilməsinin müstəqil nəzərdən keçirilməsi. O, audit qruplarını hədəfləyir
Rust kriptoqrafiyası təcrübəsi və Çin Milli ilə tanışlıq
Kriptoqrafiya standartları. Gözlənilən nəticə, əhatə edən yazılı hesabatdır
icra riskləri, uyğunluq boşluqları və prioritetləşdirilmiş düzəliş təlimatları
SM-in təqdimatından qabaq baxışdan istehsala keçir.

# Proqram Snapshot

- ** Buraxılış əhatəsi:** Iroha 2/3 paylaşılan kod bazası, deterministik yoxlama
  qovşaqlar və SDK-lar arasında yollar, konfiqurasiya qoruyucusu arxasında mövcud imza.
- **Cari mərhələ:** Rust ilə SM-P3.2 (OpenSSL/Tongsuo backend inteqrasiyası)
  yoxlama və simmetrik istifadə halları üçün artıq göndərilən tətbiqlər.
- **Hədəf qərar tarixi:** 30-04-2026 (audit nəticələrinə görə get/no-go
  validator qurmalarında SM imzalamağa imkan verir).
- **İzlənilən əsas risklər:** üçüncü tərəfdən asılılıq şəcərəsi, deterministik
  qarışıq aparat altında davranış, operator uyğunluq hazırlığı.

# Kod və Quraşdırma İstinadları

- `crates/iroha_crypto/src/sm.rs` — Rust tətbiqləri və əlavə OpenSSL
  bağlamalar (`sm-ffi-openssl` xüsusiyyəti).
- `crates/ivm/tests/sm_syscalls.rs` — IVM hashing üçün sistem əhatəsi,
  yoxlama və simmetrik rejimlər.
- `crates/iroha_data_model/tests/sm_norito_roundtrip.rs` — Norito faydalı yük
  SM artefaktları üçün gediş-gəliş.
- `docs/source/crypto/sm_program.md` — proqram tarixçəsi, asılılıq auditi və
  sürüşmə qoruyucuları.
- `docs/source/crypto/sm_operator_rollout.md` — operatorla bağlı imkan və
  geri qaytarma prosedurları.
- `docs/source/crypto/sm_compliance_brief.md` — tənzimləyici xülasə və ixrac
  mülahizələr.
- `scripts/sm_openssl_smoke.sh` / `crates/iroha_crypto/tests/sm_openssl_smoke.rs`
  — OpenSSL dəstəkli axınlar üçün deterministik tüstü kəməri.
- `fuzz/sm_*` corpora — SM3/SM4 primitivlərini əhatə edən RustCrypto əsaslı fuzz toxumları.

# Tələb olunan Audit Həcmi1. **Spesifikasiyaya uyğunluq**
   - SM2 imza yoxlanışını, ZA hesablamasını və kanonik təsdiqləyin
     kodlaşdırma davranışı.
   - SM3/SM4 primitivlərinin GM/T 0002-2012 və GM/T 0007-2012-yə uyğun olduğunu təsdiqləyin,
     sayğac rejimi invariantları və IV idarəetmə də daxil olmaqla.
2. **Determinizm və daimi zaman təminatı**
   - Qovşağın icrası üçün budaqlanma, cədvəl axtarışları və aparat göndərilməsini nəzərdən keçirin
     CPU ailələri arasında deterministik olaraq qalır.
   - Şəxsi açar əməliyyatları üçün daimi tələbləri qiymətləndirin və təsdiqləyin
     OpenSSL/Tongsuo yolları sabit zaman semantikasını saxlayır.
3. **Yan kanal və nasazlıq təhlili**
   - Həm Rust, həm də vaxt, keş və güc yan kanal risklərini yoxlayın
     FFI tərəfindən dəstəklənən kod yolları.
   - İmzanın yoxlanılması üçün xətaların idarə edilməsini və xətaların yayılmasını qiymətləndirin və
     təsdiqlənmiş şifrələmə uğursuzluqları.
4. **İnşaat, asılılıq və təchizat zəncirinin nəzərdən keçirilməsi**
   - OpenSSL/Tongsuo artefaktlarının təkrarlana bilən quruluşlarını və mənşəyini təsdiqləyin.
   - Asılılıq ağacının lisenziyalaşdırılması və auditin əhatə dairəsini nəzərdən keçirin.
5. **Sınaq və doğrulama qoşqularının tənqidi**
   - Deterministik tüstü testlərini, tüylü qoşquları və Norito qurğularını qiymətləndirin.
   - Əlavə əhatə dairəsini tövsiyə edin (məsələn, diferensial sınaq, əmlaka əsaslanan
     sübutlar) əgər boşluqlar qalırsa.
6. **Uyğunluq və operator rəhbərliyinin təsdiqi**
   - Göndərilən sənədləri qanuni tələblərə və gözlənilənlərə uyğun yoxlayın
     operator nəzarəti.

# Çatdırılma və Logistika

- **Başlama:** 24-02-2026 (virtual, 90 dəqiqə).
- **Müsahibələr:** Crypto WG, IVM baxıcıları, platforma əməliyyatları (lazım olduqda).
- **Artefakta giriş:** yalnız oxumaq üçün nəzərdə tutulmuş depo güzgüsü, CI boru kəməri qeydləri, armatur
  çıxışlar və asılılıq SBOM-ları (CycloneDX).
- **Aralıq yeniləmələr:** həftəlik yazılı status + risk qeydləri.
- **Yekun çatdırılmalar (2026-04-15 tarixinə qədər):**
  - Risk reytinqi ilə icra xülasəsi.
  - Ətraflı tapıntılar (məsələ üzrə: təsir, ehtimal, kod istinadları,
    təmirə dair təlimat).
  - Yenidən sınaq/yoxlama planı.
  - Determinizm, daimi duruş və uyğunluq uyğunluğu haqqında bəyanat.

## Nişan statusu

| Satıcı | Status | Başlanğıc | Sahə Pəncərəsi | Qeydlər |
|--------|--------|----------|--------------|-------|
| Bitlərin izi (CN təcrübəsi) | İcra edilmiş işlərin hesabatı 2026-02-21 | 24-02-2026 | 2026-02-24-2026-03-22 | Çatdırılma tarixi 2026-04-15; Hui Zhang, mühəndislik həmkarı kimi Alexey M. ilə aparıcı nişandır. Həftəlik status zəngi çərşənbə günləri 09:00UTC. |
| NCC Group (APAC) | Fövqəladə hallar üçün yer ayrılmışdır | N/A (gözləmədədir) | Müvəqqəti 2026-05-06-2026-05-31 | Yalnız yüksək riskli tapıntılar ikinci keçid tələb etdikdə aktivləşdirmə; hazırlıq Priya N. (Təhlükəsizlik) və NCC Group nişan masası tərəfindən təsdiqlənib 2026-02-22. |

# Qoşmalar Yayım Paketinə Daxildir- `docs/source/crypto/sm_program.md`
- `docs/source/crypto/sm_operator_rollout.md`
- `docs/source/crypto/sm_compliance_brief.md`
- `docs/source/crypto/sm_lock_refresh_plan.md`
- `docs/source/crypto/sm_rust_vector_check.md`
- `docs/source/crypto/attachments/sm_iroha_crypto_tree.txt` — `cargo tree -p iroha_crypto --no-default-features --features "sm sm-ffi-openssl"` snapshot.
- `docs/source/crypto/attachments/sm_iroha_crypto_metadata.json` — `iroha_crypto` qutusu üçün `cargo metadata` ixracı (bağlı asılılıq qrafiki).
- `docs/source/crypto/attachments/sm_openssl_smoke.log` — ən son `scripts/sm_openssl_smoke.sh` işləməsi (provayder dəstəyi olmadıqda SM2/SM4 yollarını atlayır).
- `docs/source/crypto/attachments/sm_openssl_provenance.md` — yerli alət dəstinin mənşəyi (pkg-config/OpenSSL versiya qeydləri).
- Qeyri-səlis korpus manifest (`fuzz/sm_corpus_manifest.json`).

> **Ətraf mühit xəbərdarlığı:** Hazırkı inkişaf snapshotı satıcı tərəfindən təqdim edilmiş OpenSSL 3.x alətlər silsilməsindən istifadə edir (`openssl` sandıq `vendored` xüsusiyyəti), lakin macOS-da SM3/SM4 CPU intrinsics yoxdur və defolt provayder tüstüdən çıxmır, buna görə də açıq SMPS4-SMS4- əhatə dairəsi və Əlavə Nümunə SM2 təhlili. İş yerindən asılılıq dövrü (`sorafs_manifest ↔ sorafs_car`) həmçinin `cargo check` xətasını yaydıqdan sonra köməkçi skripti işə keçməyə məcbur edir. Xarici auditdən əvvəl tam paritet əldə etmək üçün paketi Linux buraxılış qurma mühitində (SM4 ilə OpenSSL/Tongsuo aktiv və dövriyyəsiz) yenidən işə salın.

# Namizəd audit tərəfdaşları və əhatə dairəsi

| Firma | Müvafiq təcrübə | Tipik əhatə dairəsi və çatdırılma | Qeydlər |
|------|---------------------|------------------------------|-------|
| Bitlərin izi (CN kriptoqrafiya təcrübəsi) | Rust kodu baxışları (`ring`, zkVMs), mobil ödəniş yığınları üçün əvvəlki GM/T qiymətləndirmələri. | Xüsusi uyğunluq fərqi (GM/T 0002/3/4), Rust + OpenSSL yollarının daimi nəzərdən keçirilməsi, diferensial tündləşmə, təchizat zəncirinin nəzərdən keçirilməsi, remediasiya yol xəritəsi. | Artıq nişanlı; gələcək yeniləmə dövrlərini planlaşdırarkən tamlıq üçün saxlanılan cədvəl. |
| NCC Group APAC | Avadanlıq/SOC + Rust kriptoqrafiya qırmızı qrupları, RustCrypto primitivləri və ödəniş HSM körpüləri haqqında dərc edilmiş rəylər. | Rust + JNI/FFI bağlamalarının vahid qiymətləndirilməsi, deterministik siyasətin təsdiqi, perf/telemetriya qapısının nəzərdən keçirilməsi, operatorun oyun kitabının gedişi. | Fövqəladə hal kimi qorunur; həmçinin Çin tənzimləyiciləri üçün ikidilli hesabat təqdim edə bilər. |
| Kudelski Təhlükəsizlik (Blockchain və kripto komandası) | Rust-da həyata keçirilən Halo2, Mina, zkSync, xüsusi imza sxemlərinin auditləri. | Elliptik əyri düzgünlüyünə, transkript bütövlüyünə, aparatın sürətləndirilməsi üçün təhlükə modelləşdirməsinə və CI/çıxarma sübutlarına diqqət yetirin. | Aparat sürətləndirilməsi (SM-5a) və FASTPQ-dan SM-ə qarşılıqlı əlaqə haqqında ikinci rəylər üçün faydalıdır. |
| Ən az Səlahiyyət | Rust əsaslı blokçeynlər (Filecoin, Polkadot) üçün kriptoqrafik protokol auditləri, təkrar istehsal konsaltinqləri. | Deterministik quruluş yoxlanışı, Norito kodek yoxlanışı, uyğunluq sübutlarının çarpaz yoxlanışı, operator rabitəsinin nəzərdən keçirilməsi. | Tənzimləyicilər kodun nəzərdən keçirilməsindən kənar müstəqil yoxlama tələb etdikdə şəffaflıq/audit-hesabat nəticələri üçün çox uyğundur. |

Bütün tapşırıqlar yuxarıda sadalanan eyni artefakt paketini və firmadan asılı olaraq aşağıdakı əlavə əlavələri tələb edir:- **Spec uyğunluğu və deterministik davranış:** SM2 ZA törəməsinin, SM3 dolğunluğunun, SM4 dairəvi funksiyalarının və `sm_accel` iş vaxtı göndərmə qapısının sətir-sətir yoxlanılması, sürətlənmənin semantikanı heç vaxt dəyişməməsini təmin edir.
- **Yanlı kanal və FFI baxışı:** Daimi vaxt tələblərinin, təhlükəli kod bloklarının və OpenSSL/Tongsuo körpü qatlarının, o cümlədən Rust yoluna qarşı fərqli sınaqların yoxlanılması.
- **CI / təchizat zəncirinin təsdiqi:** `sm_interop_matrix`, `sm_openssl_smoke` və `sm_perf` qoşqularının SBOM/SLSA sertifikatları ilə birlikdə reproduksiyası, beləliklə, audit nəticələri sübutların açıqlanması üçün birbaşa əlaqələndirilə bilər.
- **Operatorla üz-üzə olan girov:** Sənədlərdə vəd edilən yumşaldıcı tədbirlərin texniki cəhətdən tətbiq oluna biləcəyini təsdiqləmək üçün `sm_operator_rollout.md`, uyğunluq sənədləşdirmə şablonları və telemetriya tablosunu çarpaz yoxlayın.

Gələcək auditlərin əhatə dairəsini müəyyənləşdirərkən, təchizatçının güclü tərəflərini xüsusi yol xəritəsinin mərhələləri ilə uyğunlaşdırmaq üçün bu cədvəldən yenidən istifadə edin (məsələn, aparat/mükəmməl ağır buraxılışlar üçün Kudelski, dil/iş vaxtının düzgünlüyü üçün Bitlərin İzi və təkrarlana bilən tikinti zəmanətləri üçün Ən az Səlahiyyətliliyə üstünlük verin).

# Əlaqə Nöqtələri

- **Texniki sahibi:** Crypto WG rəhbəri (Aleksey M., `alexey@iroha.tech`)
- **Proqram meneceri:** Platforma Əməliyyatları koordinatoru (Sarah K.,
  `sarah@iroha.tech`)
- **Təhlükəsizlik əlaqəsi:** Təhlükəsizlik Mühəndisliyi (Priya N., `security@iroha.tech`)
- **Sənədləşdirmə əlaqəsi:** Sənədlər/DevRel rəhbəri (Cəmilə R.,
  `docs@iroha.tech`)