---
lang: az
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3c952f92e009ea4b2ccba55940737889e8e70f506d09899598bd913a2ac68d2d
source_last_modified: "2026-01-22T14:35:36.839904+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-refactor-plan
title: Sora Nexus ledger refactor plan
description: Mirror of `docs/source/nexus_refactor_plan.md`, detailing the phased clean-up work for the Iroha 3 codebase.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
Bu səhifə `docs/source/nexus_refactor_plan.md`-i əks etdirir. Çoxdilli nəşr portalda yerləşənə qədər hər iki nüsxəni düzülmüş saxlayın.
:::

# Sora Nexus Ledger Refactor Planı

Bu sənəd Sora Nexus Ledger ("Iroha 3") refaktoru üçün dərhal yol xəritəsini əks etdirir. O, mövcud anbar tərtibatını və genezis/WSV mühasibat uçotu, Sumeragi konsensus, smart-kontrakt tetikleyicileri, snapshot sorğuları, göstərici-ABI host bağlamaları və Norito kodeklərində müşahidə olunan reqressiyaları əks etdirir. Məqsəd, bütün düzəlişləri bir monolit yamaqda yerləşdirməyə cəhd etmədən ardıcıl, sınaqdan keçirilə bilən bir arxitekturada birləşməkdir.

## 0. Rəhbər Prinsiplər
- Heterojen aparatlarda deterministik davranışı qoruyun; sürətləndirmədən yalnız eyni ehtiyatları olan qoşulma funksiyası bayraqları vasitəsilə istifadə edin.
- Norito serializasiya qatıdır. İstənilən vəziyyət/sxem dəyişikliyinə Norito kodlaşdırma/deşifrə gediş-dönüş testləri və qurğu yeniləmələri daxil edilməlidir.
- Konfiqurasiya `iroha_config` vasitəsilə axır (istifadəçi → faktiki → defolt). İstehsal yollarından ad-hoc mühit keçidlərini silin.
- ABI siyasəti V1 olaraq qalır və müzakirə olunmur. Hostlar naməlum göstərici növlərini/sistem zənglərini deterministik şəkildə rədd etməlidir.
- `cargo test --workspace` və qızıl testlər (`ivm`, `norito`, `integration_tests`) hər bir mərhələ üçün əsas qapı olaraq qalır.

## 1. Repozitor Topologiyası Snapshot
- `crates/iroha_core`: Sumeragi aktyorlar, WSV, genezis yükləyicisi, boru kəmərləri (sorğu, üst-üstə düşmə, zk zolaqları), smart-müqavilə əsas yapışqan.
- `crates/iroha_data_model`: zəncirdəki məlumatlar və sorğular üçün səlahiyyətli sxem.
- `crates/iroha`: CLI, testlər, SDK tərəfindən istifadə edilən müştəri API.
- `crates/iroha_cli`: operator CLI, hazırda `iroha`-də çoxsaylı API-ləri əks etdirir.
- `crates/ivm`: Kotodama bayt kodu VM, göstərici-ABI host inteqrasiyası giriş nöqtələri.
- `crates/norito`: JSON adapterləri və AoS/NCB arxa ucları ilə seriallaşdırma kodek.
- `integration_tests`: genezis/bootstrap, Sumeragi, tetikleyiciler, səhifələşdirmə və s. əhatə edən çarpaz komponent təsdiqləmələri.
- Sənədlər artıq Sora Nexus Ledger məqsədlərini təsvir edir (`nexus.md`, `new_pipeline.md`, `ivm.md`), lakin icra kodla müqayisədə parçalanmış və qismən köhnəlib.

## 2. Refaktor Sütunları və Mərhələləri

### Faza A – Əsaslar və Müşahidə Edilə bilənlik
1. **WSV Telemetriya + Snapşotlar**
   - Sorğular, Sumeragi və CLI tərəfindən istifadə edilən `state` (`WorldStateSnapshot` xüsusiyyət)-də kanonik snapshot API qurun.
   - `iroha state dump --format norito` vasitəsilə deterministik snapshotlar yaratmaq üçün `scripts/iroha_state_dump.sh` istifadə edin.
2. **Genesis/Bootstrap Determinism**
   - Tək Norito ilə işləyən boru kəməri (`iroha_core::genesis`) vasitəsilə axmaq üçün refaktor genezisi qəbulu.
  - Yaradılış üstəgəl birinci bloku təkrarlayan və arm64/x86_64 (`integration_tests/tests/genesis_replay_determinism.rs` altında izlənilir) üzrə eyni WSV köklərini təsdiq edən inteqrasiya/reqressiya əhatəsi əlavə edin.
