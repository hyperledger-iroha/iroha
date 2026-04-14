<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
direction: ltr
source: docs/source/kura_wsv_security_performance_audit_20260219.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 194721ce71f5593cc9e4df6313c6e3aa85c5c3dc0e3efe4a28d0ded968c0584a
source_last_modified: "2026-02-19T08:31:06.766140+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Kür / WSV Təhlükəsizlik və Performans Auditi (2026-02-19)

## Əhatə dairəsi

Bu audit aşağıdakıları əhatə edirdi:

- Kür əzmkarlığı və büdcə yolları: `crates/iroha_core/src/kura.rs`
- İstehsal WSV/dövlət öhdəliyi/sorğu yolları: `crates/iroha_core/src/state.rs`
- IVM WSV saxta host səthləri (test/dev əhatə dairəsi): `crates/ivm/src/mock_wsv.rs`

Əhatə dairəsi xaricində: əlaqəli olmayan qutular və tam sistemli sınaq təkrarları.

## Risk Xülasəsi

- Kritik: 0
- Yüksək: 4
- Orta: 6
- Aşağı: 2

## Tapıntılar (Ciddiliyə görə sıralanıb)

### Yüksək

1. **Kür yazıçısı giriş/çıxış xətalarında panikləyir (qovşağın mövcudluğu riski)**
- Komponent: Kür
- Növ: Təhlükəsizlik (DoS), Etibarlılıq
- Təfərrüat: bərpa edilə bilən xətaları qaytarmaq əvəzinə əlavə/indeks/fsync xətalarında yazıçı döngəsi panikasına düşür, beləliklə, keçici disk nasazlıqları qovşaq prosesini dayandıra bilər.
- Sübut:
  - `crates/iroha_core/src/kura.rs:1697`
  - `crates/iroha_core/src/kura.rs:1724`
  - `crates/iroha_core/src/kura.rs:1845`
  - `crates/iroha_core/src/kura.rs:1854`
  - `crates/iroha_core/src/kura.rs:1860`
- Təsir: uzaqdan yükləmə + yerli disk təzyiqi qəza/yenidən başladın döngələrə səbəb ola bilər.2. **Kürənin çıxarılması `block_store` mutex altında tam məlumat/indeks yenidən yazır**
- Komponent: Kür
- Növ: Performans, Əlçatımlılıq
- Təfərrüat: `evict_block_bodies`, `block_store` kilidini saxlayaraq müvəqqəti fayllar vasitəsilə `blocks.data` və `blocks.index`-i yenidən yazır.
- Sübut:
  - Kilidin alınması: `crates/iroha_core/src/kura.rs:834`
  - Tam yenidən yazma döngələri: `crates/iroha_core/src/kura.rs:921`, `crates/iroha_core/src/kura.rs:942`
  - Atom dəyişdirmə/sinxronizasiya: `crates/iroha_core/src/kura.rs:956`, `crates/iroha_core/src/kura.rs:960`
- Təsir: evakuasiya hadisələri böyük tarixlərdə uzun müddət yazılanları/oxumaları dayandıra bilər.

3. **Dövlət öhdəliyi ağır işlərdə kobud `view_lock` təşkil edir**
- Komponent: İstehsal WSV
- Növ: Performans, Əlçatımlılıq
- Təfərrüat: bloklama əməliyyatları, blok hashləri və dünya vəziyyətini həyata keçirərkən eksklüziv `view_lock` saxlayır, ağır bloklar altında oxucu aclığı yaradır.
- Sübut:
  - Kilidin saxlanması başlayır: `crates/iroha_core/src/state.rs:17456`
  - Daxili kiliddə işləmək: `crates/iroha_core/src/state.rs:17466`, `crates/iroha_core/src/state.rs:17476`, `crates/iroha_core/src/state.rs:17483`
- Təsir: davamlı ağır öhdəliklər sorğu/konsensusun cavab vermə qabiliyyətini aşağı sala bilər.4. **IVM JSON admin ləqəbləri zəng edənin yoxlanılması olmadan imtiyazlı mutasiyalara icazə verir (test/dev hostu)**
- Komponent: IVM WSV saxta host
- Növ: Təhlükəsizlik (test/dev mühitlərində imtiyazların artırılması)
- Təfərrüat: JSON ləqəb işləyiciləri zəng edənin əhatəli icazə nişanlarını tələb etməyən rol/icazə/peer mutasiya metodlarına birbaşa marşrut verir.
- Sübut:
  - Admin ləqəbləri: `crates/ivm/src/mock_wsv.rs:4274`, `crates/ivm/src/mock_wsv.rs:4371`, `crates/ivm/src/mock_wsv.rs:4448`
  - Bağlanmamış mutatorlar: `crates/ivm/src/mock_wsv.rs:1035`, `crates/ivm/src/mock_wsv.rs:1055`, `crates/ivm/src/mock_wsv.rs:855`
  - Fayl sənədlərində əhatə dairəsi qeydi (test/dev niyyəti): `crates/ivm/src/mock_wsv.rs:295`
