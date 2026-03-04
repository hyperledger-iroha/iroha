---
lang: az
direction: ltr
source: docs/source/crypto/sm_audit_vendor_landscape.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f39199767280be0fdd582301cdc3e8929497cf372a96f9f300e718f827000a7
source_last_modified: "2025-12-29T18:16:35.941305+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM Audit Satıcı Landşaftı
% Iroha Kripto İşçi Qrupu
% 2026-02-12

# Baxış

Kripto İşçi Qrupuna müstəqil rəyçilərdən ibarət daimi bir sıra lazımdır
həm Rust kriptoqrafiyasını, həm də Çin GM/T (SM2/SM3/SM4) standartlarını başa düşür.
Bu qeyd müvafiq istinadları olan firmaları kataloqlayır və auditi yekunlaşdırır
təklif sorğusu (RFP) dövrlərinin sürətli və sürətli qalması üçün biz adətən tələb edirik
ardıcıl.

# Namizəd Firmalar

## Bitlərin İzi (CN Kriptoqrafiya Təcrübəsi)

- Sənədləşdirilmiş tapşırıqlar: Ant Group-un Tongsuo-nun 2023 təhlükəsizlik baxışı
  (SM-in effektiv OpenSSL paylanması) və Rust əsaslı təkrar yoxlamalar
  Diem/Libra, Sui və Aptos kimi blokçeynlər.
- Güclü tərəflər: xüsusi Rust kriptoqrafiya komandası, avtomatlaşdırılmış daimi vaxt
  analiz alətləri, deterministik icranı təsdiqləyən təcrübə və aparat
  göndərmə siyasəti.
- Iroha üçün uyğundur: cari SM audit SOW-u genişləndirə və ya müstəqil icra edə bilər
  təkrar testlər; Norito qurğuları və IVM sistemi ilə rahat işləmə
  səthlər.

## NCC Group (APAC Kriptoqrafiya Xidmətləri)

- Sənədləşdirilmiş tapşırıqlar: regional ödəniş üçün gm/T (SM) kodu imtahanları
  şəbəkələr və HSM satıcıları; Parity Substrate, Polkadot üçün əvvəlki Rust rəyləri,
  və Tərəzi komponentləri.
- Güclü tərəflər: ikidilli hesabatlı böyük APAC skamyası, birləşmək bacarığı
  dərin kodun nəzərdən keçirilməsi ilə uyğunluq tərzi proses yoxlamaları.
- Iroha üçün uyğundur: ikinci rəy qiymətləndirmələri və ya idarəetməyə əsaslanan qiymətləndirmələr üçün idealdır
  Trail of Bits tapıntıları ilə yanaşı doğrulama.

## SECBIT Labs (Pekin)

- Sənədləşdirilmiş tapşırıqlar: açıq mənbəli `libsm` Rust qutusunun baxıcıları
  Nervos CKB və CITA tərəfindən istifadə olunur; Nervos, Muta və üçün Guomi effektivliyini yoxladı
  FISCO BCOS Rust komponentləri ikidilli çatdırılma ilə.
- Güclü tərəflər: Rustda SM primitivlərini aktiv şəkildə göndərən mühəndislər, güclü
  əmlakın sınaqdan keçirilməsi imkanları, daxili uyğunluqla dərindən tanışlıq
  tələblər.
- Iroha üçün uyğundur: müqayisəli məlumat verə bilən rəyçilərə ehtiyac duyduğumuz zaman dəyərlidir
  tapıntılarla yanaşı test vektorları və icra təlimatı.

## SlowMist Təhlükəsizlik (Chengdu)

- Sənədləşdirilmiş tapşırıqlar: Substrat/Polkadot Rust təhlükəsizlik rəyləri daxil olmaqla
  Çin operatorları üçün Guomi çəngəlləri; SM2/SM3/SM4 pul kisəsinin müntəzəm qiymətləndirilməsi
  və birjalar tərəfindən istifadə edilən körpü kodu.
- Güclü tərəflər: blokçeyn mərkəzli audit təcrübəsi, insidentlərə inteqrasiya olunmuş reaksiya,
  əsas protokol kodunu və operator alətlərini əhatə edən təlimat.
- Iroha üçün uyğundur: SDK paritetini və əməliyyat əlaqə nöqtələrini yoxlamaq üçün faydalıdır
  əsas qutulara əlavə olaraq.

## Chaitin Tech (QAX 404 Təhlükəsizlik Laboratoriyası)- Sənədləşdirilmiş tapşırıqlar: GmSSL/Tongsuo hardening və SM2/SM3/-a töhfə verənlər
  Yerli maliyyə institutları üçün SM4 tətbiqi təlimatı; yaradılmışdır
  TLS yığınlarını və kriptoqrafik kitabxanaları əhatə edən Rust audit təcrübəsi.
- Güclü tərəflər: dərin kriptoanaliz fonu, rəsmi yoxlamanı cütləşdirmək bacarığı
  əl ilə nəzərdən keçirilən artefaktlar, uzunmüddətli tənzimləyici əlaqələri.
- Iroha üçün uyğundur: tənzimləyici imzalama və ya rəsmi sübut artefaktları olduqda uyğundur
  standart kodun nəzərdən keçirilməsi hesabatını müşayiət etməlidir.

# Tipik Audit Həcmi və Təqdim olunanlar

- **Spesifikasiyaya uyğunluq:** SM2 ZA hesablamasını, imzasını təsdiq edin
  kanonikləşdirmə, SM3 doldurma/sıxılma və SM4 açar cədvəli və IV idarəsi
  GM/T 0003-2012, GM/T 0004-2012 və GM/T 0002-2012-yə qarşı.
- **Determinizm və daimi davranış:** budaqlanmanı, axtarışı yoxlayın
  təmin etmək üçün masalar və aparat xüsusiyyətləri qapıları (məsələn, NEON, SM4 təlimatları).
  Rust və FFI göndərilməsi dəstəklənən aparatlarda deterministik olaraq qalır.
- **FFI və provayder inteqrasiyası:** OpenSSL/Tongsuo bağlamalarını nəzərdən keçirin,
  PKCS#11/HSM adapterləri və konsensus təhlükəsizliyi üçün səhvlərin yayılması yolları.
- **Sınaq və qurğuların əhatə dairəsi:** tüylü qoşquları qiymətləndirin, Norito gediş-gəliş,
  deterministik tüstü testləri və boşluqların olduğu yerlərdə diferensial sınaqları tövsiyə edin
  görünür.
- **Asılılıq və təchizat zəncirinin nəzərdən keçirilməsi:** mənşəyi, satıcını təsdiqləyin
  yamaq siyasəti, SBOM dəqiqliyi və təkrarlana bilən qurma təlimatları.
- **Sənədləşdirmə və əməliyyatlar:** operator runbooks, uyğunluğu təsdiq edin
  brifinqlər, konfiqurasiya defoltları və geri qaytarma prosedurları.
- **Hesabat gözləntiləri:** risk reytinqi ilə icraçı xülasə, ətraflı
  kod istinadları və düzəliş təlimatları, yenidən sınaq planı və
  determinizm zəmanətlərini əhatə edən sertifikatlar.

# Növbəti Addımlar

- RFQ dövrləri zamanı bu satıcı siyahısından istifadə edin; yuxarıdakı əhatə dairəsi yoxlama siyahısını tənzimləyin
  RFP verməzdən əvvəl aktiv SM mərhələ ilə uyğunlaşın.
- `docs/source/crypto/sm_audit_brief.md`-də nişanlanma nəticələrini qeyd edin və
  müqavilələr icra edildikdən sonra `status.md`-də səth statusu yeniləmələri.