3. **Cross-Crate Fixity Tests**
   - WSV, boru kəməri və ABI invariantlarını bir qoşqda təsdiqləmək üçün `integration_tests/tests/genesis_json.rs` genişləndirin.
  - Sxem sürüşməsində çaxnaşmaya səbəb olan `cargo xtask check-shape` iskelesini təqdim edin (DevEx alətlərinin yığılması ilə izlənilir; `scripts/xtask/README.md` fəaliyyət elementinə baxın).

### Faza B – WSV və Sorğu Səthi
1. **Dövlət Saxlama Əməliyyatları**
   - `state/storage_transactions.rs`-i sifariş vermə və münaqişənin aşkarlanmasına məcbur edən tranzaksiya adapterinə yığışdırın.
   - Vahid testləri indi aktivin/dünyanın/tetikleyici dəyişikliklərin uğursuzluqla geri dönməsini təsdiqləyir.
2. **Sorğu Modeli Refaktoru**
   - Səhifələmə/kursor məntiqini `crates/iroha_core/src/query/` altında təkrar istifadə edilə bilən komponentlərə köçürün. `iroha_data_model`-də Norito təsvirlərini uyğunlaşdırın.
  - Deterministik sifarişlə (cari əhatə dairəsi üçün `crates/iroha_core/tests/snapshot_iterable.rs` vasitəsilə izlənilir) triggerlər, aktivlər və rollar üçün snapshot sorğuları əlavə edin.
3. **Snapshot ardıcıllığı**
   - `iroha ledger query` CLI-nin Sumeragi/alıcılarla eyni snapshot yolundan istifadə etdiyinə əmin olun.
   - CLI snapshot reqressiya testləri `tests/cli/state_snapshot.rs` altında yaşayır (yavaş qaçışlar üçün xüsusiyyət qapalı).

### Faza C – Sumeragi Boru Kəməri
1. **Topologiya və Epoxa İdarəetmə**
   - `EpochRosterProvider`-i WSV pay snapshotları ilə dəstəklənən tətbiqlərlə əlamətə çıxarın.
  - `WsvEpochRosterAdapter::from_peer_iter` skamyalar/testlər üçün sadə istehzaya uyğun konstruktor təklif edir.
2. **Konsensus axınının sadələşdirilməsi**
   - `crates/iroha_core/src/sumeragi/*`-i modullara yenidən təşkil edin: `consensus` altında paylaşılan növlərlə `pacemaker`, `aggregation`, `availability`, `witness`.
  - Ad-hoc mesaj ötürülməsini çap edilmiş Norito zərfləri ilə əvəz edin və görünüş dəyişikliyi xüsusiyyəti testlərini tətbiq edin (Sumeragi mesajlaşma qeydində izlənilir).
3. **Line/Proof İnteqrasiya**
   - Zolaq sübutlarını DA öhdəlikləri ilə uyğunlaşdırın və RBC keçidinin vahid olmasını təmin edin.
   - `integration_tests/tests/extra_functional/seven_peer_consistency.rs` end-to-end inteqrasiya testi indi RBC-nin aktivləşdirilmiş yolunu yoxlayır.

### Faza D – Ağıllı Müqavilələr və Pointer-ABI Hostları
1. **Host Sərhəd Auditi**
   - Göstərici tipli yoxlamaları (`ivm::pointer_abi`) və host adapterlərini (`iroha_core::smartcontracts::ivm::host`) birləşdirin.
   - Göstərici cədvəli gözləntiləri və host manifest bağlamaları qızıl TLV xəritələrini həyata keçirən `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` və `ivm_host_mapping.rs` tərəfindən əhatə olunur.
2. **Trigger Execution Sandbox**
   - Refaktor qaz, göstəricinin yoxlanılması və hadisə jurnalını tətbiq edən ümumi `TriggerExecutor` vasitəsilə işləmək üçün tetikler.
  - Uğursuzluq yollarını əhatə edən zəng/zaman tetikleyicileri üçün reqressiya testləri əlavə edin (`crates/iroha_core/tests/trigger_failure.rs` vasitəsilə izlənilir).
3. **CLI & Client Alignment**
   - CLI əməliyyatlarının (`audit`, `gov`, `sumeragi`, `ivm`) sürüşmənin qarşısını almaq üçün paylaşılan `iroha` müştəri funksiyalarına arxalanmasını təmin edin.
   - CLI JSON snapshot testləri `tests/cli/json_snapshot.rs`-də canlıdır; əsas əmr çıxışının kanonik JSON istinadına uyğun gəlməsi üçün onları yeni saxlayın.

