---
lang: az
direction: ltr
source: docs/source/iroha_monitor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05149d624d680d04433be41a4525538c97bd103ae7f80dda2613a6adb181a93d
source_last_modified: "2025-12-29T18:16:35.968850+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha Monitor

Yenidən işlənmiş Iroha monitoru yüngül terminal UI-ni animasiya ilə birləşdirir
festival ASCII sənəti və ənənəvi Etenraku mövzusu.  İkiyə diqqət yetirir
sadə iş prosesləri:

- **Spawn-lite rejimi** – həmyaşıdları təqlid edən efemer status/metrik stubları başladın.
- **Əlavə rejimi** – monitoru mövcud Torii HTTP son nöqtələrinə yönəldin.

UI hər yeniləmədə üç bölgəni təqdim edir:

1. **Torii səma xətti başlığı** – animasiyalı torii qapısı, Fuji dağı, koi dalğaları və ulduz
   təzələmə kadansı ilə sinxron fırlanan sahə.
2. **Xülasə zolağı** – yığılmış bloklar/əməliyyatlar/qaz üstəgəl yeniləmə vaxtı.
3. **Həmyaşıdlar masası və festival pıçıltıları** – solda həmyaşıd sıraları, fırlanan tədbir
   Xəbərdarlıqları tutan sağda daxil olun (faydalar, böyük ölçülü yüklər və s.).
4. **İstəyə görə qaz trendi** – parıldamaq üçün `--show-gas-trend`-i aktiv edin
   bütün həmyaşıdları arasında ümumi qaz istifadəsinin ümumiləşdirilməsi.

Bu refaktorda yeni:

- Koi, torii və fənərlərlə animasiya edilmiş Yapon üslublu ASCII səhnəsi.
- Sadələşdirilmiş əmr səthi (`--spawn-lite`, `--attach`, `--interval`).
- Gagaku mövzusunun isteğe bağlı audio səsləndirilməsi ilə giriş banneri (xarici MIDI
  pleyer və ya platforma/audio yığını onu dəstəklədikdə daxili yumşaq sintez).
- CI və ya sürətli tüstü qaçışları üçün `--no-theme` / `--no-audio` bayraqları.
- Ən son xəbərdarlığı, icra müddətini və ya iş vaxtını göstərən “əhval” sütunu.

## Sürətli başlanğıc

Monitoru qurun və onu darmadağın edilmiş həmyaşıdlara qarşı işlədin:

```bash
cargo run -p iroha_monitor -- --spawn-lite --peers 3
```

Mövcud Torii son nöqtələrinə əlavə edin:

```bash
cargo run -p iroha_monitor -- \
  --attach http://127.0.0.1:8080 http://127.0.0.1:8081 \
  --interval 500
```

CI dostu çağırış (giriş animasiyasını və audionu keçin):

```bash
cargo run -p iroha_monitor -- --spawn-lite --no-theme --no-audio
```

### CLI bayraqları

```
--spawn-lite         start local status/metrics stubs (default if no --attach)
--attach <URL...>    attach to existing Torii endpoints
--interval <ms>      refresh interval (default 800ms)
--peers <N>          stub count when spawn-lite is active (default 4)
--no-theme           skip the animated intro splash
--no-audio           mute theme playback (still prints the intro frames)
--midi-player <cmd>  external MIDI player for the built-in Etenraku .mid
--midi-file <path>   custom MIDI file for --midi-player
--show-gas-trend     render the aggregate gas sparkline panel
--art-speed <1-8>    multiply the animation step rate (1 = default)
--art-theme <name>   choose between night, dawn, or sakura palettes
--headless-max-frames <N>
                     cap headless fallback to N frames (0 = unlimited)
```

## Mövzu girişi

Varsayılan olaraq, başlanğıc Etenraku hesab edərkən qısa ASCII animasiyasını oynayır
başlayır.  Audio seçim qaydası:

1. `--midi-player` təmin edilibsə, demo MIDI yaradın (və ya `--midi-file` istifadə edin)
   və əmri verin.
