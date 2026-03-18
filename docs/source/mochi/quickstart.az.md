---
lang: az
direction: ltr
source: docs/source/mochi/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44faf6c98d141959cf8cf40b1df7d3d82c3448e6f2b1bc4fa54cdeceb97994b0
source_last_modified: "2025-12-29T18:16:35.985408+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# MOCHI Sürətli Başlanğıc

**MOCHI** yerli Hyperledger Iroha şəbəkələri üçün iş masası nəzarətçisidir. Bu bələdçi keçir
ilkin şərtlərin quraşdırılması, tətbiqin qurulması, egui qabığının işə salınması və istifadə edilməsi
gündəlik inkişaf üçün işləmə zamanı alətləri (parametrlər, anlıq görüntülər, silmələr).

## İlkin şərtlər

- Rust alətlər silsiləsi: `rustup default stable` (iş sahəsi hədəfləri nəşr 2024 / Rust 1.82+).
- Platforma alətlər silsiləsi:
  - macOS: Xcode Komanda Xətti Alətləri (`xcode-select --install`).
  - Linux: GCC, pkg-config, OpenSSL başlıqları (`sudo apt install build-essential pkg-config libssl-dev`).
- Iroha iş sahəsindən asılılıqlar:
  - `cargo xtask mochi-bundle` qurulmuş `irohad`, `kagami` və `iroha_cli` tələb edir. Onları bir dəfə vasitəsilə qurun
    `cargo build -p irohad -p kagami -p iroha_cli`.
- İsteğe bağlı: `direnv` və ya `cargo binstall` yerli yük ikililərini idarə etmək üçün.

MOCHI, CLI ikili fayllarına cavab verir. Onların ətraf mühit dəyişənləri vasitəsilə aşkar oluna biləcəyinə əmin olun
aşağıda və ya PATH-də mövcuddur:

| İkili | Ətraf mühitin ləğvi | Qeydlər |
|----------|----------------------|-----------------------------------------|
| `irohad` | `MOCHI_IROHAD` | Həmyaşıdlarına nəzarət edir |
| `kagami` | `MOCHI_KAGAMI` | Genesis manifestləri/snapshots yaradır |
| `iroha_cli` | `MOCHI_IROHA_CLI` | Qarşıdan gələn köməkçi funksiyalar üçün könüllü |

## MOCHI binası

Repozitor kökündən:

```bash
cargo build -p mochi-ui-egui
```

Bu əmr həm `mochi-core`, həm də egui frontendini qurur. Paylana bilən paket yaratmaq üçün aşağıdakıları yerinə yetirin:

```bash
cargo xtask mochi-bundle
```

Paket tapşırığı `target/mochi-bundle` altında ikili faylları, manifestləri və konfiqurasiya stublarını toplayır.

## Egui qabığının işə salınması

UI-ni birbaşa yükdən işə salın:

```bash
cargo run -p mochi-ui-egui
```

Varsayılan olaraq MOCHI müvəqqəti məlumat qovluğunda tək bərabərli ilkin təyinat yaradır:

- Məlumat kökü: `$TMPDIR/mochi`.
- Torii əsas port: `8080`.
- P2P baza portu: `1337`.

Başladıqda defoltları ləğv etmək üçün CLI bayraqlarından istifadə edin:

```bash
cargo run -p mochi-ui-egui -- \
  --data-root /path/to/workspace \
  --profile four-peer-bft \
  --torii-start 12000 \
  --p2p-start 13000 \
  --kagami /path/to/kagami \
  --irohad /path/to/irohad
```

CLI bayraqları buraxıldıqda ətraf mühit dəyişənləri eyni ləğvetmələri əks etdirir: `MOCHI_DATA_ROOT` təyin edin,
`MOCHI_PROFILE`, `MOCHI_CHAIN_ID`, `MOCHI_TORII_START`, `MOCHI_P2P_START`, `MOCHI_RESTART_MODE`,
`MOCHI_RESTART_MAX` və ya `MOCHI_RESTART_BACKOFF_MS` nəzarətçi qurucusunu əvvəlcədən təyin etmək; ikili yollar
`MOCHI_IROHAD`/`MOCHI_KAGAMI`/`MOCHI_IROHA_CLI` və `MOCHI_CONFIG` nöqtələrinə hörmət etməyə davam edin.
açıq `config/local.toml`.

