---
lang: az
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9eafd2f3786dbe0ccfa7db2ec138d1d91e306e1691fbee801ba04a4165131655
source_last_modified: "2026-01-05T09:28:11.915061+00:00"
translation_last_reviewed: 2026-02-07
id: privacy-metrics-pipeline
title: SoraNet Privacy Metrics Pipeline (SNNet-8)
sidebar_label: Privacy Metrics Pipeline
description: Privacy-preserving telemetry collection for SoraNet relays and orchestrators.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

# SoraNet Məxfilik Metrikləri Boru Kəməri

SNNet-8 relenin işləmə müddəti üçün məxfilikdən xəbərdar telemetriya səthini təqdim edir. The
relay indi əl sıxma və dövrə hadisələrini dəqiqəlik vedrələrə toplayır və
fərdi sxemləri saxlayaraq yalnız qaba Prometheus sayğaclarını ixrac edir
operatorlara təsirli görünmə imkanı verərkən əlaqəni kəsmək mümkün deyil.

## Aqreqatora İcmal

- İş vaxtının tətbiqi `tools/soranet-relay/src/privacy.rs` kimi yaşayır
  `PrivacyAggregator`.
- Vedrələr divar saatı dəqiqəsinə görə açar (`bucket_secs`, standart 60 saniyə) və
  məhdud halqada saxlanılır (`max_completed_buckets`, standart 120). Kollektor
  səhmlər öz məhdud ehtiyat işini saxlayır (`max_share_lag_buckets`, defolt 12)
  belə ki, köhnə Prio pəncərələri sızan deyil, basdırılmış vedrələr kimi yuyulur
  yaddaş və ya yapışmış kollektorların maskalanması.