2. Əks halda, macOS/Windows-da (və ya `--features iroha_monitor/linux-builtin-synth` ilə Linux)
   daxili gagaku soft synth ilə hesabı göstərin (xarici audio yoxdur
   aktivlər tələb olunur).
3. Səs söndürülübsə və ya işə salınma uğursuz olarsa, giriş hələ də onu çap edir
   animasiya və dərhal TUI-yə daxil olur.

CPAL ilə işləyən sintezator macOS və Windows-da avtomatik işə salınır. Linux-da belədir
iş sahəsinin qurulması zamanı ALSA/Pulse başlıqlarını itirməmək üçün daxil olun; imkan verin
sisteminiz təmin edərsə, `--features iroha_monitor/linux-builtin-synth` ilə
işləyən audio yığını.

CI və ya başsız mərmilərdə işləyərkən `--no-theme` və ya `--no-audio` istifadə edin.

Yumşaq sintez indi *MIDI synth dizaynında çəkilmiş tənzimləməni izləyir
Rust.pdf*: hichiriki və ryūteki şo zamanı heterofonik melodiya paylaşır
sənəddə təsvir olunan aitake yastıqlarını təmin edir.  Müddətli qeyd datası yaşayır
`etenraku.rs`-də; o, həm CPAL geri çağırışını, həm də yaradılan demo MIDI-ni gücləndirir.
Səs çıxışı mümkün olmadıqda, monitor oxutmadan keçir, lakin yenə də göstərir
ASCII animasiyası.

## UI icmalı- **Başlıq sənəti** – hər bir çərçivə `AsciiAnimator` tərəfindən yaradılıb; koi, torii fənərləri,
  və dalğalar davamlı hərəkət etmək üçün sürüklənir.
- **Xülasə zolağı** – onlayn həmyaşıdları, bildirilən həmyaşıdların sayını, blokların cəmini,
  boş olmayan blok cəmləri, tx təsdiqləri/rəddləri, qaz istifadəsi və yeniləmə dərəcəsi.
- **Peer cədvəli** – ləqəb/son nöqtə, bloklar, əməliyyatlar, növbə ölçüsü üçün sütunlar,
  qazdan istifadə, gecikmə və “əhval-ruhiyyə” işarəsi (xəbərdarlıqlar, icra vaxtı, iş vaxtı).
- **Festival pıçıltıları** – xəbərdarlıqların yuvarlanan jurnalı (bağlantı xətaları, faydalı yük)
  məhdudiyyət pozuntuları, yavaş son nöqtələr).  Mesajlar tərsinə çevrilir (ən son yuxarıda).

Klaviatura qısa yolları:

- `n` / Sağ / Aşağı – diqqəti növbəti həmyaşıdına köçürün.
- `p` / Sol / Yuxarı – diqqəti əvvəlki həmyaşıdına köçürün.
- `q` / Esc / Ctrl-C – terminaldan çıxın və bərpa edin.

Monitor alternativ ekran buferi ilə krossterm + ratatui istifadə edir; çıxışda
kursoru bərpa edir və ekranı təmizləyir.

## Duman testləri

Sandıq həm rejimi, həm də HTTP məhdudiyyətlərini tətbiq edən inteqrasiya testlərini göndərir:

- `spawn_lite_smoke_renders_frames`
- `attach_mode_with_stubs_runs_cleanly`
- `invalid_endpoint_surfaces_warning`
- `status_limit_warning_is_rendered`
- `attach_mode_with_slow_peer_renders_multiple_frames`

Yalnız monitor testlərini yerinə yetirin:

```bash
cargo test -p iroha_monitor -- --nocapture
```

İş yerində daha ağır inteqrasiya testləri var (`cargo test --workspace`). Qaçış
monitor testləri ayrıca etdiyiniz zaman tez doğrulama üçün hələ də faydalıdır
tam paketə ehtiyac yoxdur.

## Ekran görüntüləri yenilənir

