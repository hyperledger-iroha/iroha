---
lang: az
direction: ltr
source: docs/source/iroha_2_whitepaper.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a4e8824c128b9f2a34262a5c9bc09f6b2cd790a0561aa083fa18a987accd7004
source_last_modified: "2026-01-22T16:26:46.570053+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2.0

Hyperledger Iroha v2 deterministik, Bizans səhvlərinə dözümlü paylanmış kitabdır və
modul arxitektura, güclü defoltlar və əlçatan API-lər. Platforma Pas qutuları dəsti kimi göndərilir
sifarişli yerləşdirmələrə daxil edilə bilən və ya istehsal blokçeyn şəbəkəsini idarə etmək üçün birlikdə istifadə edilə bilər.

---

## 1. İcmal

Iroha 2, Iroha 1 ilə təqdim olunan dizayn fəlsəfəsini davam etdirir: seçilmiş kolleksiyanı təqdim edir.
imkanları qutudan kənara çıxarır, beləliklə operatorlar böyük miqdarda sifariş yazmadan bir şəbəkədə dayana bilsinlər
kod. v2 buraxılışı icra mühitini, konsensus boru kəmərini və məlumat modelini birləşdirir
vahid vahid iş sahəsi.

v2 xətti öz icazəli və ya konsorsiumunu idarə etmək istəyən təşkilatlar üçün nəzərdə tutulmuşdur
blokçeynlər. Hər bir yerləşdirmə öz konsensus şəbəkəsini idarə edir, müstəqil idarəetməni saxlayır və uyğunlaşdıra bilər
üçüncü tərəflərdən asılı olmadan konfiqurasiya, genezis məlumatları və təkmil ritm. Paylaşılan iş sahəsi
funksiyaları seçərkən çoxlu müstəqil şəbəkələrə eyni kod bazasına qarşı qurmağa imkan verir
onların istifadə hallarına uyğun gələn siyasətlər.

Həm Iroha 2, həm də SORA Nexus (Iroha 3) eyni Iroha Virtual Maşını (IVM) işlədir. Tərtibatçılar Kotodama müəllifi ola bilərlər
bir dəfə müqavilələr bağlayın və onları yenidən tərtib etmədən və ya öz-özünə yerləşdirilən şəbəkələrdə və ya qlobal Nexus kitabçasında yerləşdirin
icra mühitini forking.
### 1.1 Hyperledger ekosistemi ilə əlaqə

Iroha komponentləri digər Hyperledger layihələri ilə işləmək üçün nəzərdə tutulub. Konsensus, məlumat modeli və
Serializasiya qutuları kompozit yığınlarda və ya Fabric, Sawtooth və Besu yerləşdirmələri ilə birlikdə təkrar istifadə edilə bilər.
Norito kodekləri və idarəetmə manifestləri kimi ümumi alətlər interfeysləri ardıcıllıqla saxlamağa kömək edir.
ekosistemi Iroha-ə rəyli defolt tətbiqi təmin etməyə icazə verir.

### 1.2 Müştəri kitabxanaları və SDK-lar

Birinci dərəcəli mobil və veb təcrübələrini təmin etmək üçün layihə saxlanılan SDK-ları dərc edir:

- `IrohaSwift` iOS və macOS müştəriləri üçün Metal/NEON sürətləndirilməsini deterministik geri dönüşlərin arxasında birləşdirir.
- Kaigi qurucuları və Norito köməkçiləri də daxil olmaqla JavaScript və TypeScript proqramları üçün `iroha_js`.
- HTTP, WebSocket və telemetriya dəstəyi ilə Python inteqrasiyaları üçün `iroha_python`.
- Terminalla idarə olunan idarəetmə və skript üçün `iroha_cli`.

dillər və platformalar.

