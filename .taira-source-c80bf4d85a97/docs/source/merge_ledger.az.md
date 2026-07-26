---
lang: az
direction: ltr
source: docs/source/merge_ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44f1c681730f1c94d9d00e8f829a0134374ce6cb29f21727a27685e096f0da40
source_last_modified: "2026-01-17T06:10:29.077000+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Ledger Dizaynını birləşdirin - Zolaqların Sonluğu və Qlobal Azaldılması

Bu qeyd Milestone 5 üçün birləşmə kitabçası dizaynını yekunlaşdırır. O, izah edir
boş olmayan blok siyasəti, keçid zolaqlı QC semantikası və yekun iş axını
zolaq səviyyəsində icranı qlobal dünya dövlət öhdəliyinə bağlayır.

Dizayn `nexus.md`-də təsvir edilən Nexus arxitekturasını genişləndirir. kimi şərtlər
"zolaqlı blok", "zolaqlı QC", "birləşmə göstərişi" və "birləşmə jurnalı" onların miras qalır
həmin sənədin tərifi; bu qeyd davranış qaydalarına diqqət yetirir və
icra vaxtı, yaddaş və WSV tərəfindən tətbiq edilməli olan icra təlimatı
təbəqələr.

## 1. Qeyri-Boş Blok Siyasəti

**Qayda (MÜTLƏQ):** Zolaq təklif edən şəxs yalnız blokda at olan zaman blok verir
ən azı bir icra edilmiş əməliyyat fraqmenti, zamana əsaslanan tetikleyici və ya deterministik
artefakt yeniləməsi (məs., DA artefakt roll-up). Boş bloklar qadağandır.

** Nəticələr:**

- Slotun canlı saxlanması: heç bir əməliyyat onun deterministik öhdəliyi pəncərəsinə cavab vermədikdə,
zolaq heç bir blok buraxmır və sadəcə növbəti yuvaya keçir. Birləşmə dəftəri
həmin zolağın əvvəlki ucunda qalır.
- Tetik toplusu: heç bir vəziyyətə keçid yaratmayan fon tetikleyicileri (məsələn,
invariantları bir daha təsdiq edən cron) boş hesab olunur və KEÇİLMƏLİDİR və ya
blok istehsal etməzdən əvvəl digər işlərlə birləşdirilmişdir.
- Telemetriya: `pipeline_detached_merged` və izləmə ölçüləri atlandı
açıq şəkildə yuvalar - operatorlar "iş yoxdur" ilə "boru kəməri dayanıb" fərqlənə bilər.
- Təkrar: blok yaddaşı sintetik boş yer tutucuları daxil etmir. Kür
replay loop, sadəcə olaraq, əgər yoxsa, ardıcıl slotlar üçün eyni ana hashı müşahidə edir
blok buraxıldı.

**Kanonik Yoxlama:** Blok təklifi və doğrulama zamanı, `ValidBlock::commit`
əlaqəli `StateBlock`-in ən azı bir ört-basdır daşıdığını iddia edir
(delta, artefakt, tetikleyici). Bu, `StateBlock::is_empty` qoruyucusu ilə uyğunlaşır
bu artıq əməliyyatsız yazıların silinməsini təmin edir. İcra əvvəl baş verir
imzalar tələb olunur ki, komitələr heç vaxt boş yüklərə səs verməsinlər.

## 2. Cross-Lane QC Merge Semantics

Komitə tərəfindən yekunlaşdırılan hər bir zolaq bloku `B_i` istehsal edir:

- `lane_state_root_i`: hər DS dövlət kökləri üzərində Poseidon2-SMT öhdəliyinə toxundu
blokda.
- `merge_hint_root_i`: birləşmə kitabçası üçün yuvarlanan namizəd (`tag =
"iroha: birləşmə: namizəd: v1\0"`).
- `lane_qc_i`: zolaq komitəsindən toplanmış imzalar
  icra-səs preimage (blok hash, `parent_state_root`,
  `post_state_root`, hündürlük/görünüş/epox, chain_id və rejim etiketi).

Birləşmə qovşaqları `{(B_i, lane_qc_i, merge_hint_root_i)}` üçün ən son məsləhətləri toplayır
bütün zolaqlar `i ∈ [0, K)`.

**Girişi birləşdirin (MÜTLƏQ):**

```
MergeLedgerEntry {
    epoch_id: u64,
    lane_tips: [Hash32; K],
    merge_hint_root: [Hash32; K],
    global_state_root: Hash32,
    merge_qc: QuorumCertificate,
}
```- `lane_tips[i]` zolaq blokunun hashıdır, zolaq üçün birləşən giriş möhürləri
  `i`. Əgər zolaq əvvəlki birləşmə girişindən sonra heç bir blok buraxmayıbsa, bu dəyərdir
  təkrarlandı.
- `merge_hint_root[i]` müvafiq zolaqdan olan `merge_hint_root`-dir
  blok. `lane_tips[i]` təkrarlananda təkrarlanır.
