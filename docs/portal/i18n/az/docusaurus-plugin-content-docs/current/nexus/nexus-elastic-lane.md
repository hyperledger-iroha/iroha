---
id: nexus-elastic-lane
lang: az
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Elastic lane provisioning (NX-7)
sidebar_label: Elastic Lane Provisioning
description: Bootstrap workflow for creating Nexus lane manifests, catalog entries, and rollout evidence.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
Bu səhifə `docs/source/nexus_elastic_lane.md`-i əks etdirir. Tərcümə portala düşənə qədər hər iki nüsxəni düzülmüş saxlayın.
:::

# Elastik Zolaq Təminetmə Alətlər dəsti (NX-7)

> **Yol xəritəsi elementi:** NX-7 — Elastik zolaq təmin edən alətlər  
> **Status:** Alət tamamlandı — manifestlər, kataloq parçaları, Norito yükləri, tüstü testləri,
> və yükləmə testi paketinin köməkçisi indi yuvanın gecikmə qapağını tikir + sübut belə təsdiqləyici göstərir
> yükləmə işləri sifarişli skript olmadan dərc edilə bilər.

Bu təlimat operatorları avtomatlaşdıran yeni `scripts/nexus_lane_bootstrap.sh` köməkçisi vasitəsilə gəzdirir.
zolaqlı manifest generasiyası, zolaq/dataspace kataloq fraqmentləri və yayılma sübutları. Məqsəd etməkdir
birdən çox faylı əl ilə redaktə etmədən yeni Nexus zolaqlarını (ictimai və ya özəl) fırlatmaq asandır.
kataloq həndəsəsini əl ilə yenidən çıxarmaq.

## 1. İlkin şərtlər

1. Zolaq ləqəbi, məlumat məkanı, validator dəsti, nasazlığa dözümlülük (`f`) və hesablaşma siyasəti üçün idarəetmə təsdiqi.
2. Tamamlanmış təsdiqləyici siyahısı (hesab identifikatorları) və qorunan ad sahəsi siyahısı.
3. Yaradılmış fraqmentləri əlavə etmək üçün node konfiqurasiya repozitoriyasına daxil olun.
4. Zolaqlı manifest reyestri üçün yollar (bax: `nexus.registry.manifest_directory` və
   `cache_directory`).
5. Zolaq üçün telemetriya kontaktları/PagerDuty tutacaqları beləliklə, siqnallar zolağa çıxan kimi ötürülə bilər
   onlayn gəlir.

## 2. Zolaq artefaktları yaradın

Repository kökündən köməkçini işə salın:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Əsas bayraqlar:

- `--lane-id` `nexus.lane_catalog`-də yeni qeydin indeksinə uyğun olmalıdır.
- `--dataspace-alias` və `--dataspace-id/hash` məlumat məkanı kataloquna girişi idarə edir (defolt olaraq
  buraxıldıqda zolaq identifikatoru).