### 1.3 Dizayn prinsipləri- ** Əvvəlcə determinizm:** Hər bir qovşaq eyni kod yollarını icra edir və eyni verildikdə eyni nəticələri verir
  girişlər. SIMD/CUDA/NEON yolları xüsusiyyət qapalıdır və deterministik skalyar tətbiqlərə qayıdır.
- **Tərtib edilə bilən modullar:** Şəbəkə, konsensus, icra, telemetriya və saxlama hər biri xüsusi bir yerdə yaşayır
  sandıqlar ki, daxil edənlər bütün yığını daşımadan alt çoxluqları qəbul edə bilsinlər.
- **Açıq konfiqurasiya:** Davranış düymələri `iroha_config` vasitəsilə üzə çıxır; mühit keçidləridir
  tərtibatçının rahatlığı ilə məhdudlaşır.
- **Təhlükəsiz defoltlar:** Kanonik kodeklər, ciddi göstərici ABI tətbiqi və versiyalı manifestlər
  şəbəkələrarası təkmilləşdirmələr proqnozlaşdırıla bilər.

## 2. Platformanın arxitekturası

### 2.1 Node tərkibi

Iroha qovşağı bir neçə əməkdaşlıq xidmətlərini idarə edir:

- **Torii (`iroha_torii`)** əməliyyatlar, sorğular, axın hadisələri üçün HTTP/WebSocket API-lərini ifşa edir və
  telemetriya (`/v1/...` son nöqtələri).
- **Core (`iroha_core`)** doğrulama, konsensus, icra, idarəetmə və dövlət idarəçiliyini əlaqələndirir.
- **Sumeragi (`iroha_core::sumeragi`)** görünüş dəyişiklikləri ilə NPoS-ə hazır konsensus boru kəmərini tətbiq edir,
  etibarlı yayım məlumatının mövcudluğu və sertifikatlar. baxın
  [Sumeragi konsensus bələdçisi](./sumeragi.md) ətraflı məlumat üçün.
- **Kura (`iroha_core::kura`)** diskdəki kanonik blokları, bərpa yan arabalarını və şahid metadatasını saxlayır.
- **World State View (`iroha_core::state`)** doğrulama üçün istifadə edilən nüfuzlu yaddaşdaxili görüntünü saxlayır
  və sorğular.
- **Iroha Virtual Maşın (`ivm`)** Kotodama bayt kodunu (`.to`) icra edir və göstərici ABI siyasətini tətbiq edir.
- **Norito (`crates/norito`)** hər bir naqilli tip üçün deterministik binar və JSON serializasiyasını təmin edir.
- **Telemetri (`iroha_telemetry`)** Prometheus ölçülərini, strukturlaşdırılmış girişi və axın hadisələrini ixrac edir.
- **P2P (`iroha_p2p`)** dedi-qodu, topologiya və həmyaşıdları arasında təhlükəsiz əlaqələri idarə edir.

### 2.2 Şəbəkə və topologiya

Iroha həmyaşıdları razılaşdırılmış vəziyyətdən əldə edilən sifarişli topologiyanı saxlayır. Hər konsensus turu bir lider seçir,
doğrulama dəsti, proxy quyruq və Set B təsdiqləyiciləri. Əməliyyatlar Norito kodlu mesajlar vasitəsilə qeybət edilir
lider onları bir təklifə bağlamazdan əvvəl. Etibarlı yayım bloklayan və dəstəkləyən zəmanət verir
sübutlar bütün vicdanlı həmyaşıdlarına çatır, hətta şəbəkə çaşqınlığı altında da məlumatların mövcudluğunu təmin edir. Dəyişikliklərə baxın
son tarixlər qaçırıldığı zaman liderlik edir və öhdəçilik sertifikatları hər bir törədilmiş blokun daşımasını təmin edir
bütün həmyaşıdları tərəfindən istifadə olunan kanonik imza dəsti.

### 2.3 Kriptoqrafiya

