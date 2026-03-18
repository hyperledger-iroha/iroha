---
id: orchestrator-tuning
lang: az
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Orchestrator Rollout & Tuning
sidebar_label: Orchestrator Tuning
description: Practical defaults, tuning guidance, and audit checkpoints for taking the multi-source orchestrator to GA.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
:::

# Orkestratorun İşlənməsi və Tuning Bələdçisi

Bu təlimat [konfiqurasiya arayışı](orchestrator-config.md) və
[çox mənbəli yayım runbook](multi-source-rollout.md). Bu izah edir
orkestratoru hər bir yayım mərhələsi üçün necə kökləmək, necə şərh etmək
skorbord artefaktları və hansı telemetriya siqnalları əvvəllər yerində olmalıdır
trafikin genişləndirilməsi. Tövsiyələri ardıcıl olaraq CLI, SDK-lar və arasında tətbiq edin
avtomatlaşdırma beləliklə hər node eyni deterministik gətirmə siyasətinə əməl edir.

## 1. Əsas Parametr Dəstləri

Paylaşılan konfiqurasiya şablonundan başlayın və kiçik düymələr dəstini kimi tənzimləyin
yayılma davam edir. Aşağıdakı cədvəldə tövsiyə olunan dəyərlər verilmişdir
ən ümumi mərhələlər; Siyahıda qeyd olunmayan dəyərlər standart parametrlərə qayıdır
`OrchestratorConfig::default()` və `FetchOptions::default()`.

| Faza | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Qeydlər |
|-------|-----------------|------------------------------|------------------------------------|---------------------------------------|------------------------------------|-------|
| **Laboratoriya / CI** | `3` | `2` | `2` | `2500` | `300` | Sıx gecikmə qapağı və lütf pəncərə səthi səs-küylü telemetriyanı tez bir zamanda aradan qaldırır. Etibarsız manifestləri daha tez üzə çıxarmaq üçün təkrar cəhdləri aşağı saxlayın. |
| **Səhnələşdirmə** | `4` | `3` | `3` | `4000` | `600` | Kəşfiyyatçı həmyaşıdları üçün boşluq buraxarkən istehsal defoltlarını əks etdirir. |
| **Kanarya** | `6` | `3` | `3` | `5000` | `900` | Defoltlara uyğun gəlir; `telemetry_region` təyin edin ki, idarə panelləri kanar trafikində dönə bilsin. |
| **Ümumi Mövcudluq** | `None` (uyğun olanların hamısını istifadə edin) | `4` | `4` | `5000` | `900` | Auditlər determinizmi tətbiq etməyə davam edərkən keçici xətaları qəbul etmək üçün təkrar cəhd və uğursuzluq hədlərini artırın. |

- Aşağı axın olmadıqca `scoreboard.weight_scale` standart `10_000` səviyyəsində qalır
  sistem fərqli tam rezolyusiya tələb edir. Ölçəni artırmaq deyil
  provayderin sifarişini dəyişdirmək; o, yalnız daha sıx kredit bölgüsü yayır.
- Fazalar arasında köçərkən, JSON paketini davam etdirin və istifadə edin
  `--scoreboard-out` beləliklə, audit izi dəqiq parametr dəstini qeyd edir.

## 2. Scoreboard Gigiyena

Hesab tablosu açıq tələbləri, provayder reklamlarını və telemetriyanı birləşdirir.
İrəli yuvarlanmadan əvvəl:

1. **Telemetriyanın təzəliyini təsdiq edin.** İstinad etdiyi snapshotlara əmin olun
   `--telemetry-json` konfiqurasiya edilmiş lütf pəncərəsində çəkildi. Girişlər
   konfiqurasiyadan köhnə `telemetry_grace_secs` ilə uğursuz olur
   `TelemetryStale { last_updated }`. Bunu çətin bir dayanma kimi qəbul edin və yeniləyin
   davam etməzdən əvvəl telemetriya ixracı.
2. **Uyğunluq səbəblərini yoxlayın.** Artefaktları vasitəsilə davam etdirin
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Hər bir giriş
   dəqiq uğursuzluq səbəbi ilə `eligibility` blokunu daşıyır. Üstündən yazmayın
   qabiliyyət uyğunsuzluğu və ya vaxtı keçmiş reklamlar; yuxarı axın yükünü düzəldin.