- `RelayConfig::privacy` düz `PrivacyConfig`-ə xəritə verir, tənzimləməni ifşa edir
  düymələr (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`,
  `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`,
  `expected_shares`). SNNet-8a zamanı istehsal müddəti standartları saxlayır
  təhlükəsiz aqreqasiya hədlərini təqdim edir.
- Runtime modulları tipli köməkçilər vasitəsilə hadisələri qeyd edir:
  `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`,
  `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`,
  `record_verified_bytes` və `record_gar_category`.

## Relay Admin Endpoint

Operatorlar vasitəsilə xam müşahidələr üçün releyin admin dinləyicisini sorğulaya bilərlər
`GET /privacy/events`. Son nöqtə yeni sətirlə ayrılmış JSON-u qaytarır
(`application/x-ndjson`) əks olunan `SoranetPrivacyEventV1` faydalı yükləri ehtiva edir
daxili `PrivacyEventBuffer`-dən. Bufer ən yeni hadisələri yuxarıda saxlayır
`privacy.event_buffer_capacity` girişlərinə (defolt 4096) və boşaldılır
oxuyun, buna görə də kazıyıcılar boşluqların qarşısını almaq üçün kifayət qədər tez-tez sorğu keçirməlidirlər. Hadisələr əhatə edir
eyni əl sıxma, tənzimləmə, təsdiqlənmiş bant genişliyi, aktiv dövrə və GAR siqnalları
Prometheus sayğaclarını gücləndirərək aşağı axın kollektorlarına arxivləşdirməyə imkan verir
məxfilik üçün təhlükəsiz çörək qırıntıları və ya feed təhlükəsiz toplama iş axınları.

## Rele Konfiqurasiyası

Operatorlar relay konfiqurasiya faylında məxfilik telemetriya kadanslarını tənzimləyir
`privacy` bölməsi:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

Sahənin defoltları SNNet-8 spesifikasiyasına uyğun gəlir və yükləmə zamanı təsdiqlənir:

| Sahə | Təsvir | Defolt |
|-------|-------------|---------|
| `bucket_secs` | Hər bir toplama pəncərəsinin eni (saniyə). | `60` |
| `min_handshakes` | Bir vedrə sayğaclar buraxmadan əvvəl minimum töhfə verənlərin sayı. | `12` |
| `flush_delay_buckets` | Təmizləmə cəhdindən əvvəl gözləmək üçün tamamlanmış vedrələr. | `1` |
| `force_flush_buckets` | Biz bastırılmış vedrə yaymadan əvvəl maksimum yaş. | `6` |
| `max_completed_buckets` | Saxlanılan vedrə ehtiyatı (məhdud yaddaşın qarşısını alır). | `120` |
| `max_share_lag_buckets` | Yatırılmadan əvvəl kollektor səhmləri üçün saxlama pəncərəsi. | `12` |
| `expected_shares` | Birləşmədən əvvəl kollektor payları tələb olunur. | `2` |
| `event_buffer_capacity` | İdarəçi axını üçün NDJSON hadisə qeydi. | `4096` |

`force_flush_buckets` parametri `flush_delay_buckets`-dən aşağı, sıfırlanır
eşiklər və ya saxlama qoruyucusunu söndürmək indi doğrulamanın qarşısını almaq üçün uğursuz olur
hər relay telemetriyasını sızdıracaq yerləşdirmələr.

`event_buffer_capacity` limiti eyni zamanda `/admin/privacy/events`-i də məhdudlaşdıraraq,
skreperlər sonsuza qədər geri qala bilməz.

## Prio kolleksiyaçı paylaşımları

SNNet-8a gizli paylaşılan Prio vedrələrini yayan ikili kollektorları yerləşdirir. The
orkestr indi hər ikisi üçün `/privacy/events` NDJSON axınını təhlil edir
`SoranetPrivacyEventV1` girişləri və `SoranetPrivacyPrioShareV1` paylaşımları,
onları `SoranetSecureAggregator::ingest_prio_share`-ə yönləndirir. Kovalar yayır
`PrivacyBucketConfig::expected_shares` töhfələri gələn dəfə, əks etdirən
rele davranışı. Səhmlər kovanın düzülməsi və histoqram forması üçün təsdiqlənir
`SoranetPrivacyBucketMetricsV1`-ə birləşdirilmədən əvvəl. Birləşdirilmişdirsə
əl sıxma sayı `min_contributors`-dən aşağı düşür, vedrə belə ixrac edilir
`suppressed`, relaydaxili toplayıcının davranışını əks etdirir. Basdırılmış
pəncərələr indi `suppression_reason` etiketi buraxır ki, operatorlar onları fərqləndirə bilsinlər.
`insufficient_contributors`, `collector_suppressed`,
`collector_window_elapsed` və `forced_flush_window_elapsed` ssenariləri
telemetriya boşluqlarının diaqnostikası. `collector_window_elapsed` səbəbi də işə düşür
Prio səhmləri `max_share_lag_buckets`-i keçərək ilişib qalan kolleksiyaçılara çevrildikdə
yaddaşda köhnəlmiş akkumulyatorları buraxmadan görünür.

## Torii Qəbul Son Nöqtələri

Torii indi iki telemetriya qapalı HTTP son nöqtəsini ifşa edir, beləliklə rele və kollektorlar
Müşahidələri sifarişli nəqliyyat daxil etmədən ötürə bilər:

- `POST /v2/soranet/privacy/event` a qəbul edir
  `RecordSoranetPrivacyEventDto` faydalı yük. Bədən sarılır a
  `SoranetPrivacyEventV1` və əlavə olaraq `source` etiketi. Torii təsdiqləyir
  aktiv telemetriya profilinə qarşı sorğu göndərir, hadisəni qeyd edir və cavab verir
  olan Norito JSON zərfinin yanında HTTP `202 Accepted` ilə
  hesablanmış çömçə pəncərəsi (`bucket_start_unix`, `bucket_duration_secs`) və
  rele rejimi.
- `POST /v2/soranet/privacy/share` `RecordSoranetPrivacyShareDto` qəbul edir
  faydalı yük. Korpusda `SoranetPrivacyPrioShareV1` və isteğe bağlı var
  `forwarded_by` işarəsi operatorların kollektor axınlarını yoxlaya bilməsi üçün. Uğurlu
  təqdimatlar ümumiləşdirən Norito JSON zərfi ilə HTTP `202 Accepted` qaytarır
  kollektor, vedrə pəncərəsi və yatırma işarəsi; doğrulama uğursuzluqlarının xəritəsi
  deterministik xətanın idarə edilməsini qorumaq üçün telemetriya `Conversion` cavabı
  kollektorlar arasında. Orkestratorun hadisə dövrəsi indi bu paylaşımları olduğu kimi yayır
  Torii-in Prio akkumulyatorunu on-rele vedrələri ilə sinxronlaşdıraraq sorğu releləri.

Hər iki son nöqtə telemetriya profilinə hörmət edir: onlar `503 Xidmətini yayırlar
Metriklər deaktiv edildikdə əlçatan deyil. Müştərilər ya Norito ikili faylı göndərə bilərlər
(`application/x.norito`) və ya Norito JSON (`application/x.norito+json`) gövdələri;
server standart Torii vasitəsilə avtomatik olaraq formatı müzakirə edir
ekstraktorlar.

## Prometheus Metriklər

Hər ixrac edilmiş vedrə `mode` (`entry`, `middle`, `exit`) və
`bucket_start` etiketləri. Aşağıdakı metrik ailələr buraxılır:

| Metrik | Təsvir |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` ilə əl sıxma taksonomiyası. |
| `soranet_privacy_throttles_total{scope}` | `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` ilə qaz sayğacları. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Sıxılmış əl sıxmaların ümumi soyuma müddəti. |
| `soranet_privacy_verified_bytes_total` | Korlanmış ölçmə sübutlarından təsdiqlənmiş bant genişliyi. |
| `soranet_privacy_active_circuits_{avg,max}` | Bir vedrə üçün orta və pik aktiv dövrələr. |
| `soranet_privacy_rtt_millis{percentile}` | RTT faiz təxminləri (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Hashed İdarəetmə Fəaliyyət Hesabatı sayğacları kateqoriya həzminə görə açar. |
| `soranet_privacy_bucket_suppressed` | Əmanətçi həddi yerinə yetirilmədiyi üçün vedrələr tutuldu. |
| `soranet_privacy_pending_collectors{mode}` | Kollektor payı akkumulyatorlar rele rejiminə görə qruplaşdırılmış birləşməni gözləyir. |
| `soranet_privacy_suppression_total{reason}` | `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` ilə basdırılmış vedrə sayğacları beləliklə, tablosuna məxfilik boşluqları aid edilə bilər. |
| `soranet_privacy_snapshot_suppression_ratio` | Xəbərdarlıq büdcələri üçün faydalı olan son boşalmanın sıxışdırılmış/boşaldılmış nisbəti (0-1). |
| `soranet_privacy_last_poll_unixtime` | Ən son uğurlu sorğunun UNIX vaxt damğası (kollektorun boş qalması xəbərdarlığını idarə edir). |
| `soranet_privacy_collector_enabled` | Məxfilik kollektoru söndürüldükdə və ya işə başlamadıqda `0`-ə çevrilən ölçmə cihazı (kollektorun söndürülməsi xəbərdarlığını idarə edir). |
| `soranet_privacy_poll_errors_total{provider}` | Relay ləqəbi ilə qruplaşdırılmış səsvermə uğursuzluqları (şifrləmə xətaları, HTTP uğursuzluqları və ya gözlənilməz status kodları üzrə artımlar). |

Müşahidələri olmayan vedrələr səssiz qalır, tablosunu səliqəli saxlayır
sıfır doldurulmuş pəncərələrin istehsalı.

## Əməliyyat Rəhbərliyi

1. **İdarəetmə panelləri** – yuxarıda `mode` və `window_start` ilə qruplaşdırılmış göstəriciləri qrafikləşdirin.
   Kollektor və ya relay problemlərini üzə çıxarmaq üçün çatışmayan pəncərələri vurğulayın. istifadə edin
   `soranet_privacy_suppression_total{reason}` töhfə verəni fərqləndirmək üçün
   boşluqları təyin edərkən kollektor tərəfindən idarə olunan supressiyadan yaranan çatışmazlıqlar. Grafana
   aktiv indi həmin şəxslər tərəfindən qidalanan xüsusi **“Supressiya Səbəbləri (5m)”** paneli göndərir.
   sayğaclar üstəgəl hesablayan **“Bastırılmış Kod %”** statı
   Seçim başına `sum(soranet_privacy_bucket_suppressed) / count(...)` belə
   operatorlar büdcə pozuntularını bir baxışda görə bilərlər. **Kollektor Paylaşımı
   Backlog** seriyası (`soranet_privacy_pending_collectors`) və **Snapshot
   Supression Ratio** statı ilişib qalan kollektorları və büdcə sürüşməsi zamanı vurğulayır
   avtomatlaşdırılmış qaçışlar.
2. **Xəbərdarlıq** – məxfilik üçün təhlükəsiz sayğaclardan siqnalları idarə edin: PoW sıçrayışlarını rədd edir,
   soyutma tezliyi, RTT sürüşməsi və tutumdan imtina. Çünki sayğaclar var
   hər vedrə daxilində monotonik, sadə tarifə əsaslanan qaydalar yaxşı işləyir.
3. **Hadisə cavabı** – əvvəlcə ümumiləşdirilmiş məlumatlara etibar edin. Daha dərin sazlama zamanı
   lazım olduqda, vedrə anlıq görüntülərini təkrar oxutmaq və ya korlanmış yoxlamaq üçün rele tələb edin
   xam trafik qeydlərini yığmaq əvəzinə ölçmə sübutları.
4. **Saxlama** – həddindən artıq olmamaq üçün kifayət qədər tez-tez qaşıyın
   `max_completed_buckets`. İxracatçılar Prometheus çıxışını kimi qəbul etməlidirlər
   kanonik mənbə və bir dəfə yönləndirildikdən sonra yerli vedrələri buraxın.

## Supression Analytics & Avtomatlaşdırılmış RunlarSNNet-8-in qəbulu avtomatlaşdırılmış kollektorların qaldığını nümayiş etdirməkdən asılıdır
sağlamdır və bu bastırma siyasət sərhədləri daxilində qalır (hər bir vedrənin ≤10%-i
istənilən 30 dəqiqəlik pəncərədən keçir). İndi o qapını təmin etmək üçün lazım olan alətlər
ağac ilə gəmilər; operatorlar bunu həftəlik rituallarına daxil etməlidirlər. Yeni
Grafana bastırma panelləri aşağıdakı PromQL parçalarını əks etdirir və zəng zamanı verir.
komandalar manuel sorğulara qayıtmadan əvvəl canlı görünürlük.

### Bastırmanın nəzərdən keçirilməsi üçün PromQL reseptləri

Operatorlar aşağıdakı PromQL köməkçilərini əllərində saxlamalıdırlar; hər ikisinə istinad edilir
paylaşılan Grafana idarə panelində (`dashboards/grafana/soranet_privacy_metrics.json`)
və Alertmanager qaydaları:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

**“Bastırılmış Kod %”** statının aşağıda qaldığını təsdiqləmək üçün nisbət çıxışından istifadə edin
siyasət büdcəsi; sürətli rəy üçün sünbül detektorunu Alertmanager-ə bağlayın
ianəçilərin sayı gözlənilmədən azaldıqda.

### Oflayn paket hesabatı CLI

İş sahəsi birdəfəlik NDJSON üçün `cargo xtask soranet-privacy-report`-i ifşa edir
tutur. Onu bir və ya daha çox relay admin ixracına yönəldin:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

Köməkçi çəkməni `SoranetSecureAggregator` vasitəsilə ötürür, a çap edir
stdout-a supressiya xülasəsi verir və isteğe bağlı olaraq strukturlaşdırılmış JSON hesabatı yazır
`--json-out <path|->` vasitəsilə. O, canlı kolleksiyaçı ilə eyni düymələrə hörmət edir
(`--bucket-secs`, `--min-contributors`, `--expected-shares` və s.), icarəyə verilməsi
operatorlar triating zamanı müxtəlif eşiklər altında tarixi çəkilişləri təkrarlayır
bir məsələ. JSON-u Grafana ekran görüntüləri ilə birlikdə əlavə edin ki, SNNet-8
bastırma analitika qapısı yoxlanıla bilər.

### İlk avtomatlaşdırılmış qaçış yoxlama siyahısı

İdarəetmə hələ də ilk avtomatlaşdırma əməliyyatının tələblərə cavab verdiyini sübut etməyi tələb edir
bastırma büdcəsi. Köməkçi indi `--max-suppression-ratio <0-1>` qəbul edir
CI və ya operatorlar sıxılmış vedrələr icazə verilən həddi aşdıqda sürətlə uğursuz ola bilər
pəncərə (standart 10%) və ya hələ heç bir vedrə olmadıqda. Tövsiyə olunan axın:

1. NDJSON-u relay admin son nöqtələrindən və orkestratordan ixrac edin
   `/v2/soranet/privacy/event|share` daxil olur
   `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Siyasət büdcəsi ilə köməkçini işə salın:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   Komanda müşahidə olunan nisbəti çap edir və büdcə olduqda sıfırdan çıxır
   heç bir vedrə hazır olmadıqda **və ya** keçdi, bu telemetriyanın hazır olmadığını göstərir
   hələ qaçış üçün istehsal edilmişdir. Canlı göstəricilər göstərilməlidir
   `soranet_privacy_pending_collectors` sıfıra doğru boşalma və
   `soranet_privacy_snapshot_suppression_ratio` eyni büdcə altında qalır
   qaçış yerinə yetirilərkən.
3. Daha əvvəl SNNet-8 sübut paketi ilə JSON çıxışını və CLI jurnalını arxivləşdirin
   Nəzarətçilər dəqiq artefaktları təkrarlaya bilməsi üçün nəqliyyat defoltunu dəyişdirmək.

## Növbəti Addımlar (SNNet-8a)

- İkili Prio kollektorlarını inteqrasiya edin, onların pay qəbulunu şəbəkəyə birləşdirin
  iş vaxtı beləliklə rele və kollektorlar ardıcıl `SoranetPrivacyBucketMetricsV1` yayır
  faydalı yüklər. *(Hazır - bax `ingest_privacy_payload`
  `crates/sorafs_orchestrator/src/lib.rs` və müşayiət olunan testlər.)*
- Paylaşılan Prometheus idarə paneli JSON və xəbərdarlıq qaydalarını dərc edin
  bastırma boşluqları, kollektorun sağlamlığı və anonimlik pozğunluqları. *(Tamamlandı - bax
  `dashboards/grafana/soranet_privacy_metrics.json`,
  `dashboards/alerts/soranet_privacy_rules.yml`,
  `dashboards/alerts/soranet_policy_rules.yml` və doğrulama qurğuları.)*
--də təsvir olunan diferensial-məxfilik kalibrləmə artefaktlarını istehsal edin
  `privacy_metrics_dp.md`, o cümlədən təkrarlana bilən noutbuklar və idarəetmə
  həzm edir. *(Tamamlandı — notebook + tərəfindən yaradılan artefaktlar
  `scripts/telemetry/run_privacy_dp.py`; CI sarğı
  `scripts/telemetry/run_privacy_dp_notebook.sh` notebooku vasitəsilə icra edir
  `.github/workflows/release-pipeline.yml` iş axını; idarəçilik jurnalı təqdim edilmişdir
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

Cari buraxılış SNNet-8 təməlini təqdim edir: deterministik,
birbaşa mövcud Prometheus kazıyıcılarına daxil olan məxfilik üçün təhlükəsiz telemetriya
və idarə panelləri. Diferensial məxfilik kalibrləmə artefaktları yerindədir
buraxılış boru kəmərinin iş axını notebook çıxışlarını təzə saxlayır, qalanları
iş ilk avtomatlaşdırılmış qaçışın monitorinqinə və yatırmanın genişləndirilməsinə yönəlmişdir
xəbərdarlıq analitikası.