`iroha_crypto` qutusu əsas idarəetmə, hashing və imza yoxlamasına səlahiyyət verir:- Ed25519 standart təsdiqləyici açar sxemidir.
- Könüllü backendlərə Secp256k1, TC26 GOST, BLS (ümumi attestasiyalar üçün) və ML-DSA köməkçiləri daxildir.
- Axın kanalları Norito axın seanslarını təmin etmək üçün Ed25519 şəxsiyyətlərini Kyber əsaslı HPKE ilə cütləşdirir.
- Bütün hashing rutinləri iş sahəsi ilə deterministik tətbiqlərdən (SHA-2, SHA-3, Blake2, Poseidon2) istifadə edir.
  `docs/source/crypto/dependency_audits.md`-də sənədləşdirilmiş auditlər.

### 2.4 Axın və tətbiq körpüləri

- **Norito axın (`iroha_core::streaming`, `norito::streaming`)** deterministik, şifrələnmiş media təmin edir
  və sessiya görüntüləri, HPKE düymələrinin fırlanması və telemetriya qarmaqları olan məlumat kanalları. Kaigi konfransı və
  məxfi sübut köçürmələri bu zolaqdan istifadə edir.
- **Bağlantı körpüsü (`connect_norito_bridge`)** platforma SDK-larını gücləndirən C ABI səthini ifşa edir
  (Swift, Kotlin/Android) başlıq altında Rust müştərilərini təkrar istifadə edərkən.
- **ISO 20022 körpüsü (`iroha_torii::iso20022_bridge`)** tənzimlənən ödəniş mesajlarını Norito-ə çevirir
  konsensus və ya doğrulamadan yan keçmədən maliyyə iş axınları ilə qarşılıqlı fəaliyyətə imkan verən əməliyyatlar.
- Bütün körpülər deterministik Norito faydalı yükləri qoruyur ki, aşağı axın sistemləri vəziyyət keçidlərini yoxlaya bilsin.

## 3. Məlumat modeli

`iroha_data_model` qutusu bütün kitab obyektlərini, təlimatları, sorğuları və hadisələri müəyyən edir. Əsas məqamlar:

- **Domenlər, hesablar və aktivlər** kanonik I105 hesab ID-lərindən istifadə edir (üstünlük verilir); `name@domain` marşrutlaşdırma olaraq qalır
  açıq şəkildə təqdim edildikdə ləqəb. Metadata deterministikdir (`Metadata` xəritəsi). Rəqəmsal aktivlər sabit nöqtəni dəstəkləyir
  əməliyyatlar; NFT-lər ixtiyari strukturlaşdırılmış metadata daşıyır.
- **Rollar və icazələr** Norito nömrəli tokenlərdən istifadə edir ki, onlar birbaşa icraçı yoxlamaları ilə əlaqələndirirlər.
- **Tetiklər** (vaxt əsaslı, blok əsaslı və ya predikatla idarə olunan) zəncir üzərindən deterministik əməliyyatlar yayır
  icraçı.
- **Hadisələr** Torii vasitəsilə axın və məxfi axınlar və
  idarəetmə tədbirləri.
- **Əməliyyatlar, bloklar və manifestlər** Norito kodlu (`SignedTransaction`, `SignedBlockWire`) ilə
  açıq versiya başlıqları, irəli uzadılan deşifrləməni təmin edir.
- **Fərdiləşdirmə** icraçı məlumat modeli vasitəsilə baş verir: operatorlar xüsusi təlimatları qeydiyyatdan keçirə bilər,
  determinizmi qoruyarkən icazələr və parametrlər.
- **Repozitoriyalar (`RepoInstruction`)** deterministik təkmilləşdirmə planlarını (icraçılar, manifestlər və
  aktivlər) beləliklə, çox mərhələli buraxılışlar idarəetmənin təsdiqi ilə zəncirdə idarə oluna bilər.
- **Konsensus artefaktları**—məs,məsuliyyət sertifikatları və şahid siyahıları—məlumat modelində yerləşir və
  `iroha_core`, Torii və SDK-lar arasında uyğunluğu təmin etmək üçün qızıl testlər vasitəsilə gediş-gəliş.