3. **Çəki deltalarını nəzərdən keçirin.** `normalised_weight` sahəsini ilə müqayisə edin
   əvvəlki buraxılış. Çəkinin >10% dəyişməsi qəsdən reklamla əlaqələndirilməlidir
   və ya telemetriya dəyişiklikləri və yayım jurnalında təsdiq edilməlidir.
4. **Arxiv artefaktları.** `scoreboard.persist_path`-i konfiqurasiya edin ki, hər qaçış emissiya etsin
   yekun tablonun görüntüsü. Artefaktı buraxılış qeydinə əlavə edin
   manifest və telemetriya paketi ilə yanaşı.
5. **Provayder qarışıq sübutunu qeyd edin.** `scoreboard.json` metadata _və_
   uyğun gələn `summary.json` `provider_count`-i ifşa etməlidir,
   `gateway_provider_count` və əldə edilən `provider_mix` etiketi rəyçilər
   qaçışın `direct-only`, `gateway-only` və ya `mixed` olduğunu sübut edə bilər.
   Gateway, buna görə də `provider_count=0` plus hesabatını tutur
   `provider_mix="gateway-only"`, qarışıq qaçışlar üçün sıfırdan fərqli saylar tələb olunarkən
   hər iki mənbə. `cargo xtask sorafs-adoption-check` bu sahələri tətbiq edir (və
   saylar/etiketlər uyğun gəlməyəndə uğursuz olur), ona görə də həmişə onu yanaşı işlədin
   `ci/check_sorafs_orchestrator_adoption.sh` və ya sizin sifarişinizlə çəkmə skripti
   `adoption_report.json` sübut paketini istehsal edin. Torii şlüzləri olduqda
   iştirak edir, `gateway_manifest_id`/`gateway_manifest_cid`-i tabloda saxlayın
   metadata beləliklə qəbul qapısı manifest zərfini ilə əlaqələndirə bilər
   tutulan provayder qarışığı.

Ətraflı sahə tərifləri üçün baxın
`crates/sorafs_car/src/scoreboard.rs` və CLI xülasə strukturu tərəfindən ifşa olunur
`sorafs_cli fetch --json-out`.

## CLI & SDK Bayraq Referansı

`sorafs_cli fetch` (bax: `crates/sorafs_car/src/bin/sorafs_cli.rs`) və
`iroha_cli app sorafs fetch` sarğı (`crates/iroha_cli/src/commands/sorafs.rs`)
eyni orkestr konfiqurasiya səthini paylaşın. Zaman aşağıdakı bayraqlardan istifadə edin
yayılma dəlili əldə etmək və ya kanonik qurğuları təkrar oynamaq:

Paylaşılan çoxmənbəli bayraq arayışı (yalnız bu faylı redaktə etməklə CLI yardımını və sənədləri sinxronlaşdırın):