- Təsir: sınaq müqavilələri/alətləri inteqrasiya qoşqularında təhlükəsizlik fərziyyələrini öz-özünə yüksəldə və etibarsız edə bilər.

### Orta

5. **Kür büdcə yoxlamaları hər növbədə gözləyən blokları yenidən kodlaşdırır (hər yazı üçün O(n))**
- Komponent: Kür
- Növ: Performans
- Təfərrüat: hər növbə, gözləyən blokları təkrarlamaqla və hər birini kanonik naqil ölçüsü yolu ilə seriallaşdırmaqla gözləyən növbə baytlarını yenidən hesablayır.
- Sübut:
  - Növbə skanı: `crates/iroha_core/src/kura.rs:2509`
  - Blok başına kodlaşdırma yolu: `crates/iroha_core/src/kura.rs:2194`, `crates/iroha_core/src/kura.rs:2525`
  - Növbədə büdcə yoxlamasına çağırılır: `crates/iroha_core/src/kura.rs:2580`, `crates/iroha_core/src/kura.rs:2050`
- Təsir: gecikmə altında ötürmə qabiliyyətinin deqradasiyasını yazın.6. **Kür büdcə yoxlamaları hər növbəyə təkrar blok-mağaza metadata oxunmasını həyata keçirir**
- Komponent: Kür
- Növ: Performans
- Təfərrüat: hər bir yoxlama `block_store` kilidini bağlayarkən davamlı indeks sayını və fayl uzunluqlarını oxuyur.
- Sübut:
  - `crates/iroha_core/src/kura.rs:2538`
  - `crates/iroha_core/src/kura.rs:2548`
  - `crates/iroha_core/src/kura.rs:2575`
- Təsir: isti növbə yolunda qarşısı alına bilən giriş/çıxış/bağlantı yükü.

7. **Kür köçürülməsi növbə büdcəsi yolundan daxil edilir**
- Komponent: Kür
- Növ: Performans, Əlçatımlılıq
- Təfərrüat: növbə yolu, yeni blokları qəbul etməzdən əvvəl sinxron olaraq evakuasiya çağıra bilər.
- Sübut:
  - Zəng zəncirini sıralayın: `crates/iroha_core/src/kura.rs:2050`
  - Daxili çıxarılma çağırışı: `crates/iroha_core/src/kura.rs:2603`
- Təsir: büdcəyə yaxın olduqda əməliyyat/blok qəbulunda gecikmə sürəti artır.

8. **`State::view` mübahisə altında kobud kilid əldə etmədən geri qayıda bilər**
- Komponent: İstehsal WSV
- Növ: Ardıcıllıq/Performans mübadiləsi
- Təfərrüat: yazma kilidi mübahisəsində, `try_read` geri qaytarılması dizaynı ilə qaba qoruyucu olmadan görünüşü qaytarır.
- Sübut:
  - `crates/iroha_core/src/state.rs:14543`
  - `crates/iroha_core/src/state.rs:14545`
  - `crates/iroha_core/src/state.rs:18301`
- Təsir: təkmilləşdirilmiş canlılıq, lakin zəng edənlər mübahisə altında daha zəif çarpaz komponent atomikliyinə dözməlidirlər.9. **`apply_without_execution` DA kursorunun irəliləyişində sərt `expect` istifadə edir**
- Komponent: İstehsal WSV
- Növ: Təhlükəsizlik (panic-on-invariant-break vasitəsilə DoS), Etibarlılıq
- Təfərrüat: DA kursorunun irəliləyiş invariantları uğursuz olarsa, bağlanmış blok yol panikasını tətbiq edir.
- Sübut:
  - `crates/iroha_core/src/state.rs:17621`
  - `crates/iroha_core/src/state.rs:17625`
- Təsir: gizli doğrulama/indeksləmə səhvləri qovşaq öldürən uğursuzluqlara çevrilə bilər.