- **Məxfi qeydlər və hadisələr** qorunan aktiv deskriptorlarını, doğrulayıcı açarları, öhdəlikləri,
  nullifiers və hadisə yükləri (`ConfidentialEvent::{Shielded,Transferred,Unshielded}`) belə məxfi axınlar
  açıq mətn məlumatlarını sızdırmadan yoxlanıla bilər.

## 4. Əməliyyatın həyat dövrü1. **Qəbul:** Torii Torii Norito faydalı yükünü deşifrə edir, imzaları, TTL və ölçü məhdudiyyətlərini yoxlayır, sonra sıraya qoyur
   yerli əməliyyat.
2. **Qeybət:** Əməliyyat topologiya üzrə yayılır; həmyaşıdları hash ilə təkmilləşdirir və qəbulu təkrarlayır
   çeklər.
3. **Seçim:** Cari lider gözlənilən dəstdən əməliyyatları çıxarır və vətəndaşlığı olmayan yoxlama aparır.
4. **Statistik simulyasiya:** Namizəd əməliyyatları IVM və ya müraciət edərək keçici `StateBlock` daxilində icra edilir.
   quraşdırılmış təlimatlar. Münaqişələr və ya qayda pozuntuları qəti şəkildə aradan qaldırılır.
5. **Trigger materializasiyası:** Raundda planlaşdırılan tətiklər daxili əməliyyatlara çevrilir
   və eyni boru kəmərindən istifadə etməklə təsdiq edilmişdir.
6. **Təklifin möhürlənməsi:** Blok limitlərinə çatdıqda və ya fasilələr başa çatdıqda, lider Norito kodlu siqnal verir
   `BlockCreated` mesajı.
7. **Validasiya:** Doğrulama dəstindəki həmyaşıdlar vətəndaşlığı olmayan/vəziyyəti təsdiq edən yoxlamaları yenidən həyata keçirirlər. Uğurlu həmyaşıdları işarə edir
   `BlockSigned` mesajları və onları deterministik kollektor dəstinə yönləndirin.
8. **Öhdəlik:** Kollektor kanonik imza dəstini topladıqdan sonra öhdəlik sertifikatı toplayır,
   `BlockCommitted` yayımlayır və bloku yerli olaraq yekunlaşdırır.
9. **Tətbiq:** Bütün həmyaşıdlar Kürdəki bloku qeyd edir, vəziyyət yeniləmələrini tətbiq edir, telemetriya/hadisələr yayır, təmizləyir
   mempooldan əməliyyatlar həyata keçirin və topologiya rollarını çevirin.

Bərpa yolları çatışmayan blokları təkrar ötürmək üçün deterministik yayımdan istifadə edir və dəyişikliklərə baxış liderliyi fırladır
müddətlər bitdikdə. Yan avtomobillər və telemetriya konsensus nəticələrini mutasiya etmədən diaqnostik fikirlər təqdim edir.

## 5. Ağıllı müqavilələr və icra

Ağıllı müqavilələr Iroha Virtual Maşın (IVM) üzərində işləyir:

- **Kotodama** yüksək səviyyəli `.ko` mənbələrini deterministik `.to` bayt koduna tərtib edir.
- **Pointer ABI tətbiqi** təsdiqlənmiş göstərici növləri vasitəsilə müqavilələrin host yaddaşı ilə qarşılıqlı əlaqəsini təmin edir.
  Syscall səthləri `ivm/docs/syscalls.md`-də təsvir edilmişdir; ABI siyahısı heşlənmiş və versiyalaşdırılmışdır.
- **Syscalls və hosts** kitab vəziyyətinə girişi, trigger planlamasını, məxfi primitivləri, Kaigi mediasını əhatə edir.
  axınlar və deterministik təsadüfilik.