- `--max-peers=<count>`, neçə uyğun provayderin tablo filtrindən sağ qalmasını məhdudlaşdırır. Hər uyğun provayderdən yayım üçün ayarlanmamış buraxın, yalnız tək mənbəli ehtiyatı qəsdən həyata keçirərkən `1` olaraq təyin edin. `maxPeers` düyməsini SDK-larda əks etdirir (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>`, `FetchOptions` tərəfindən tətbiq edilən hər bir hissəyə yenidən cəhd limitinə yönləndirir. Tövsiyə olunan dəyərlər üçün tənzimləmə təlimatında təqdim olunan cədvəldən istifadə edin; Sübut toplayan CLI qaçışları pariteti saxlamaq üçün SDK defoltlarına uyğun olmalıdır.
- `--telemetry-region=<label>` teqləri `sorafs_orchestrator_*` Prometheus seriyası (və OTLP releləri) region/env etiketi ilə, beləliklə idarə panelləri laboratoriya, səhnələşdirmə, kanareyka və GA trafikini ayıra bilsin.
- `--telemetry-json=<path>` skorbord tərəfindən istinad edilən snapshotı yeridir. Auditorların qaçışı təkrarlaya bilməsi üçün (və beləliklə, `cargo xtask sorafs-adoption-check --require-telemetry` hansı OTLP axınının çəkilişi qidalandırdığını sübut edə bilsin) hesab lövhəsinin yanında JSON-u davam etdirin.
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) körpü müşahidəçi qarmaqlarını işə salır. Quraşdırıldıqda, orkestrator yerli Norito/Kaigi proksisi vasitəsilə parçaları axın edir ki, brauzer müştəriləri, qoruyucu keşlər və Kaigi otaqları Rust tərəfindən buraxılan eyni qəbzləri alır.
- `--scoreboard-out=<path>` (isteğe bağlı olaraq `--scoreboard-now=<unix_secs>` ilə qoşalaşmışdır) auditorlar üçün uyğunluq snapshotunu saxlayır. Həmişə davamlı JSON-u buraxılış biletində istinad edilən telemetriya və manifest artefaktları ilə cütləşdirin.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` reklam metadatasının üstündə deterministik düzəlişlər tətbiq edir. Bu bayraqları yalnız məşqlər üçün istifadə edin; istehsalın aşağı salınması idarəetmə artefaktlarından keçməlidir ki, hər bir node eyni siyasət paketini tətbiq etsin.
- `--provider-metrics-out` / `--chunk-receipts-out` hər bir provayderin sağlamlıq göstəricilərini və buraxılış yoxlama siyahısında istinad edilən yığın qəbzlərini saxlayır; övladlığa götürmə sübutu təqdim edərkən hər iki artefakt əlavə edin.

Nümunə (dərc edilmiş qurğudan istifadə etməklə):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

SDK-lar Rust-da `SorafsGatewayFetchOptions` vasitəsilə eyni konfiqurasiyadan istifadə edir.
müştəri (`crates/iroha/src/client.rs`), JS bağlamaları
(`javascript/iroha_js/src/sorafs.js`) və Swift SDK
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Bu köməkçiləri içəridə saxlayın
operatorların avtomatlaşdırma üzrə siyasətləri kopyalaya bilməsi üçün CLI defoltları ilə addım-addım kilidləyin
sifarişli tərcümə qatları olmadan.

## 3. Siyasət Sazişini əldə edin

`FetchOptions` təkrar cəhd davranışına, paralellik və yoxlamaya nəzarət edir. Nə vaxt
tənzimləmə:

- **Yenidən cəhdlər:** `per_chunk_retry_limit`-i `4`-dən kənara qaldırmaq bərpanı artırır
  vaxt, lakin provayder xətalarını maskalamaq riski var. Tavan kimi `4`-ə üstünlük verin və
  yoxsul ifaçıları üzə çıxarmaq üçün provayderin fırlanmasına arxalanaraq.
- **Uğursuzluq həddi:** `provider_failure_threshold`
  provayder sessiyanın qalan hissəsi üçün deaktivdir. Bu dəyəri uyğunlaşdırın
  yenidən cəhd siyasəti: yenidən cəhd büdcəsindən aşağı hədd orkestratoru məcbur edir
  bütün cəhdlər tükənməzdən əvvəl həmyaşıdını çıxarmaq.
- **Uyğunluq:** `global_parallel_limit`-i (`None`) təyin olunmamış buraxın
  xüsusi mühit reklam edilən diapazonları doyura bilməz. Quraşdırıldıqda, əmin olun
  dəyər ≤ aclığın qarşısını almaq üçün provayder axını büdcələrinin cəmidir.
- **Doğrulama keçidləri:** `verify_lengths` və `verify_digests` qalmalıdır
  istehsala imkan verir. Qarışıq provayder donanması olduqda determinizmə zəmanət verirlər
  oyundadır; onları yalnız təcrid olunmuş fuzzing mühitlərində söndürün.

## 4. Nəqliyyat və Anonimlik Hazırlanması

`rollout_phase`, `anonymity_policy` və `transport_policy` sahələrindən istifadə edin.
məxfilik mövqeyini təmsil edir:- `rollout_phase="snnet-5"`-ə üstünlük verin və defolt anonimlik siyasətinə icazə verin
  SNNet-5 mərhələlərini izləyin. Yalnız `anonymity_policy_override` vasitəsilə ləğv edin
  idarəetmə imzalanmış göstəriş verdikdə.
- SNNet-4/5/5a/5b/6a/7/8/12/13 🈺 olarkən `transport_policy="soranet-first"`-i baza kimi saxlayın
  (bax `roadmap.md`). `transport_policy="direct-only"` yalnız sənədləşdirilmiş üçün istifadə edin
  endirmələr/uyğunluq təlimləri və əvvəl PQ əhatə dairəsinin nəzərdən keçirilməsini gözləyin
  `transport_policy="soranet-strict"`-ə yüksəldilməsi—həmin səviyyə tez uğursuz olarsa
  yalnız klassik relelər qalır.
- `write_mode="pq-only"` yalnız hər yazma yolu (SDK,
  orkestr, idarəetmə alətləri) PQ tələblərini ödəyə bilər. ərzində
  buraxılışlar `write_mode="allow-downgrade"`-ni saxlayır, beləliklə, fövqəladə hallara cavablar etibar edə bilər
  birbaşa marşrutlarda, telemetriya isə endirməni qeyd edir.
- Mühafizəçi seçimi və dövrə quruluşu SoraNet kataloquna əsaslanır. tədarük edin
  imzalanmış `relay_directory` snapshot və `guard_set` önbelleğini qoruyun
  boşluq razılaşdırılmış saxlama pəncərəsi daxilində qalır. Keş barmaq izi qeyd edildi
  `sorafs_cli fetch` tərəfindən təqdim edilən sübutun bir hissəsini təşkil edir.

## 5. Endirmə və Uyğunluq Qarmaqları

İki orkestr alt sistemi əl müdaxiləsi olmadan siyasəti həyata keçirməyə kömək edir:

- **Endirmə remediasiyası** (`downgrade_remediation`): monitorlar
  `handshake_downgrade_total` hadisələri və konfiqurasiya edilmiş `threshold`-dən sonra
  `window_secs` daxilində aşdı, yerli proxy-ni `target_mode`-ə məcbur edir
  (yalnız metadata standart olaraq). Defoltları saxlayın (`threshold=3`, `window=300`,
  `cooldown=900`). İstənilən sənəd
  buraxılış jurnalında ləğv edin və tablosunun izlənilməsini təmin edin
  `sorafs_proxy_downgrade_state`.
- **Uyğunluq siyasəti** (`compliance`): yurisdiksiya və açıq-aşkar kəsilmələr
  idarəetmə tərəfindən idarə olunan imtina siyahıları vasitəsilə axın. Heç vaxt daxili ad-hoc ləğv etmə
  konfiqurasiya paketində; əvəzinə imzalanmış yeniləmə tələb edin
  `governance/compliance/soranet_opt_outs.json` və yaradılan JSON-u yenidən yerləşdirin.

Hər iki sistem üçün ortaya çıxan konfiqurasiya paketini davam etdirin və onu daxil edin
sübutları buraxın ki, auditorlar yerdəyişmələrin necə baş verdiyini izləyə bilsinlər.

## 6. Telemetriya və İdarə Panelləri

Yayımlanmanı genişləndirməzdən əvvəl aşağıdakı siqnalların canlı olduğunu təsdiqləyin
hədəf mühit:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` -
  kanareyka tamamlandıqdan sonra sıfır olmalıdır.
- `sorafs_orchestrator_retries_total` və
  `sorafs_orchestrator_retry_ratio` — müddət ərzində 10%-dən aşağı stabilləşməlidir
  kanareyka və GA-dan sonra 5%-dən aşağı qalır.
- `sorafs_orchestrator_policy_events_total` — gözlənilənləri təsdiq edir
  yayma mərhələsi aktivdir (`stage` etiketi) və `outcome` vasitəsilə qaralmaları qeyd edir.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — qarşı PQ rele təchizatını izləyin
  siyasət gözləntiləri.
- `telemetry::sorafs.fetch.*` jurnal hədəfləri — paylaşılan jurnala axın edilməlidir
  `status=failed` üçün saxlanmış axtarışları olan aqreqator.

Buradan kanonik Grafana tablosunu yükləyin
`dashboards/grafana/sorafs_fetch_observability.json` (portalda ixrac edilib
**SoraFS → Müşahidə Edilmə**) altında bölgə/manifest seçiciləri,
provayder istilik xəritəsini, yığın gecikmə histoqramlarını və dayanma sayğaclarını yenidən sınayın
Yanma zamanı SRE nə nəzərdən keçirir. Alertmanager qaydalarını daxil edin
`dashboards/alerts/sorafs_fetch_rules.yml` və Prometheus sintaksisini doğrulayın
`scripts/telemetry/test_sorafs_fetch_alerts.sh` ilə (köməkçi avtomatik olaraq
`promtool test rules` yerli və ya Docker ilə işləyir). Alert təhvil-off tələb edir
skriptin çap etdiyi eyni marşrutlaşdırma bloku, operatorlar sübutları bağlaya bilsinlər
buraxılış bileti.

### Telemetriya ilə işləmə prosesi

Yol xəritəsi elementi **SF-6e** üçün 30 günlük telemetriya yanması tələb olunur
GA defoltlarına çox mənbəli orkestr. üçün depo skriptlərindən istifadə edin
pəncərədə hər gün üçün təkrarlana bilən artefakt paketini çəkin:

1. Yanma mühiti ilə `ci/check_sorafs_orchestrator_adoption.sh`-i işə salın
   düymələr dəsti. Misal:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   Köməkçi `fixtures/sorafs_orchestrator/multi_peer_parity_v1`-i təkrarlayır,
   yazır `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` və `adoption_report.json` altında
   `artifacts/sorafs_orchestrator/<timestamp>/` və minimum sayı tətbiq edir
   uyğun provayderlərin `cargo xtask sorafs-adoption-check` vasitəsilə.
2. Yandırılan dəyişənlər mövcud olduqda skript də emissiya edir
   `burn_in_note.json`, etiket, gün indeksi, manifest identifikatoru, telemetriya
   mənbə və artefakt həzmləri. Bu JSON-u yayma jurnalına əlavə edin
   30 günlük pəncərədə hər gün hansı ələ keçirməkdən məmnun olduğu aydındır.
3. Yenilənmiş Grafana lövhəsini idxal edin (`dashboards/grafana/sorafs_fetch_observability.json`)
   səhnələşdirmə/istehsal iş sahəsinə daxil edin, onu yanma etiketi ilə işarələyin və
   hər panelin sınaqdan keçirilən manifest/region üçün nümunələr göstərdiyini təsdiqləyin.
4. `scripts/telemetry/test_sorafs_fetch_alerts.sh` (və ya `promtool test rules …`) işə salın
   hər dəfə `dashboards/alerts/sorafs_fetch_rules.yml` bunu sənədləşdirmək üçün dəyişdikdə
   xəbərdarlıq marşrutu yanma zamanı ixrac edilmiş ölçülərə uyğun gəlir.
5. Nəticə olan tablosunun snapshotını, xəbərdarlıq testinin çıxışını və log quyruğunu arxivləşdirin
   orkestrlə yanaşı `telemetry::sorafs.fetch.*` axtarışlarından
   artefaktlar, beləliklə, idarəetmə ölçmələri götürmədən sübutları təkrarlaya bilər
   canlı sistemlər.

## 7. Təqdimat Yoxlama Siyahısı

1. Namizəd konfiqurasiyasından və ələ keçirmədən istifadə edərək CI-də xal lövhələrini bərpa edin
   versiya nəzarəti altında artefaktlar.
2. Hər bir mühitdə (laboratoriya, səhnələşdirmə,
   kanareyka, istehsal) və `--scoreboard-out` və `--json-out` əlavə edin
   artefaktlar buraxılış rekorduna.
3. Bütün ölçüləri təmin edərək, çağırış üzrə mühəndislə telemetriya tablosunu nəzərdən keçirin
   yuxarıda canlı nümunələr var.
4. Son konfiqurasiya yolunu qeyd edin (adətən `iroha_config` vasitəsilə) və
   reklamlar və uyğunluq üçün istifadə edilən idarəetmə reyestrinin git commit.
5. Yayım izləyicisini yeniləyin və müştəriyə yeni defoltlar barədə SDK komandalarına məlumat verin
   inteqrasiyalar uyğunlaşır.

Bu təlimatı izləmək orkestrator yerləşdirmələrini deterministik və yoxlanıla bilir
Yenidən cəhd büdcələrini tənzimləmək üçün aydın rəy döngələri təmin edərkən, provayder
qabiliyyəti və məxfilik mövqeyi.