- `global_state_root` bərabərdir `ReduceMergeHints(merge_hint_root[0..K-1])`, a
  Domen ayırma etiketi ilə Poseidon2 qat
  `"iroha:merge:reduce:v1\0"`. Azaltma deterministikdir və MÜTLƏQDİR
  həmyaşıdları arasında eyni dəyəri yenidən qurun.
- `merge_qc` birləşmə komitəsinin BFT kvorum sertifikatıdır.
  seriallaşdırılmış giriş.

**QC yükünü birləşdirin (MÜTLƏQ):**

Birləşmə komitəsinin üzvləri deterministik bir həzm imzalayır:

```
merge_qc_digest = blake2b32(
    "iroha:merge:qc:v1\0" ||
    chain_id ||
    norito(MergeLedgerSignPayload {
        view,
        epoch_id,
        lane_tips,
        merge_hint_roots,
        global_state_root,
    })
)
```

- `view` zolaq ipuçlarından əldə edilən birləşmə komitəsinin görünüşüdür (maks.
  Giriş tərəfindən möhürlənmiş zolaq başlıqları boyunca `view_change_index`).
- `chain_id` konfiqurasiya edilmiş zəncir identifikatoru sətridir (UTF-8 bayt).
- Faydalı yük yuxarıda göstərilən sahə sırası ilə Norito kodlaşdırmasından istifadə edir.

Alınan həzm `merge_qc.message_digest`-də saxlanılır və mesajdır
BLS imzaları ilə təsdiqlənir.

**QC Tikintisini birləşdirin (MÜTLƏQ):**

- Birləşmə komitəsinin siyahısı cari komissiya-topologiya təsdiqləyici dəstidir.
- Tələb olunan kvorum = `commit_quorum_from_len(roster_len)`.
- `merge_qc.signers_bitmap` iştirakçı təsdiqləyici indeksləri kodlayır (LSB-ilk)
  commit-topologiya qaydasında.
- `merge_qc.aggregate_signature` həzm üçün BLS-normal aqreqatdır
  yuxarıda.

**Təsdiqləmə (MÜTLƏQ):**

1. Hər bir `lane_qc_i`-i `lane_tips[i]`-ə qarşı yoxlayın və blok başlıqlarını təsdiqləyin
   uyğun `merge_hint_root_i` daxildir.
2. `Invalid` və ya icra olunmamış bloka heç bir `lane_qc_i` nöqtəsinin olmadığından əmin olun. The
   yuxarıdakı boş olmayan siyasət başlığa dövlət örtüklərinin daxil olmasını təmin edir.
3. `ReduceMergeHints`-i yenidən hesablayın və `global_state_root` ilə müqayisə edin.
4. Birləşmə QC həzmini yenidən hesablayın və imzalayan bitmapını, kvorum həddini,
   və commit-topologiya siyahısına qarşı ümumi imza.

**Müşahidə qabiliyyəti:** Birləşən qovşaqlar üçün Prometheus sayğacları buraxır
Slotları atlayan zolaqları vurğulamaq üçün `merge_entry_lane_repeats_total{i}`
əməliyyat görmə qabiliyyəti.

## 3. Yekun İş Akışı

### 3.1 Zolaq Səviyyəsi Sonluq

1. Əməliyyatlar deterministik slotlarda hər zolağa planlaşdırılır.
2. İcraçı `StateBlock`-ə örtüklər tətbiq edərək deltalar və
artefaktlar.
3. Təsdiq edildikdən sonra zolaq komitəsi icra-səsvermə preimajını imzalayır ki,
   blok hash, dövlət kökləri və hündürlüyü/görünüşü/epoxunu bağlayır. Tuple
   `(block_hash, lane_qc_i, merge_hint_root_i)` zolaqlı final sayılır.
4. Yüngül müştərilər DS-məhdud sübutlar üçün zolağın ucunu yekun hesab edə bilərlər, lakin
birləşmə kitabçası ilə barışmaq üçün əlaqəli `merge_hint_root` qeyd etməlidir
sonra.Zolaqlı komitələr hər bir verilənlər məkanıdır və qlobal öhdəliyi əvəz etmir
topologiya. Komitənin ölçüsü `3f+1` səviyyəsində müəyyən edilmişdir, burada `f`
məlumat məkanı kataloqu (`fault_tolerance`). Doğrulayıcı hovuz məlumat məkanıdır
validatorlar (admin tərəfindən idarə olunan zolaqlar və ya ictimai zolaqlar üçün zolaq idarəetmə manifestləri
pay seçilmiş zolaqlar üçün staking qeydləri). Komissiya üzvüdür
ilə bağlı VRF epox toxumundan istifadə edərək, deterministik olaraq dövr başına bir dəfə nümunə götürülür
`dataspace_id` və `lane_id`. Hovuz `3f+1`-dən kiçikdirsə, zolaq sonluğu
kvorum bərpa olunana qədər fasilə verir (fövqəladə halların bərpası ayrıca aparılır).