- **Daxili icraçı** aktiv, hesab, icazə,
  və idarəetmə əməliyyatları. Fərdi icraçılar Norito sxemlərinə riayət etməklə təlimat dəstini genişləndirə bilərlər.
- **Məxfi xüsusiyyətlər**, o cümlədən qorunan köçürmələr və yoxlayıcı reyestrlər icraçı vasitəsilə ifşa olunur.
  təlimatlar və Poseidon öhdəlikləri ilə ev sahibləri tərəfindən təsdiq edilmişdir.

## 6. Saxlama və davamlılıq- **Kür blok mağazası** hər bir yekunlaşdırılmış bloku Norito başlığı ilə `SignedBlockWire` faydalı yük kimi yazır.
  kanonik başlıqlar, əməliyyatlar, icra sertifikatları və şahid məlumatları birlikdə.
- **World State View** sürətli sorğular üçün səlahiyyətli dövləti yaddaşda saxlayır. Deterministik snapshotlar və
  boru kəməri yan avtomobilləri (`pipeline/sidecars.norito` + `pipeline/sidecars.index`) bərpa və yoxlamaları dəstəkləyir.
- **Dövlət pilləsi** deterministliyi qoruyaraq böyük yerləşdirmələr üçün isti/soyuq bölməyə imkan verir
  doğrulama.
- **Sinxronizasiya və təkrar oynatma** eyni doğrulama qaydalarından istifadə edərək törədilmiş blokları yenidən vəziyyətə yükləyin. Determinist
  yayım, həmyaşıdların etibarlı yaddaşa etibar etmədən qonşulardan itkin məlumatları bərpa edə bilməsini təmin edir.

## 7. İdarəetmə və iqtisadiyyat

- Zəncirdə olan parametrlər (`SetParameter`) nəzarət konsensus taymerləri, mempool limitləri, telemetriya düymələri, ödəniş diapazonları,
  və xüsusiyyət bayraqları. `kagami` tərəfindən yaradılan Yaradılış manifestləri ilkin konfiqurasiyanı quraşdırır.
- **Kaigi** təlimatları əməkdaşlıq sessiyalarını idarə edir (yaratmaq/qoşulmaq/buraxmaq/sonlandırmaq) və Norito axınını qidalandırır
  konfrans istifadə halları üçün telemetriya.
- **Hijiri** konsensus, qəbulla inteqrasiya edərək deterministik həmyaşıd və hesab reputasiyası təmin edir.
  siyasətlər və ödəniş çarpanları (Q16 sabit nöqtəli riyaziyyat). Sübutlar, yoxlama nöqtələri və nüfuz
  reyestrlər zəncir üzərində qurulur və müşahidəçi profilləri qəbzlərin mənşəyini idarə edir.
- **NPoS rejimi** (aktiv olduqda) qoruyarkən VRF ilə dəstəklənən seçki pəncərələrindən və pay ölçülmüş komitələrdən istifadə edir
  deterministik konfiqurasiya defoltları.
- **Məxfi registrlər** sıfır bilikli doğrulayıcı açarları, sübut həyat dövrlərini və öhdəlikləri idarə edir
  qorunan axınlar.

## 8. Müştəri təcrübəsi və alətlər

- **Torii API** əməliyyatlar, sorğular, hadisə axınları, telemetriya və üçün REST və WebSocket interfeyslərini təklif edir.
  idarəetmənin son nöqtələri. JSON proqnozları Norito sxemlərindən əldə edilmişdir.
- **CLI alətləri** (`iroha_cli`, `iroha_monitor`) idarəetməni, canlı həmyaşıd tablosunu və boru kəmərini əhatə edir
  yoxlama.
- **Yaradılış aləti** (`kagami`) Norito kodlu manifestləri, doğrulayıcı açar materialını və konfiqurasiyanı yaradır
  şablonlar.