## Parametrlər və davamlılıq

Nəzarətçinin konfiqurasiyasını tənzimləmək üçün tablosuna alətlər panelindən **Parametrlər** dialoqunu açın:

- **Məlumat kökü** — həmyaşıd konfiqurasiyalar, yaddaş, qeydlər və snapshotlar üçün əsas kataloq.
- **Torii / P2P baza portları** — deterministik ayırma üçün başlanğıc portlar.
- **Log görünürlüğü** — jurnala baxıcıda stdout/stderr/sistem kanallarını dəyişdirin.

Nəzarətçinin yenidən başlatma siyasəti kimi qabaqcıl düymələr mövcuddur
`config/local.toml`. `[supervisor.restart] mode = "never"`-i söndürmək üçün təyin edin
hadisənin aradan qaldırılması zamanı avtomatik yenidən başladın və ya tənzimləyin
`max_restarts`/`backoff_ms` (ya konfiqurasiya faylı və ya CLI bayraqları vasitəsilə
Yenidən cəhdə nəzarət etmək üçün `--restart-mode`, `--restart-max`, `--restart-backoff-ms`)
davranış.Dəyişikliklərin tətbiqi nəzarətçini yenidən qurur, işləyən hər hansı həmyaşıdları yenidən işə salır və əvəzsiz təyinatları yazır.
`config/local.toml`. Konfiqurasiya birləşməsi əlaqəli olmayan açarları qoruyur ki, qabaqcıl istifadəçilər saxlaya bilsinlər
MOCHI tərəfindən idarə olunan dəyərlərlə yanaşı əl ilə düzəlişlər.

## Ani görüntülər və silin/yenidən yaradılış

**Maintenance** dialoqu iki təhlükəsizlik əməliyyatını nümayiş etdirir:

- **İxrac snapshot** — həmyaşıd yaddaşı/konfiqurasiyanı/loqları və cari genezisi nüsxə edir
  Aktiv məlumat kökü altında `snapshots/<label>`. Etiketlər avtomatik olaraq təmizlənir.
- **Snapşotu bərpa edin** — həmyaşıd yaddaşını, snapshot köklərini, konfiqurasiyaları, qeydləri və genezisi yenidən nəmləndirir
  mövcud paketdən manifest. `Supervisor::restore_snapshot` ya mütləq yolu, ya da qəbul edir
  sanitarlaşdırılmış `snapshots/<label>` qovluq adı; UI bu axını əks etdirir, ona görə də Maintenance → Restore
  sənədlərə əl ilə toxunmadan sübut paketlərini təkrarlaya bilər.
- **Silin və yenidən yaradılış** — həmyaşıdların işləməsini dayandırır, yaddaş qovluqlarını silir, genezisi bərpa edir
  Kagami və silmə tamamlandıqda həmyaşıdları yenidən işə salır.

Hər iki axın reqressiya testləri ilə əhatə olunur (`export_snapshot_captures_storage_and_metadata`,
`wipe_and_regenerate_resets_storage_and_genesis`) deterministik nəticələri təmin etmək üçün.

## Qeydlər və axınlar

İdarə paneli məlumat/ölçmələri bir baxışda göstərir:

- **Logs** — `irohad` stdout/stderr/sistem həyat dövrü mesajlarını izləyir. Parametrlərdə kanalları dəyişin.
- **Bloklar / Hadisələr** — idarə olunan axınlar eksponensial geri çəkilmə və annotasiya çərçivələri ilə avtomatik yenidən bağlanır
  Norito-deşifrə edilmiş xülasələrlə.