### 3.2 Merge-Ledger Sonluğu

1. Birləşmə komitəsi ən son zolaq tövsiyələrini toplayır, hər bir `lane_qc_i`-ni yoxlayır və
yuxarıda müəyyən edildiyi kimi `MergeLedgerEntry` qurur.
2. Deterministik azalmanı yoxladıqdan sonra birləşmə komitəsi imzalayır
giriş (`merge_qc`).
3. Qovşaqlar girişi birləşmə jurnalı jurnalına əlavə edir və onu qeydin yanında saxlayır
zolaqlı blok istinadları.
4. `global_state_root` dünyanın nüfuzlu dövlət öhdəliyinə çevrilir.
dövr/slot. Tam qovşaqlar bunu əks etdirmək üçün WSV yoxlama nöqtəsi metadatasını yeniləyir
dəyər; deterministik təkrarlama eyni reduksiyanı təkrarlamalıdır.

### 3.3 WSV və Saxlama İnteqrasiyası

- `State::commit_merge_entry` hər zolaqlı dövlət köklərini və
  son `global_state_root`, zolağın icrasını qlobal yoxlama cəmi ilə əlaqələndirir.
- Kür `MergeLedgerEntry` zolaq bloku artefaktlarına bitişik olaraq davam edir
  təkrar oynatma həm zolaq səviyyəli, həm də qlobal sonluq ardıcıllığını yenidən qura bilər.
- Zolaq bir yuvadan keçdikdə, saxlama sadəcə olaraq əvvəlki ucunu saxlayır; yox
  yertutan birləşmə qeydləri ən azı bir zolaq yenisini yaradana qədər yaradılır
  blok.
- API səthləri (Torii, telemetriya) həm zolaq ipuçlarını, həm də ən son birləşməni ifşa edir
  operatorlar və müştərilər hər zolaqlı və qlobal görünüşləri uyğunlaşdıra bilməsi üçün giriş.

## 4. İcra Qeydləri- `crates/iroha_core/src/state.rs`: `State::commit_merge_entry` təsdiqləyir
  azaldılması və zolaqlı/qlobal metaməlumatların dünya dövlətinə ötürülməsi üçün sorğular
  və müşahidəçilər birləşmə göstərişlərinə və nüfuzlu qlobal hashlara daxil ola bilərlər.
- `crates/iroha_core/src/kura.rs`: `Kura::store_block_with_merge_entry` növbələr
  bloklayır və əlaqəli birləşmə girişini bir addımda davam etdirir, geriyə yuvarlanır
  əlavə uğursuz olduqda yaddaş bloku heç vaxt bloku qeyd etmir
  möhürlənmiş metadata olmadan. Birləşmə-mühasibat jurnalı kilidləmə addımında kəsilir
  başlanğıcın bərpası zamanı təsdiqlənmiş blok hündürlüyü ilə və yaddaşda keşlənir
  məhdud pəncərə ilə (`kura.merge_ledger_cache_capacity`, standart 256)
  uzun müddət davam edən qovşaqlarda qeyri-məhdud böyümədən qaçın. Bərpa qismən və ya kəsilir
  böyük ölçülü birləşmə-mühasibat dəftərinin quyruq girişlərini əlavə edin və yuxarıdakı qeydləri rədd edir
  maksimum paylama ölçüsü qoruyucu ayırmaları.
- `crates/iroha_core/src/block.rs`: blok yoxlaması olmadan blokları rədd edir
  giriş nöqtələri (xarici əməliyyatlar və ya vaxt tetikleyicileri) və deterministik olmadan
  DA paketləri (`BlockValidationError::EmptyBlock`) kimi artefaktlar,
  boş olmayan siyasət imzalar tələb olunmadan və daşınmazdan əvvəl tətbiq edilir
  birləşmə kitabçasına daxil edin.
- Deterministik azalma köməkçisi birləşmə xidmətində yaşayır: `reduce_merge_hint_roots`
  (`crates/iroha_core/src/merge.rs`) yuxarıda təsvir edilən Poseidon2 qatını həyata keçirir.
  Avadanlıq sürətləndirici qarmaqlar gələcək iş olaraq qalır, lakin skalyar yol indi tətbiq olunur
  deterministik olaraq kanonik azalma.
- Telemetriya inteqrasiyası: hər zolaqlı birləşmə təkrarlarını və
  `global_state_root` ölçmə göstəricisi müşahidə oluna bilənlər siyahısında izlənilir, beləliklə
  tablosunun işi birləşmə xidmətinin təqdimatı ilə yanaşı göndərilə bilər.
- Komponentlər arası testlər: birləşmənin azaldılması üçün qızıl təkrar əhatə dairəsi
  gələcək dəyişiklikləri təmin etmək üçün inteqrasiya testi ilə izlənilir
  `reduce_merge_hint_roots` qeydə alınmış kökləri sabit saxlayır.