10. **IVM TLV dərc sistem zəngində ayırmadan əvvəl bağlanmış açıq zərf ölçüsü yoxdur (test/dev host)**
- Komponent: IVM WSV saxta host
- Növ: Təhlükəsizlik (yaddaş DoS), Performans
- Təfərrüat: başlıq uzunluğunu oxuyur, sonra bu yolda host səviyyəsində qapaq olmadan tam TLV yükünü ayırır/nüsxələyir.
- Sübut:
  - `crates/ivm/src/mock_wsv.rs:3750`
  - `crates/ivm/src/mock_wsv.rs:3755`
  - `crates/ivm/src/mock_wsv.rs:3759`
- Təsir: zərərli test yükləri böyük ayırmaları məcbur edə bilər.

### Aşağı

11. **Kür bildiriş kanalı məhdudiyyətsizdir (`std::sync::mpsc::channel`)**
- Komponent: Kür
- Növ: Performans/Yaddaş gigiyenası
- Təfərrüat: bildiriş kanalı davamlı istehsalçı təzyiqi zamanı lazımsız oyanma hadisələrini toplaya bilər.
- Sübut:
  - `crates/iroha_core/src/kura.rs:552`
- Təsir: yaddaş artımı riski hər hadisə ölçüsünə görə aşağıdır, lakin qarşısını almaq olar.12. **Boru kəmərinin yan vaqon növbəsi yazıçı boşalana qədər yaddaşda məhdudiyyətsizdir**
- Komponent: Kür
- Növ: Performans/Yaddaş gigiyenası
- Təfərrüat: yan vaqon növbəsi `push_back`-də açıq qapaq/arxa təzyiq yoxdur.
- Sübut:
  - `crates/iroha_core/src/kura.rs:104`
  - `crates/iroha_core/src/kura.rs:3427`
- Təsir: uzunmüddətli yazıçı gecikmələri zamanı potensial yaddaş artımı.

## Mövcud Test Əhatəsi və Boşluqlar

### Kür

- Mövcud əhatə dairəsi:
  - saxlama büdcəsi davranışı: `store_block_rejects_when_budget_exceeded`, `store_block_rejects_when_pending_blocks_exceed_budget`, `store_block_evicts_when_block_exceeds_budget` (`crates/iroha_core/src/kura.rs:6820`, `crates/iroha_core/src/kura.rs:6949`, `crates/iroha_core/src/kura.rs:6984`)
  - çıxarılma düzgünlüyü və rehidrasiya: `evict_block_bodies_does_not_truncate_unpersisted`, `evicted_block_rehydrates_from_da_store` (`crates/iroha_core/src/kura.rs:8040`, `crates/iroha_core/src/kura.rs:8126`)
- Boşluqlar:
  - çaxnaşma olmadan əlavə/index/fsync nasazlığının aradan qaldırılması üçün xəta-injection əhatə dairəsi yoxdur
  - Gözləyən böyük növbələr və büdcə yoxlaması xərcləri üçün performans reqressiya testi yoxdur
  - Kilid mübahisəsi altında uzun tarixli çıxarılma gecikmə testi yoxdur

### İstehsal WSV

- Mövcud əhatə dairəsi:
  - mübahisənin geri qaytarılması davranışı: `state_view_returns_when_view_lock_held` (`crates/iroha_core/src/state.rs:18293`)
  - səviyyəli arxa hissə ətrafında kilid sifarişi təhlükəsizliyi: `state_commit_does_not_hold_tiered_backend_while_waiting_for_view_lock` (`crates/iroha_core/src/state.rs:18321`)
- Boşluqlar:
  - ağır dünya öhdəlikləri altında maksimum məqbul öhdəlik saxlama müddətini təsdiqləyən kəmiyyət mübahisəsi testi yoxdur
  - DA kursorunun irəliləmə invariantları gözlənilmədən pozularsa, paniksiz işləmə üçün reqressiya testi yoxdur

### IVM WSV Mock Host- Mövcud əhatə dairəsi:
  - icazə JSON parser semantikası və peer təhlili (`crates/ivm/src/mock_wsv.rs:5234`, `crates/ivm/src/mock_wsv.rs:5332`)
  - TLV deşifrəsi və JSON deşifrəsi ətrafında siscall tüstü testləri (`crates/ivm/src/mock_wsv.rs:5962`, `crates/ivm/src/mock_wsv.rs:6078`)
- Boşluqlar:
  - icazəsiz admin ləqəbdən imtina testləri yoxdur
  - `INPUT_PUBLISH_TLV`-də böyük ölçülü TLV zərfinin rədd edilməsi testləri yoxdur
  - Nəzarət məntəqəsi/bərpa klon dəyəri ətrafında heç bir etalon/korkuluk testləri

## Prioritetləşdirilmiş Təmir Planı

### Faza 1 (Yüksək təsirli sərtləşmə)

