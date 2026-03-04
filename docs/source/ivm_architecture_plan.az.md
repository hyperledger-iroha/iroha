---
lang: az
direction: ltr
source: docs/source/ivm_architecture_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da8a99adbbcf1d8b209a25da32e256c0dad2860633f373d7410a3a91d790c938
source_last_modified: "2026-01-21T19:17:13.236818+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM Memarlıq Refaktor Planı

Bu plan Iroha Virtual Maşını yenidən formalaşdırmaq üçün qısamüddətli mərhələləri əhatə edir.
(IVM) təhlükəsizlik və performans xüsusiyyətlərini qoruyarkən daha aydın təbəqələrə çevrilir.
O, məsuliyyətləri təcrid etməyə, ev sahibi inteqrasiyasını daha təhlükəsiz etməyə və
Kotodama dil yığınını müstəqil qutuya çıxarmaq üçün hazırlamaq.

## Məqsədlər

1. **Layered runtime fasad** – VM üçün açıq iş vaxtı interfeysini təqdim edin
   nüvə dar bir xüsusiyyətin arxasına yerləşdirilə bilər və alternativ cəbhələr inkişaf edə bilər
   daxili modullara toxunmadan.
2. **Host/syscall sərhədinin sərtləşdirilməsi** – marşrut sistemi vasitəsilə sistem çağırışının göndərilməsi
   hər hansı bir hostdan əvvəl ABI siyasətini və göstərici təsdiqini tətbiq edən xüsusi adapter
   kod icra edir.
3. **Dil/alət bölgüsü** – Kotodama xüsusi kodunu yeni qutuya köçürün və
   `ivm`-də yalnız bayt kodu icra səthini saxlayın.
4. **Konfiqurasiya uyğunluğu** – sürətləndirməni birləşdirin və funksiyaları olduğu kimi dəyişdirin
   `iroha_config` vasitəsilə idarə olunur, istehsalda ətraf mühitə əsaslanan düymələri çıxarır
   yollar.

## Faza Dağılımı

### Faza 1 – İş vaxtı fasadı (davam etməkdədir)
- Həyat dövrünü təsvir edən `VmEngine` xüsusiyyətini təyin edən `runtime` modulu əlavə edin
  əməliyyatlar (`load_program`, `execute`, əsas santexnika).
- `IVM` xüsusiyyətini həyata keçirməyi öyrət.  Bu, mövcud strukturu saxlayır, lakin imkan verir
  istehlakçılar (və gələcək testlər) beton əvəzinə interfeysdən asılı olacaq
  növləri.
- `lib.rs`-dən birbaşa modulun təkrar ixracını buraxmağa başlayın ki, zəng edənlər bu vasitəsilə idxal etsinlər.
  mümkün olduqda fasad.

**Təhlükəsizlik / performansa təsir**: Fasad birbaşa daxili girişi məhdudlaşdırır
dövlət; yalnız təhlükəsiz giriş nöqtələri açıqdır.  Bu, hostun yoxlanılmasını asanlaşdırır
qarşılıqlı əlaqə və qaz və ya TLV ilə işləmə ilə bağlı səbəb.

### Mərhələ 2 – Syscall dispetçeri
- `IVMHost`-i əhatə edən və ABI-ni tətbiq edən `SyscallDispatcher` komponentini təqdim edin
  siyasət və göstərici doğrulaması bir yerdə, bir dəfə.
- Dispetçerdən istifadə etmək üçün defolt host və istehzalı hostları köçürün
  təkrarlanan doğrulama məntiqi.
- Dispetçeri qoşula bilən edin ki, hostlar xüsusi alətlər olmadan təmin edə bilsinlər
  təhlükəsizlik yoxlamalarından yan keçmək.
- Klonlanmış VM-lərin yönləndirilməsi üçün `SyscallDispatcher::shared(...)` köməkçisi təmin edin
  hər bir işçi binası olmadan paylaşılan `Arc<Mutex<..>>` host vasitəsilə sistem çağırışları
  sifarişli sarğılar.

**Təhlükəsizlik/performans təsiri**: Mərkəzləşdirilmiş qapılar hostlardan qoruyur
`is_syscall_allowed`-ə zəng etməyi unutmayın və bu, göstəricinin gələcək keşləşdirilməsinə imkan verir
təkrar sistem çağırışları üçün doğrulamalar.

### Faza 3 – Kotodama çıxarılması
- Kotodama kompilyatoru `crates/kotodama_lang`-ə çıxarılıb (`crates/ivm/src/kotodama`-dən).
- VM-nin istehlak etdiyi minimal bayt kodu API təmin edin (`compile_to_ivm_bytecode`).

**Təhlükəsizlik / performans təsiri**: Ayırma VM-nin hücum səthini aşağı salır
əsas və tərcüməçi reqressiyalarını riskə atmadan dil innovasiyasına imkan verir.### Faza 4 – Konfiqurasiya konsolidasiyası
- `iroha_config` əvvəlcədən təyinetmələri (məsələn, GPU arxa uçlarını aktivləşdirmək) vasitəsilə ip sürətləndirmə variantlarını işlətmə zamanı öldürmə açarları kimi mövcud mühitin ləğvini (`IVM_DISABLE_CUDA`, `IVM_DISABLE_METAL`) saxlayaraq.
- `RuntimeConfig` obyektini yeni fasad vasitəsilə nümayiş etdirin ki, ev sahibləri seçim etsinlər
  deterministik sürətləndirmə siyasətləri açıq şəkildə.

**Təhlükəsizlik / performans təsiri**: Env əsaslı keçidlərin aradan qaldırılması səssizliyin qarşısını alır
konfiqurasiya sürüşməsi və yerləşdirmələr arasında deterministik davranışı təmin edir.

## Dərhal növbəti addımlar

- Fasad xüsusiyyətini əlavə etməklə və yüksək səviyyəli zəng saytlarını yeniləməklə 1-ci Mərhələni tamamlayın
  ondan asılıdır.
- Yalnız fasad və qəsdən ictimai API-ləri təmin etmək üçün ictimai reeksportları yoxlayın
  qutudan sızmaq.
- Sistem zəngi dispetçer API-ni ayrıca modulda prototip edin və onu köçürün
  təsdiqləndikdən sonra standart host.

Tətbiq edildikdən sonra hər bir mərhələ üzrə irəliləyiş `status.md`-də izləniləcək
davam edir.