### Faza E – Norito Codec Hardening
1. **Sxem Qeydiyyatı**
   - Əsas məlumat növləri üçün kanonik kodlaşdırmalar əldə etmək üçün `crates/norito/src/schema/` altında Norito sxem reyestrini yaradın.
   - Nümunə yükünün kodlaşdırılmasını (`norito::schema::SamplePayload`) yoxlayan sənəd testləri əlavə edildi.
2. **Qızıl qurğuların yenilənməsi**
   - `crates/norito/tests/*` qızıl qurğularını yeni WSV sxeminə uyğunlaşdırmaq üçün refaktor eniş etdikdən sonra yeniləyin.
   - `scripts/norito_regen.sh`, `norito_regen_goldens` köməkçisi vasitəsilə Norito JSON qızıl rənglərini deterministik şəkildə bərpa edir.
3. **IVM/Norito İnteqrasiya**
   - Göstərici ABI metadatasının ardıcıl olmasını təmin edərək, Kotodama manifest serializasiyasını Norito vasitəsilə doğrulayın.
   - `crates/ivm/tests/manifest_roundtrip.rs` manifestlər üçün Norito kodlaşdırma/deşifrə paritetini saxlayır.

## 3. Çapraz kəsişmə narahatlığı
- **Sınaq Strategiyası**: Hər bir mərhələ vahid testlərini → qutu testlərini → inteqrasiya testlərini təşviq edir. Uğursuz testlər cari reqressiyaları ələ keçirir; yeni testlər onların yenidən üzə çıxmasına mane olur.
- **Sənədləşdirmə**: Hər mərhələ endikdən sonra, `status.md`-i yeniləyin və tamamlanmış tapşırıqları budayarkən açıq elementləri `roadmap.md`-ə yuvarlayın.
- **Performans Qiymətləndirmələri**: `iroha_core`, `ivm` və `norito`-də mövcud skamyaları qoruyun; heç bir reqressiyanı təsdiqləmək üçün refaktordan sonrakı ilkin ölçüləri əlavə edin.
- **Funksiya Bayraqları**: Yalnız xarici alətlər zəncirləri (`cuda`, `zk-verify-batch`) tələb edən arxa uçlar üçün sandıq səviyyəli keçidləri saxlayın. CPU SIMD yolları həmişə iş vaxtında qurulur və seçilir; dəstəklənməyən aparat üçün deterministik skalyar ehtiyatları təmin edir.

## 4. Dərhal Növbəti Tədbirlər
- Faza A iskele (snapshot xüsusiyyəti + telemetriya naqilləri) – yol xəritəsi yeniləmələrində icra edilə bilən tapşırıqlara baxın.
- `sumeragi`, `state` və `ivm` üçün son qüsur auditi aşağıdakı məqamları üzə çıxardı:
  - `sumeragi`: ölü kod müavinətləri baxış dəyişikliyinə sübut yayımını, VRF təkrar oxutma vəziyyətini və EMA telemetriya ixracını qoruyur. Faza C-nin konsensus axınının sadələşdirilməsi və zolaqlı/sübut inteqrasiyası ilə nəticələnənə qədər bunlar qapalı qalır.
  - `state`: `Cell` təmizləmə və telemetriya marşrutu Faza A WSV telemetriya yoluna keçir, SoA/paralel-tətbiq qeydləri isə Faza C boru kəmərinin optimallaşdırılması gecikməsinə qatlanır.
  - `ivm`: CUDA ekspozisiya, zərflərin yoxlanılması və Halo2/Metal əhatə dairəsi xəritəsini Faza D host-sərhəd işinə üstəgəl kəsişən GPU sürətləndirilməsi mövzusuna dəyişdirin; ləpələr hazır olana qədər xüsusi GPU-da qalır.
- İnvaziv kod dəyişikliklərinə enməzdən əvvəl bu planı imzalamaq üçün ümumiləşdirərək komandalararası RFC hazırlayın.

## 5. Açıq suallar
- RBC P1-dən sonra isteğe bağlı qalmalıdır, yoxsa Nexus kitabçası zolaqları üçün məcburidir? Maraqlı tərəflərin qərarını tələb edir.
- P1-də DS birləşmə qruplarını tətbiq edirik və ya zolaq sübutları yetişənə qədər onları əlil saxlayırıq?
- ML-DSA-87 parametrləri üçün kanonik yer nədir? Namizəd: yeni `crates/fastpq_isi` qutusu (yaradılışı gözləyir).

---

_Son yenilənmə: 2025-09-12_