- `--validator` təkrarlana və ya `--validators-file`-dən əldə edilə bilər.
- `--route-instruction` / `--route-account` yapışdırmağa hazır marşrutlaşdırma qaydalarını yayır.
- `--metadata key=value` (və ya `--telemetry-contact/channel/runbook`) runbook kontaktlarını ələ keçirin.
  tablosuna dərhal doğru sahibləri siyahıya alır.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` manifestə iş vaxtı təkmilləşdirmə çəngəlini əlavə edin
  zolaq uzadılmış operator nəzarətini tələb etdikdə.
- `--encode-space-directory` `cargo xtask space-directory encode`-i avtomatik çağırır. ilə cütləşdirin
  `--space-directory-out` kodlanmış `.to` faylını standartdan başqa yerdə istədiyiniz zaman.

Skript `--output-dir` daxilində üç artefakt yaradır (defolt olaraq cari qovluq üçün),
kodlaşdırma aktiv olduqda əlavə dördüncü:

1. `<slug>.manifest.json` — təsdiqləyici kvorumu, qorunan ad boşluqlarını və ehtiva edən zolaqlı manifest
   isteğe bağlı iş vaxtı təkmilləşdirmə çəngəl metadatası.
2. `<slug>.catalog.toml` — `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` ilə TOML fraqmenti,
   və istənilən marşrutlaşdırma qaydaları. `fault_tolerance`-in məlumat məkanı girişində ölçüyə uyğun olduğundan əmin olun
   zolaqlı relay komitəsi (`3f+1`).
3. `<slug>.summary.json` — həndəsəni (slug, seqmentlər, metaməlumatlar) təsvir edən audit xülasəsi
   tələb olunan buraxılış addımları və dəqiq `cargo xtask space-directory encode` əmri (
   `space_directory_encode.command`). Sübut üçün bu JSON-u uçuş biletinə əlavə edin.
4. `<slug>.manifest.to` — `--encode-space-directory` təyin edildikdə yayılır; Torii üçün hazırdır
   `iroha app space-directory manifest publish` axını.

Faylları yazmadan JSON/ fraqmentlərinə baxmaq üçün `--dry-run`, üzərinə yazmaq üçün isə `--force` istifadə edin.
mövcud artefaktlar.

## 3. Dəyişiklikləri tətbiq edin

1. Manifest JSON-u konfiqurasiya edilmiş `nexus.registry.manifest_directory`-ə (və keş- yaddaşa) kopyalayın
   reyestr uzaq paketləri əks etdirirsə qovluq). Manifestlər versiyadadırsa, faylı daxil edin
   konfiqurasiya repo.
2. Kataloq parçasını `config/config.toml` (və ya müvafiq `config.d/*.toml`) əlavə edin. təmin etmək
   `nexus.lane_count` ən azı `lane_id + 1`-dir və istənilən `nexus.routing_policy.rules`-i yeniləyin
   yeni zolağa işarə etməlidir.
3. Kodlaşdırın (əgər siz `--encode-space-directory`-i atlasanız) və manifesti Kosmik Kataloqda dərc edin
   xülasədə çəkilmiş əmrdən istifadə edərək (`space_directory_encode.command`). Bu istehsal edir
   `.manifest.to` faydalı yük Torii auditorlar üçün sübut gözləyir və qeyd edir; vasitəsilə təqdim edin
   `iroha app space-directory manifest publish`.
4. `irohad --sora --config path/to/config.toml --trace-config`-i işə salın və iz çıxışını arxivləşdirin
   buraxılış bileti. Bu, yeni həndəsənin yaradılan şlak/kura seqmentlərinə uyğun olduğunu sübut edir.
5. Manifest/kataloq dəyişiklikləri tətbiq edildikdən sonra zolağa təyin edilmiş validatorları yenidən işə salın. Saxla
   gələcək auditlər üçün biletdəki xülasə JSON.

## 4. Reyestr paylama paketini yaradın

Yaradılmış manifest və üst-üstə düşməni paketləyin ki, operatorlar zolaqlı idarəetmə məlumatlarını olmadan paylaya bilsinlər
hər hostda konfiqurasiyaları redaktə etmək. Bundler köməkçisi manifestləri kanonik tərtibata köçürür,
`nexus.registry.cache_directory` üçün isteğe bağlı idarəetmə kataloqu örtüyü istehsal edir və
oflayn köçürmələr üçün tarball:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Çıxışlar:

1. `manifests/<slug>.manifest.json` — bunları konfiqurasiya edilmişlərə kopyalayın
   `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` — `nexus.registry.cache_directory`-ə düşür. Hər `--module`
   giriş qoşula bilən modul tərifinə çevrilərək idarəetmə modulunun dəyişdirilməsinə (NX-2) imkan verir.
   `config.toml`-i redaktə etmək əvəzinə keş örtüyünün yenilənməsi.
3. `summary.json` — heşlər, üst-üstə düşən metadata və operator təlimatları daxildir.
4. Könüllü `registry_bundle.tar.*` — SCP, S3 və ya artefakt izləyiciləri üçün hazırdır.

Bütün kataloqu (və ya arxivi) hər bir təsdiqləyici ilə sinxronlaşdırın, hava boşluğu olan hostlarda çıxarın və kopyalayın
manifestlər + Torii-i yenidən başlatmazdan əvvəl onların reyestr yollarına keş örtüyü.

## 5. Validator tüstü testləri

Torii yenidən işə salındıqdan sonra, `manifest_ready=true` zolaq hesabatlarını yoxlamaq üçün yeni tüstü köməkçisini işə salın,
ölçülər gözlənilən zolaq sayını göstərir və möhürlənmiş ölçü aydındır. Manifest tələb edən zolaqlar
boş olmayan `manifest_path`-i ifşa etməlidir; yol belə itkin zaman köməkçi indi dərhal uğursuz
hər NX-7 yerləşdirmə qeydinə imzalanmış açıq sübutlar daxildir:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Öz-özünə imzalanmış mühitləri sınaqdan keçirərkən `--insecure` əlavə edin. Zolaq varsa, skript sıfırdan çıxır
gözlənilən dəyərlərdən əskik, möhürlənmiş və ya ölçülər/telemetriya sürüşməsi. istifadə edin
`--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` və
`--max-headroom-events` düymələri hər zolaqlı blokun hündürlüyünü/sonluğu/arxa planı/baxış yeri telemetriyasını saxlamaq üçün
əməliyyat zərfləriniz daxilində və onları `--max-slot-p95` / `--max-slot-p99` ilə birləşdirin
(plus `--min-slot-samples`) köməkçidən ayrılmadan NX‑18 slot müddəti hədəflərini tətbiq etmək.

Hava boşluqlu doğrulamalar (və ya CI) üçün canlı yayım yerinə vurmaq əvəzinə çəkilmiş Torii cavabını təkrarlaya bilərsiniz.
son nöqtə:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`fixtures/nexus/lanes/` altında qeydə alınmış qurğular bootstrap tərəfindən istehsal olunan artefaktları əks etdirir
köməkçi, belə ki, yeni manifestlər sifarişli skript olmadan rənglənə bilər. CI vasitəsilə eyni axını həyata keçirir
`ci/check_nexus_lane_smoke.sh` və `ci/check_nexus_lane_registry_bundle.sh`
(ləqəb: `make check-nexus-lanes`) NX-7 tüstü yardımçısının dərc edilmiş sənədlərlə uyğunlaşdığını sübut etmək üçün
faydalı yük formatı və paketin həzmlərinin/örtmələrinin təkrarlana bilən qalmasını təmin etmək.

Zolağın adı dəyişdirildikdə, `nexus.lane.topology` telemetriya hadisələrini çəkin (məsələn,
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) və onları geri qaytarın
tüstü köməkçisi. `--telemetry-file/--from-telemetry` bayrağı yeni sətirlə ayrılmış jurnalı qəbul edir və
`--require-alias-migration old:new` iddia edir ki, `alias_migrated` hadisəsi adının dəyişdirilməsini qeyd edib:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

`telemetry_alias_migrated.ndjson` qurğusu, CI-nin yoxlaya bilməsi üçün kanonik adlandırma nümunəsini birləşdirir
canlı node ilə əlaqə olmadan telemetriya təhlili yolu.

## Validator yük testləri (NX-7 sübutu)

Yol xəritəsi **NX-7**, təkrarlana bilən validator yük əməliyyatını göndərmək üçün hər yeni zolağı tələb edir. istifadə edin
`scripts/nexus_lane_load_test.py` tüstü yoxlamalarını, yuva müddəti qapılarını və yuva paketini tikmək üçün
idarəetmənin təkrarlaya biləcəyi tək artefakt dəstinə çevrilir:

```bash
scripts/nexus_lane_load_test.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --slot-range 81200-81600 \
  --workload-seed NX7-PAYMENTS-2026Q2 \
  --require-alias-migration core:payments \
  --out-dir artifacts/nexus/load/payments-2026q2
```

Köməkçi eyni DA kvorumunu, oracle, hesablaşma buferini, TEU və slot müddəti qapılarını tətbiq edir.
tüstü köməkçisi tərəfindən və `smoke.log`, `slot_summary.json`, yuva paketi manifestini yazır və
`load_test_manifest.json` seçilmiş `--out-dir`-ə daxil edilir, beləliklə yük qaçışları birbaşa olaraq əlavə edilə bilər
sifarişli skript olmadan satış biletləri.

## 6. Telemetriya və idarəetmə təqibləri

- Zolaqlı idarə panellərini (`dashboards/grafana/nexus_lanes.json` və əlaqəli örtüklər) yeniləyin
  yeni zolaq id və metadata. Yaradılmış metadata açarları (`contact`, `channel`, `runbook` və s.)
  etiketləri əvvəlcədən doldurmaq asandır.
- Qəbuldan əvvəl yeni zolaq üçün PagerDuty/Alertmanager qaydalarını yazın. `summary.json`
  sonrakı addımlar massivi [Nexus əməliyyatlarında](./nexus-operations) yoxlama siyahısını əks etdirir.
- Validator dəsti aktiv olduqdan sonra manifest paketini Space Directory-də qeyd edin. Eyni istifadə edin
  köməkçi tərəfindən yaradılan manifest JSON idarəçilik kitabına uyğun olaraq imzalanır.
- Tüstü testləri üçün [Sora Nexus operatoru](./nexus-operator-onboarding) izləyin (FindNetworkStatus, Torii
  əlçatanlıq) və yuxarıda hazırlanmış artefakt dəsti ilə sübutları əldə edin.

## 7. Quru qaçış nümunəsi

Faylları yazmadan artefaktları nəzərdən keçirmək üçün:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --dry-run
```

Əmr JSON xülasəsini və TOML parçasını stdout-a çap edir, bu da əməliyyat zamanı tez təkrarlamağa imkan verir.
planlaşdırma.

---

Əlavə kontekst üçün bax:- [Nexus əməliyyatları](./nexus-operations) — əməliyyat yoxlama siyahısı və telemetriya tələbləri.
- [Sora Nexus operatorun işə salınması](./nexus-operator-onboarding) — aşağıdakılara istinad edən ətraflı bort axını
  yeni köməkçi.
- [Nexus zolaqlı model](./nexus-lane-model) — alət tərəfindən istifadə edilən zolaq həndəsəsi, şlaklar və saxlama planı.