- **SDKs** (Swift, JS/TS, Python) təlimatlara, sorğulara, tetikleyicilərə və telemetriyaya idiomatik girişi təmin edir.
- `scripts/` daxilində **skriptlər və CI qarmaqları** tablosunun yoxlanmasını, kodek bərpasını və tüstüsünü avtomatlaşdırır
  testlər.

## 9. Performans, möhkəmlik və yol xəritəsi- Cari boru kəməri əlverişli şəbəkə altında **2–3 saniyə** bloklama müddətləri ilə **20,000 ts** hədəfləyir
  toplu imza yoxlaması və deterministik planlaşdırma ilə dəstəklənən şərtlər.
- **Telemetriya** konsensus taymerləri, mempool doluluğu, blokun yayılması sağlamlığı,
  Kaigi istifadəsi və Hijiri reputasiyası yeniləmələri.
- **Dayanıqlıq xüsusiyyətləri** deterministik məlumatların mövcudluğu, bərpa yan arabaları, topologiyanın fırlanması və
  konfiqurasiya edilə bilən baxış/dəyişiklik hədləri.
- Gələcək yol xəritəsi mərhələləri (bax: `roadmap.md`) Nexus məlumat boşluqları, təkmilləşdirilmiş məxfilik üzərində işi davam etdirir
  alətlər və deterministik nəticələri qoruyarkən daha geniş aparat sürətləndirilməsi.

## 10. Əməliyyatlar və yerləşdirmə

- **Artifaktlar:** Dockerfiles, Nix flake və `cargo` iş axınları təkrarlana bilən quruluşları dəstəkləyir. `kagami` yayır
  genezis manifestləri, təsdiqləyici açarlar və həm icazə verilən, həm də NPoS yerləşdirmələri üçün nümunə konfiqurasiyalar.
- **Özündə yerləşdirilən şəbəkələr:** Operatorlar öz həmyaşıd dəstlərini, qəbul qaydalarını və təkmil ritmi idarə edirlər. The
  iş sahəsi koordinasiya olmadan birlikdə mövcud olan, yalnız paylaşılan bir çox müstəqil Iroha 2 şəbəkələrini dəstəkləyir.
  yuxarı kod.
- **Konfiqurasiyanın həyat dövrü:** `iroha_config` istifadəçini → faktiki → defolt təbəqələri həll edir, hər düymənin işləməsini təmin edir.
  açıq və versiya ilə idarə olunur. İş vaxtı dəyişiklikləri `SetParameter` təlimatları vasitəsilə həyata keçirilir.
- **Müşahidə edilə bilənlik:** `iroha_telemetry` Prometheus ölçülərini, strukturlaşdırılmış jurnalları və idarə paneli datasını yoxlayır
  CI skriptləri ilə (`ci/check_swift_dashboards.sh`, `scripts/render_swift_dashboards.sh`,
  `scripts/check_swift_dashboard_data.py`). Axın, konsensus və hiciri hadisələr artıq mövcuddur
  WebSocket və `scripts/sumeragi_backpressure_log_scraper.py` kardiostimulyatorun əks təzyiqi ilə əlaqələndirir
  problemlərin aradan qaldırılması üçün telemetriya.
- **Test:** `cargo test --workspace`, inteqrasiya testləri (`integration_tests/`), dil SDK dəstləri və
  Norito qızıl qurğular determinizmi qoruyur. Göstərici ABI, syscall siyahıları və idarəetmə manifestləri var
  xüsusi qızıl testlər.
- **Bərpa:** Kür yan avtomobilləri, deterministik təkrar oxutma və yayım sinxronizasiyası qovşaqlara diskdən vəziyyəti bərpa etməyə imkan verir
  və ya həmyaşıdları. Hijiri yoxlama məntəqələri və idarəetmə manifestləri uyğunluq üçün yoxlanıla bilən görüntülər təqdim edir.

# Lüğət

Bu sənəddə istinad edilən terminologiya üçün bu ünvanda olan layihə üzrə geniş lüğətə müraciət edin
.