1. Kür yazıçısı `panic!` filiallarını bərpa edilə bilən xəta yayılması + pisləşmiş sağlamlıq siqnalı ilə əvəz edin.
- Hədəf fayllar: `crates/iroha_core/src/kura.rs`
- Qəbul:
  - enjekte edilmiş əlavə/index/fsync uğursuzluqları panik yaratmır
  - xətalar telemetriya/logging vasitəsilə aşkar edilir və yazıçı idarə oluna bilir

2. IVM saxta host TLV nəşri və JSON zərf yolları üçün məhdud zərf çekləri əlavə edin.
- Hədəf fayllar: `crates/ivm/src/mock_wsv.rs`
- Qəbul:
  - böyük ölçülü faydalı yüklər ağır işlənmədən əvvəl rədd edilir
  - yeni testlər həm TLV, həm də JSON böyük ölçülü halları əhatə edir

3. JSON admin ləqəbləri (və ya ciddi yalnız test funksiyası bayraqlarının arxasındakı qapı ləqəbləri və aydın sənəd) üçün açıq zəng edən icazə yoxlamalarını tətbiq edin.
- Hədəf fayllar: `crates/ivm/src/mock_wsv.rs`
- Qəbul:
  - icazəsiz zəng edən şəxs ləqəblər vasitəsilə rol/icazə/peer vəziyyətini dəyişdirə bilməz

### Faza 2 (Qaynar yol performansı)4. Kür büdcəsinin uçotu artımlı olsun.
- Növbə başına tam gözlənilən növbənin yenidən hesablanmasını növbəyə/davam etməyə/düşməyə yenilənən saxlanılan sayğaclarla əvəz edin.
- Qəbul:
  - Gözləyən baytların hesablanması üçün O(1) yaxınlığında növbə dəyəri
  - reqressiya bençmarkı gözlənilən dərinlik artdıqca sabit gecikməni göstərir

5. Evdən çıxarılma kilidinin saxlanma müddətini azaldın.
- Seçimlər: seqmentlərə bölünmüş sıxlaşdırma, kilid buraxma sərhədləri ilə parçalanmış surət və ya məhdud ön planda bloklama ilə arxa plana qulluq rejimi.
- Qəbul:
  - böyük tarixdə çıxarılma gecikməsi azalır və ön planda olan əməliyyatlar həssas olaraq qalır

6. Mümkün olduqda qaba `view_lock` kritik hissəni qısaldın.
- Eksklüziv saxlama pəncərələrini minimuma endirmək üçün öhdəliyin bölünməsi mərhələlərini və ya mərhələli deltaların snapshotını qiymətləndirin.
- Qəbul:
  - çəkişmə ölçüləri ağır bloklamalar zamanı 99p saxlama müddətini azaldır

### Faza 3 (Əməliyyat qoruyucuları)

7. Kür yazıçısı və yan vaqon növbəsinin əks təzyiqi/qapaqları üçün məhdud/birləşdirilmiş oyanma siqnalını təqdim edin.
8. Telemetriya panellərini aşağıdakılar üçün genişləndirin:
- `view_lock` gözləmə/tutma paylamaları
- evakuasiya müddəti və hər qaçış üçün geri alınmış bayt
- büdcəni yoxlamaq növbəsinin gecikməsi

## Təklif olunan Test Əlavələri1. `kura_writer_io_failures_do_not_panic` (vahid, nasaz inyeksiya)
2. `kura_budget_check_scales_with_pending_depth` (performans reqressiyası)
3. `kura_eviction_does_not_block_reads_beyond_threshold` (inteqrasiya/mükəmməl)
4. `state_commit_view_lock_hold_under_heavy_world_commit` (mübahisəli reqressiya)
5. `state_apply_without_execution_handles_da_cursor_error_without_panic` (davamlılıq)
6. `mock_wsv_admin_alias_requires_permissions` (təhlükəsizlik reqressiyası)
7. `mock_wsv_input_publish_tlv_rejects_oversize` (DoS mühafizəsi)
8. `mock_wsv_checkpoint_restore_cost_regression` (mükəmməl meyar)

## Əhatə və Etibar haqqında Qeydlər

- `crates/iroha_core/src/kura.rs` və `crates/iroha_core/src/state.rs` üçün tapıntılar istehsal yolu tapıntılarıdır.
- `crates/ivm/src/mock_wsv.rs` üçün tapıntılar fayl səviyyəli sənədlərə görə açıq şəkildə test/dev host əhatə edir.
- Bu auditin özü ABI versiyasında dəyişiklik tələb etmir.