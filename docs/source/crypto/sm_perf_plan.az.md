---
lang: az
direction: ltr
source: docs/source/crypto/sm_perf_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 493c3c0f6a991b2a5d04f33f97b7e97bff372271c5c57751ff41f5e86d43cbc7
source_last_modified: "2025-12-29T18:16:35.944695+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## SM Performans Çəkmə və Əsas Plan

Vəziyyəti: Hazırlanıb — 2025-05-18  
Sahiblər: Performans WG (aparıcı), İnfra Ops (laboratoriya planlaşdırması), QA Guild (CI qapısı)  
Əlaqədar yol xəritəsi tapşırıqları: SM-4c.1a/b, SM-5a.3b, FASTPQ Mərhələ 7 cihazlar arası çəkiliş

### 1. Məqsədlər
1. Neover medianı `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`-də qeyd edin. Cari əsas xətlər aarch64 macOS/Linux üçün 0,70-ə qədər genişləndirilmiş SM3 müqayisə tolerantlığı ilə `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (CPU etiketi `neoverse-proxy-macos`) altında `neoverse-proxy-macos` çəkilişindən ixrac edilir. Çılpaq metal vaxtı açıldıqda, Neoverse hostunda `scripts/sm_perf_capture_helper.sh --matrix --cpu-label neoverse-n2-b01 --output artifacts/sm_perf/<date>/neoverse-n2-b01`-i yenidən işə salın və ümumiləşdirilmiş medianı bazaya doğru irəliləyin.  
2. Uyğun x86_64 medianı toplayın ki, `ci/check_sm_perf.sh` hər iki host sinifini qoruya bilsin.  
3. Gələcək mükəmməl qapıların qəbilə biliklərinə etibar etməməsi üçün təkrarlana bilən tutma prosedurunu (əmrlər, artefakt tərtibatı, rəyçilər) dərc edin.

### 2. Avadanlığın mövcudluğu
Cari iş sahəsində yalnız Apple Silicon (macOS arm64) hostları əlçatandır. `neoverse-proxy-macos` çəkilişi müvəqqəti Linux bazası kimi ixrac edilir, lakin çılpaq metal Neoverse və ya x86_64 medianlarının çəkilməsi hələ də laboratoriya pəncərəsi açıldıqdan sonra Performans WG tərəfindən idarə edilməsi üçün `INFRA-2751` altında izlənilən paylaşılan laboratoriya aparatını tələb edir. Qalan tutma pəncərələri indi artefakt ağacında sifariş edilir və izlənilir:

- Neoverse N2 çılpaq metal (Tokio rack B) 2026-03-12 üçün sifariş edilib. Operatorlar Bölmə 3-dəki əmrləri təkrar istifadə edəcək və artefaktları `artifacts/sm_perf/2026-03-lab/neoverse-b01/` altında saxlayacaqlar.
- x86_64 Xeon (Sürix rack D) 2026-03-19 tarixində səs-küyü azaltmaq üçün SMT deaktiv edilib; artefaktlar `artifacts/sm_perf/2026-03-lab/xeon-d01/` altında düşəcək.
- Hər iki qaçışdan sonra əsas JSON-lara medianı təşviq edin və `ci/check_sm_perf.sh`-də CI qapısını aktivləşdirin (hədəf keçid tarixi: 2026-03-25).

Bu tarixlərə qədər yalnız macOS arm64 bazası yerli olaraq yenilənə bilər.### 3. Çəkmə Proseduru
1. **Alət zəncirlərini sinxronlaşdırın**  
   ```bash
   rustup override set $(cat rust-toolchain.toml)
   cargo fetch
   ```
2. **Çəkmə matrisi yaradın** (host üçün)  
   ```bash
   scripts/sm_perf_capture_helper.sh --matrix \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}
   ```
   Köməkçi indi hədəf kataloqu altında `capture_commands.sh` və `capture_plan.json` yazır. Skript hər rejimdə `raw/*.json` çəkmə yollarını təyin edir ki, laboratoriya texnikləri qaçışları deterministik şəkildə yığa bilsinlər.
3. **Çəkilişləri həyata keçirin**  
   `capture_commands.sh`-dən hər bir əmri yerinə yetirin (və ya ekvivalentini əl ilə işə salın), hər rejimin `--capture-json` vasitəsilə strukturlaşdırılmış JSON blobunu yaymasını təmin edin. Həmişə `--cpu-label "<model/bin>"` (və ya `SM_PERF_CPU_LABEL=<label>`) vasitəsilə host etiketini təmin edin ki, tutma metadata və sonrakı ilkin göstəricilər medianı yaradan dəqiq aparatı qeyd etsin. Köməkçi artıq müvafiq yolu təmin edir; əl ilə işləmələr üçün nümunə belədir:
   ```bash
   SM_PERF_CAPTURE_LABEL=auto \
   scripts/sm_perf.sh --mode auto \
     --cpu-label "neoverse-n2-lab-b01" \
     --capture-json artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/auto.json
   ```
4. **Nəticələri təsdiq edin**  
   ```bash
   scripts/sm_perf_check \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json
   ```
   Qaçışlar arasında fərqin ±3% daxilində qalmasına əmin olun. Əks halda, təsirlənmiş rejimi yenidən işə salın və jurnalda təkrar cəhdi qeyd edin.
5. **Medianı təşviq edin**  
   Medianları hesablamaq və onları baza JSON fayllarına köçürmək üçün `scripts/sm_perf_aggregate.py` istifadə edin:
   ```bash
   scripts/sm_perf_aggregate.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json
   ```
   Köməkçi qruplar `metadata.mode` tərəfindən ələ keçirərək, hər bir dəsti paylaşdığını təsdiqləyir.
   eyni `{target_arch, target_os}` üçlü və bir giriş ilə JSON xülasəsi yayır
   rejimə görə. Baza fayllarında yer almalı olan medianlar altında yaşayır
   `modes.<mode>.benchmarks`, onu müşayiət edən `statistics` blok qeydləri
   rəyçilər və CI üçün tam nümunə siyahısı, min/maks, orta və əhali stdev.
   Birləşdirilmiş fayl mövcud olduqdan sonra siz əsas JSON-ları avtomatik olaraq yaza bilərsiniz (
   standart tolerantlıq xəritəsi) vasitəsilə:
   ```bash
   scripts/sm_perf_promote_baseline.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json \
     --out-dir crates/iroha_crypto/benches \
     --target-os unknown_linux_gnu \
     --overwrite
   ```
   Alt çoxluqla məhdudlaşdırmaq üçün `--mode`-i ləğv edin və ya pin etmək üçün `--cpu-label`
   cəmlənmiş mənbə onu buraxdıqda qeydə alınmış CPU adı.
   Hər iki host hər arxitektura başa çatdıqdan sonra yeniləyin:
   - `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`
   - `sm_perf_baseline_x86_64_unknown_linux_gnu_{scalar,auto}.json` (yeni)

   `aarch64_unknown_linux_gnu_*` faylları indi `m3-pro-native`-i əks etdirir
   tutma (prosessor etiketi və metadata qeydləri qorunur) beləliklə `scripts/sm_perf.sh`
   aarch64-naməlum-linux-gnu hostlarını əl ilə işarələmədən avtomatik aşkar edin. Zaman
   çılpaq metal laboratoriyası işləməsi tamamlanır, `scripts/sm_perf.sh --mode  yenidən işə salın
   --write-baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_.json`
   aralıq medianların üzərinə yazmaq və realı möhürləmək üçün yeni çəkilişlərlə
   host etiketi.

   > İstinad: İyul 2025 Apple Silicon ələ keçirmə (CPU etiketi `m3-pro-local`)
   > `artifacts/sm_perf/2025-07-lab/takemiyacStudio.lan/{raw,aggregated.json}` altında arxivləşdirilmişdir.
   > Neoverse/x86 artefaktlarını dərc edərkən həmin planı əks etdirin
   > xammal/məcmu nəticələri ardıcıl olaraq fərqləndirə bilər.

### 4. Artefact Layout & Sign-off
```
artifacts/sm_perf/
  2025-07-lab/
    neoverse-b01/
      raw/
      aggregated.json
      run-log.md
    neoverse-b02/
      …
    xeon-d01/
    xeon-d02/
```
- `run-log.md` əmr hash, git revision, operator və hər hansı anomaliyaları qeyd edir.
- Birləşdirilmiş JSON faylları birbaşa əsas yeniləmələrə qidalanır və `docs/source/crypto/sm_perf_baseline_comparison.md`-də performans icmalına əlavə olunur.
- QA Gildiyası Performans bölməsi altında `status.md`-də əsaslar dəyişməzdən əvvəl artefaktları nəzərdən keçirir və imzalanır.### 5. CI Gating Timeline
| Tarix | Mərhələ | Fəaliyyət |
|------|-----------|--------|
| 2025-07-12 | Neoverse çəkilişləri tamamlandı | `sm_perf_baseline_aarch64_*` JSON fayllarını yeniləyin, yerli olaraq `ci/check_sm_perf.sh`-i işlədin, artefaktlar əlavə edilməklə PR açın. |
| 24-07-2025 | x86_64 tamamladı | `ci/check_sm_perf.sh`-də yeni baza faylları + qapılar əlavə edin; çarpaz qövs CI zolaqlarının onları istehlak etməsini təmin edin. |
| 27-07-2025 | CI tətbiqi | Hər iki host sinifində işləmək üçün `sm-perf-gate` iş axınını aktivləşdirin; reqressiyalar konfiqurasiya edilmiş tolerantlıqları aşarsa birləşmələr uğursuz olur. |

### 6. Asılılıqlar və Ünsiyyət
- `infra-ops@iroha.tech` vasitəsilə laboratoriyaya giriş dəyişikliklərini əlaqələndirin.  
- Performans WG, çəkilişlər işləyərkən gündəlik yeniləmələri `#perf-lab` kanalında yerləşdirir.  
- QA Gildiyası müqayisə fərqini (`scripts/sm_perf_compare.py`) hazırlayır ki, rəyçilər deltaları görüntüləyə bilsinlər.  
- Əsas xətlər birləşdirildikdən sonra çəkilişin tamamlanma qeydləri ilə `roadmap.md` (SM-4c.1a/b, SM-5a.3b) və `status.md`-i yeniləyin.

Bu planla SM sürətləndirmə işi “ehtiyat laboratoriya pəncərələri və tutma medianları” fəaliyyət elementini təmin edərək təkrarlana bilən medianlar, CI qapısı və izlənilə bilən sübut izi əldə edir.

### 7. CI Gate & Local Smoke

- `ci/check_sm_perf.sh` kanonik CI giriş nöqtəsidir. O, `SM_PERF_MODES`-də hər bir rejim üçün `scripts/sm_perf.sh`-ə çatır (defolt olaraq `scalar auto neon-force`) və `CARGO_NET_OFFLINE=true`-i təyin edir ki, skamyalar CI şəkillərində deterministik şəkildə işləsin.  
- `.github/workflows/sm-neon-check.yml` indi macOS arm64 runner-də darvaza çağırır ki, hər çəkmə sorğusu yerli olaraq istifadə edilən eyni köməkçi vasitəsilə skalyar/avtomatik/neon-güc üçlüyünü həyata keçirir; tamamlayıcı Linux/Neoverse zolağı x86_64 qurunu ələ keçirdikdən və Neoverse proksi bazası çılpaq metal qaçışla yeniləndikdən sonra qoşulacaq.  
- Operatorlar rejim siyahısını yerli olaraq ləğv edə bilər: `SM_PERF_MODES="scalar" bash ci/check_sm_perf.sh` sürətli tüstü testi üçün qaçışı tək keçidə kəsir, əlavə arqumentlər (məsələn, `--tolerance 0.20`) birbaşa `scripts/sm_perf.sh`-ə yönləndirilir.  
- `make check-sm-perf` indi tərtibatçının rahatlığı üçün qapını bağlayır; CI işləri birbaşa olaraq skripti işə sala bilər, eyni zamanda macOS tərtibatçıları istehsal hədəfinə arxalanırlar.  
- Neoverse/x86_64 əsas xəttləri endikdən sonra, eyni skript artıq `scripts/sm_perf.sh`-də mövcud olan host avtomatik aşkarlama məntiqi vasitəsilə müvafiq JSON-u götürəcək, ona görə də hər host hovuzu üçün istənilən rejim siyahısını təyin etməkdən başqa iş axınlarında əlavə naqillərə ehtiyac yoxdur.

### 8. Rüblük yeniləmə köməkçisi- `artifacts/sm_perf/2026-Q1/<label>/` kimi dörddəbir möhürlənmiş kataloq yaratmaq üçün `scripts/sm_perf_quarterly.sh --owner "<name>" --cpu-label "<label>" [--quarter YYYY-QN] [--output-root artifacts/sm_perf]`-i işə salın. Köməkçi `scripts/sm_perf_capture_helper.sh --matrix`-i əhatə edir və `capture_commands.sh`, `capture_plan.json` və `quarterly_plan.json` (sahibi + dörddəbir metadata) yayır ki, laboratoriya operatorları əl ilə yazılmış planlar olmadan qaçışları planlaşdıra bilsinlər.
- Yaradılmış `capture_commands.sh`-i hədəf hostda yerinə yetirin, xam çıxışları `scripts/sm_perf_aggregate.py --output <dir>/aggregated.json` ilə birləşdirin və medianı `scripts/sm_perf_promote_baseline.py --out-dir crates/iroha_crypto/benches --overwrite` vasitəsilə əsas JSON-lara təşviq edin. Toleransların yaşıl qalmasını təsdiqləmək üçün `ci/check_sm_perf.sh`-i yenidən işə salın.
- Aparat və ya alətlər zəncirləri dəyişdikdə, `docs/source/crypto/sm_perf_baseline_comparison.md`-də müqayisə dözümlülüklərini/qeydlərini yeniləyin, yeni medianlar sabitləşərsə, `ci/check_sm_perf.sh` toleranslarını sərtləşdirin və hər hansı idarə paneli/xəbərdarlıq hədlərini yeni əsas xətlərlə uyğunlaşdırın ki, əməliyyat siqnalları mənalı olsun.
- Əsas yeniləmələrlə yanaşı `quarterly_plan.json`, `capture_plan.json`, `capture_commands.sh` və ümumiləşdirilmiş JSON-u qəbul edin; eyni artefaktları izlənilmək üçün status/yol xəritəsi yeniləmələrinə əlavə edin.