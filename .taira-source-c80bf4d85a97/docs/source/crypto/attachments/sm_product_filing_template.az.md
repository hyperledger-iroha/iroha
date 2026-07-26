---
lang: az
direction: ltr
source: docs/source/crypto/attachments/sm_product_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7116d28e32d8bd77434edd6767427cc3d2ae0624f4de132b1d0cec3c7d44b86
source_last_modified: "2025-12-29T18:16:35.938246+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 Məhsul Faylı (开发备案) Şablonu
% Hyperledger Iroha Uyğunluq İşçi Qrupu
% 2026-05-06

# Təlimat

Bir əyalətə *məhsul inkişafı sənədi* təqdim edərkən bu şablondan istifadə edin
və ya bələdiyyə Dövlət Kriptoqrafiya Administrasiyasının (SCA) ofisinə müraciət etməzdən əvvəl
SM-in effektiv ikili faylları və ya materik Çin daxilindən mənbə artefaktları. dəyişdirin
layihəyə aid təfərrüatları olan yer tutucular, doldurulmuş formanı PDF olaraq ixrac edin
tələb olunur və yoxlama siyahısında istinad edilən artefaktları əlavə edin.

# 1. Ərizəçi və Məhsul Xülasəsi

| Sahə | Dəyər |
|-------|-------|
| Təşkilatın adı | {{ TƏŞKİLAT }} |
| Qeydiyyat ünvanı | {{ ÜNVAN }} |
| Qanuni nümayəndə | {{ HÜQUQİ_REP }} |
| Əsas əlaqə (ad / başlıq / e-poçt / telefon) | {{ ƏLAQƏ }} |
| Məhsulun adı | Hyperledger Iroha {{ RELEASE_NAME }} |
| Məhsul versiyası / quruluş ID | {{ VERSİYA }} |
| Sənədləşdirmə növü | Məhsul inkişafı (开发备案) |
| Müraciət tarixi | {{ YYYY-AA-GG }} |

# 2. Kriptoqrafiyadan İstifadə Baxışı

- Dəstəklənən alqoritmlər: `SM2`, `SM3`, `SM4` (aşağıda istifadə matrisini təqdim edin).
- İstifadə konteksti:
  | Alqoritm | Komponent | Məqsəd | Deterministik təminatlar |
  |----------|-----------|---------|--------------------------|
  | SM2 | {{ KOMPONENT }} | {{ MƏQSƏD }} | RFC6979 + kanonik r∥s tətbiqi |
  | SM3 | {{ KOMPONENT }} | {{ MƏQSƏD }} | `Sm3Digest` | vasitəsilə deterministik hashing
  | SM4 | {{ KOMPONENT }} | {{ MƏQSƏD }} | AEAD (GCM/CCM) tətbiq edilməmiş siyasət |
- Quraşdırmada qeyri-SM alqoritmləri: {{ OTHER_ALGORITHMS }} (tamlıq üçün).

# 3. İnkişaf və Təchizat Zəncirinə Nəzarət

- Mənbə kodu deposu: {{ REPOSITORY_URL }}
- Deterministik qurma təlimatları:
  1. `git clone {{ REPOSITORY_URL }} && git checkout {{ COMMIT_SHA }}`
  2. `cargo build --workspace --locked --release --features "sm sm-ffi-openssl"` (lazım olduqda tənzimləyin).
  3. `cargo auditable` / CycloneDX (`{{ SBOM_PATH }}`) vasitəsilə yaradılan SBOM.
- Davamlı inteqrasiya mühitinin xülasəsi:
  | Maddə | Dəyər |
  |------|-------|
  | OS / versiya qurmaq | {{ BUILD_OS }} |
  | Kompilyator alətlər silsiləsi | {{ TALİMATLAR ZİNCİRİ }} |
  | OpenSSL / Tongsuo mənbəyi | {{ AÇIQ_MƏNBƏ }} |
  | Təkrarlanma qabiliyyətinə nəzarət məbləği | {{ YAXŞI CƏMİYYƏ }} |

# 4. Açar İdarəetmə və Təhlükəsizlik

- Defolt aktivləşdirilmiş SM xüsusiyyətləri: {{ DEFAULTS }} (məsələn, yalnız doğrulama üçün).
- İmza üçün tələb olunan konfiqurasiya bayraqları: {{ CONFIG_FLAGS }}.
- Əsas nəzarət yanaşması:
  | Maddə | Təfərrüatlar |
  |------|---------|
  | Açar yaratma vasitəsi | {{ KEY_TOOL }} |
  | Saxlama mühiti | {{ STORAGE_ORTA }} |
  | Yedəkləmə siyasəti | {{ YEDEK ALMA_SİYASƏTİ }} |
  | Giriş nəzarətləri | {{ ACCESS_CONTROLS }} |
- Hadisə ilə bağlı cavab əlaqəsi (24/7):
  | Rol | Adı | Telefon | E-poçt |
  |------|------|-------|-------|
  | Kripto aparıcı | {{ ADI }} | {{ TELEFON }} | {{ EMAIL }} |
  | Platforma əməliyyatları | {{ ADI }} | {{ TELEFON }} | {{ EMAIL }} |
  | Hüquqi əlaqə | {{ ADI }} | {{ TELEFON }} | {{ EMAIL }} |

# 5. Qoşmaların Yoxlama Siyahısı- [ ] Mənbə kodu snapshot (`{{ SOURCE_ARCHIVE }}`) və hash.
- [ ] Deterministik quruluş skripti / təkrarlanma qeydləri.
- [ ] SBOM (`{{ SBOM_PATH }}`) və asılılıq manifest (`Cargo.lock` barmaq izi).
- [ ] Deterministik test transkriptləri (`scripts/sm_openssl_smoke.sh`, `cargo test -p iroha_crypto sm`).
- [ ] SM müşahidə qabiliyyətini nümayiş etdirən telemetriya tablosunun ixracı.
- [ ] İxrac-nəzarət bəyanatı (ayrıca şablona bax).
- [ ] Audit hesabatları və ya üçüncü tərəf qiymətləndirmələri (artıq tamamlanıbsa).

№ 6. Ərizəçinin Bəyannaməsi

> Yuxarıdakı məlumatların doğru olduğunu, açıqlandığını təsdiq edirəm
> kriptoqrafik funksionallıq ÇXR-in müvafiq qanun və qaydalarına uyğundur,
> və təşkilatın ən azı təqdim edilmiş artefaktları saxlayacağını
> üç il.

- İmza (qanuni nümayəndə): ________________________
- Tarix: ______________________