- **Status** — `/status` sorğusunu aparır və növbənin dərinliyi, ötürmə qabiliyyəti və gecikmə müddəti üçün sparklines göstərir.
- **Başlanğıc hazırlığı** — **Start** (tək həmyaşıd və ya bütün həmyaşıdlar) düyməsini basdıqdan sonra MOCHI zondları
  Məhdud geriləmə ilə `/status`; pankart hər bir həmyaşıd hazır olduqda (müşahidə olunan
  növbənin dərinliyi) və ya hazır olma vaxtı keçərsə, Torii xətasını göstərir.

Dövlət tədqiqatçısı və bəstəkar üçün nişanlar hesablara, aktivlərə, həmyaşıdlara və ümumi məlumatlara sürətli girişi təmin edir
UI-dən çıxmadan təlimatlar. Peers görünüşü `FindPeers` sorğusunu əks etdirir ki, siz təsdiq edə biləsiniz
inteqrasiya testlərini keçirməzdən əvvəl hansı açıq açarların hazırda validator dəstində qeydiyyata alındığını.

İmzalama orqanlarını idxal etmək və ya redaktə etmək üçün bəstəkar alətlər panelinin **İmzalama kassasını idarə et** düyməsini istifadə edin. The
dialoq aktiv şəbəkə kökünə girişlər yazır (`<data_root>/<profile>/signers.json`) və yadda saxlanılır
anbar açarları əməliyyatların önizləmələri və təqdimatlar üçün dərhal əlçatandır. Kassa olanda
boş olduqda, bəstəkar birləşdirilmiş inkişaf düymələrinə qayıdır ki, yerli iş axınları işləməyə davam edir.
Formalar indi nanə/yandırma/köçürmə (o cümlədən gizli qəbul), domen/hesab/aktiv tərifini əhatə edir
qeydiyyat, hesaba giriş siyasətləri, multisig təklifləri, Space Directory manifestləri (AXT/AMX),
SoraFS pin manifestləri və rolların verilməsi və ya ləğv edilməsi kimi idarəetmə tədbirləri çox yayılmışdır
yol xəritəsinin müəllifi tapşırıqları Norito faydalı yükləri əl ilə yazmadan məşq edilə bilər.

## Təmizləmə və problemlərin aradan qaldırılması- Nəzarət olunan həmyaşıdları ləğv etmək üçün tətbiqi dayandırın.
- Bütün vəziyyəti sıfırlamaq üçün məlumat kökünü (`rm -rf <data_root>`) çıxarın.
- Kagami və ya irohad yerləri dəyişərsə, mühit dəyişənlərini yeniləyin və ya MOCHI-ni yenidən işə salın
  müvafiq CLI bayraqları; Parametrlər dialoqu növbəti tətbiqdə yeni yolları saxlayacaqdır.

Əlavə avtomatlaşdırma yoxlaması üçün `mochi/mochi-core/tests` (nəzarətçinin həyat dövrü testləri) və
istehza edilmiş Torii ssenariləri üçün `mochi/mochi-integration`. Paketləri göndərmək və ya tel çəkmək üçün
masaüstünü CI boru kəmərlərinə daxil etmək üçün {doc}`mochi/packaging` bələdçisinə baxın.

## Yerli sınaq qapısı

Yamaqlar göndərməzdən əvvəl `ci/check_mochi.sh`-i işə salın ki, paylaşılan CI qapısı hər üç MOCHI-ni məşq etsin.
yeşiklər:

```bash
./ci/check_mochi.sh
```

Köməkçi `cargo check`/`cargo test`, `mochi-core`, `mochi-ui-egui` və
`mochi-integration`, armatur sürüşməsini (kanonik blok/hadisə çəkilişləri) və egui qoşqularını tutan
bir atışda reqressiyalar. Skript köhnəlmiş qurğular haqqında məlumat verirsə, nəzərə alınmayan bərpa testlərini yenidən həyata keçirin,
məsələn:

```bash
cargo test -p mochi-core regenerate_block_wire_fixture -- --ignored
```

Yenidən yaratdıqdan sonra qapının yenidən işə salınması yenilənmiş baytların siz basmadan əvvəl ardıcıl qalmasını təmin edir.