Sənədlərin nümayişi indi torii siluetinə və həmyaşıdlar cədvəlinə diqqət yetirir.  Yeniləmək üçün
aktivlər, run:

```bash
make monitor-screenshots
```

Bu, `scripts/iroha_monitor_demo.sh` (kürü-lite rejimi, sabit toxum/görüntü pəncərəsi,
intro/audio yoxdur, şəfəq palitrası, art-speed 1, başsız qapaq 24) və yazır
SVG/ANSI çərçivələri üstəgəl `manifest.json` və `checksums.json`
`docs/source/images/iroha_monitor_demo/`. `make check-iroha-monitor-docs`
hər iki CI qoruyucularını (`ci/check_iroha_monitor_assets.sh` və
`ci/check_iroha_monitor_screenshots.sh`) beləliklə generator hashləri, manifest sahələri,
və yoxlama məbləğləri sinxron qalır; ekran görüntüsü yoxlanışı da olaraq göndərilir
`python3 scripts/check_iroha_monitor_screenshots.py`. `--no-fallback`-ə keçin
demo skriptini geri qaytarmaq əvəzinə tutmağın uğursuz olmasını istəyirsinizsə
monitor çıxışı boş olduqda bişmiş çərçivələr; ehtiyat xammaldan istifadə edildikdə
`.ans` faylları bişmiş çərçivələrlə yenidən yazılır ki, manifest/yoxlama məbləğləri qalır
deterministik.

## Deterministik ekran görüntüləri

Göndərilən görüntülər `docs/source/images/iroha_monitor_demo/`-də canlıdır:

![monitor icmalı](images/iroha_monitor_demo/iroha_monitor_demo_overview.svg)
![boru kəmərinə nəzarət edin](images/iroha_monitor_demo/iroha_monitor_demo_pipeline.svg)

Onları sabit görünüş pəncərəsi/toxumu ilə təkrarlayın:

```bash
scripts/iroha_monitor_demo.sh \
  --cols 120 --rows 48 \
  --interval 500 \
  --seed iroha-monitor-demo
```

Çəkmə köməkçisi `LANG`/`LC_ALL`/`TERM`, irəli istiqamətləri düzəldir
`IROHA_MONITOR_DEMO_SEED`, səsi söndürür və incəsənət mövzusunu/sürətini sabitləyir
çərçivələr platformalar arasında eyni şəkildə göstərilir. `manifest.json` yazır (generator
heshlər + ölçülər) və `checksums.json` (SHA-256 həzmləri) altında
`docs/source/images/iroha_monitor_demo/`; CI çalışır
`ci/check_iroha_monitor_assets.sh` və `ci/check_iroha_monitor_screenshots.sh`
aktivlər qeydə alınmış manifestlərdən kənara çıxdıqda uğursuzluq.

## Problemlərin aradan qaldırılması- **Audio çıxışı yoxdur** – monitor yenidən səssiz oxutma rejiminə keçir və davam edir.
- **Başsız geri dönmə erkən çıxır** – monitor başsız qaçışları bir cütə bağlayır
  dəyişə bilməyəndə onlarla kadr (standart intervalda təxminən 12 saniyə).
  terminalı xam rejimə keçirin; onu davam etdirmək üçün `--headless-max-frames 0` keçir
  qeyri-müəyyən müddətə.
- **Böyük ölçülü status yükləri** – həmyaşıdların əhval-ruhiyyə sütunu və festival jurnalı
  konfiqurasiya edilmiş limitlə (`128 KiB`) `body exceeds …` göstərin.
- **Yavaş həmyaşıdlar** – hadisə jurnalı vaxt aşımı xəbərdarlıqlarını qeyd edir; həmyaşıdına diqqət yetirin
  sıranı vurğulayın.

Festival siluetindən həzz alın!  Əlavə ASCII motivləri üçün töhfələr və ya
metrik panellər xoşdur - onları deterministik saxlayın ki, klasterlər eyni olsun
terminaldan asılı olmayaraq çərçivə çərçivə.