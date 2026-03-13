---
lang: az
direction: ltr
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T14:35:37.691616+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Məlumatların Əlçatanlığının Replikasiya Siyasəti (DA-4)

_Status: Davam edir — Sahiblər: Əsas Protokol WG / Saxlama Komandası / SRE_

DA qəbulu kəməri indi deterministik saxlama hədəflərini tətbiq edir
`roadmap.md` (iş axını DA-4)-də təsvir olunan hər blob sinfi. Torii imtina edir
konfiqurasiyaya uyğun gəlməyən zəng edənin təmin etdiyi saxlama zərflərini davam etdirin
hər bir validator/saxlama qovşağının tələb olunanı saxlamasına zəmanət verən siyasət
təqdim edənin niyyətinə əsaslanmadan dövrlərin və replikaların sayı.

## Defolt siyasət

| Blob sinfi | İsti tutma | Soyuq tutma | Tələb olunan replikalar | Saxlama sinfi | İdarəetmə etiketi |
|------------|---------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 saat | 14 gün | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 saat | 7 gün | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 saat | 180 gün | 3 | `cold` | `da.governance` |
| _Defolt (bütün digər siniflər)_ | 6 saat | 30 gün | 3 | `warm` | `da.default` |

Bu dəyərlər `torii.da_ingest.replication_policy`-ə daxil edilib və tətbiq edilir
bütün `/v2/da/ingest` təqdimatları. Torii manifestləri məcburi olanlarla yenidən yazır
saxlama profili və zəng edənlər uyğun olmayan dəyərlər təqdim etdikdə xəbərdarlıq verir
operatorlar köhnəlmiş SDK-ları aşkar edə bilər.

### Taikai mövcudluğu dərsləri

Taikai marşrutlaşdırma manifestləri (`taikai.trm` metadata) indi
`availability_class` işarəsi (`Hot`, `Warm` və ya `Cold`). Mövcud olduqda, Torii
`torii.da_ingest.replication_policy`-dən uyğun saxlama profilini seçir
faydalı yükü parçalamadan əvvəl, hadisə operatorlarına aktiv olmayan səviyyəni endirməyə imkan verir
qlobal siyasət cədvəlini redaktə etmədən tərcümələr. Varsayılanlar:

| Mövcudluq sinfi | İsti tutma | Soyuq tutma | Tələb olunan replikalar | Saxlama sinfi | İdarəetmə etiketi |
|----------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 saat | 14 gün | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 saat | 30 gün | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 saat | 180 gün | 3 | `cold` | `da.taikai.archive` |

Manifestdə `availability_class` buraxılarsa, qəbul yolu yenidən
`hot` profili beləliklə canlı yayımlar tam replika dəstini saxlayır. Operatorlar bilər
yenisini redaktə edərək bu dəyərləri ləğv edin
Konfiqurasiyada `torii.da_ingest.replication_policy.taikai_availability` bloku.

## Konfiqurasiya

Siyasət `torii.da_ingest.replication_policy` altında yaşayır və a. ifşa edir
*defolt* şablon üstəgəl hər sinif üzrə ləğvetmələr massivi. Sinif identifikatorları bunlardır
hərflərə həssas deyil və `taikai_segment`, `nexus_lane_sidecar`,
İdarəetmə tərəfindən təsdiqlənmiş genişləndirmələr üçün `governance_artifact` və ya `custom:<u16>`.
Saxlama sinifləri `hot`, `warm` və ya `cold`-i qəbul edir.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

Yuxarıda sadalanan defoltlarla işləmək üçün bloku toxunulmaz buraxın. bərkitmək üçün a
sinif, uyğunluğu ləğv etməyi yeniləmək; yeni siniflər üçün baza xəttini dəyişdirmək,
redaktə edin `default_retention`.Xüsusi Taikai əlçatanlıq siniflərini tənzimləmək üçün altına qeydlər əlavə edin
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## İcra semantikası

- Torii istifadəçi tərəfindən təmin edilmiş `RetentionPolicy`-i məcburi profillə əvəz edir
  parçalanmadan və ya açıq-aşkar emissiyadan əvvəl.
- Uyğunsuz saxlama profilini elan edən əvvəlcədən qurulmuş manifestlər rədd edilir
  `400 schema mismatch` ilə köhnə müştərilər müqaviləni zəiflədə bilməz.
- Hər ləğvetmə hadisəsi qeyd olunur (`blob_class`, gözlənilən siyasətə qarşı təqdim olunur)
  yayım zamanı uyğun gəlməyən zəng edənləri üzə çıxarmaq.

Yenilənmiş qapı üçün `docs/source/da/ingest_plan.md` (Təsdiqləmə yoxlama siyahısı) baxın
saxlanmanın icrasını əhatə edir.

## Yenidən təkrarlama iş axını (DA-4 təqibi)

Saxlamanın tətbiqi yalnız ilk addımdır. Operatorlar da bunu sübut etməlidirlər
canlı manifestlər və replikasiya sifarişləri konfiqurasiya edilmiş siyasətə uyğun olaraq qalır
ki, SoraFS uyğun olmayan blobları avtomatik olaraq təkrarlaya bilər.

1. **Drift üçün baxın.** Torii yayır
   `overriding DA retention policy to match configured network baseline` istənilən vaxt
   zəng edən köhnə saxlama dəyərlərini təqdim edir. Həmin log ilə cütləşdirin
   `torii_sorafs_replication_*` replika çatışmazlıqlarını və ya gecikmələrini aşkar etmək üçün telemetriya
   yenidən yerləşdirmələr.
2. **Məqsəd və canlı replikalar fərqlidir.** Yeni audit köməkçisindən istifadə edin:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Komanda təqdim olunandan `torii.da_ingest.replication_policy` yükləyir
   konfiqurasiya edir, hər bir manifesti deşifrə edir (JSON və ya Norito) və istəyə görə hər hansı
   `ReplicationOrderV1` manifest həzminə görə faydalı yüklər. Xülasə iki işarə edir
   şərtlər:

   - `policy_mismatch` – manifest saxlama profili məcburi profildən fərqlənir
     siyasət (Torii səhv konfiqurasiya edilmədikdə bu heç vaxt baş verməməlidir).
   - `replica_shortfall` – canlı replikasiya sifarişi daha az replika tələb edir
     `RetentionPolicy.required_replicas` və ya ondan daha az tapşırıq verir
     hədəf.

   Sıfır olmayan çıxış statusu aktiv çatışmazlığı göstərir, beləliklə CI/zəng üzrə avtomatlaşdırma
   dərhal səhifə edə bilərsiniz. JSON hesabatını əlavə edin
   Parlament səsləri üçün `docs/examples/da_manifest_review_template.md` paketi.
3. **Replikasiyanı işə salın.** Audit çatışmazlıq barədə məlumat verdikdə, yeni
   `ReplicationOrderV1` -də təsvir edilən idarəetmə alətləri vasitəsilə
   `docs/source/sorafs/storage_capacity_marketplace.md` və auditi yenidən həyata keçirin
   replika dəsti birləşənə qədər. Fövqəladə hallar üçün CLI çıxışını cütləşdirin
   `iroha app da prove-availability` ilə, beləliklə SRE-lər eyni digestə istinad edə bilər
   və PDP sübutları.

Reqressiya əhatə dairəsi `integration_tests/tests/da/replication_policy.rs`-də yaşayır;
paket `/v2/da/ingest`-ə uyğun olmayan saxlama siyasəti təqdim edir və təsdiqləyir
ki, gətirilən manifest zəng edənin əvəzinə məcburi profili ifşa edir
niyyət.

## Sağlamlığı sübut edən telemetriya və idarə panelləri (DA-5 körpüsü)

Yol xəritəsi bəndi **DA-5** PDP/PoTR tətbiqi nəticələrinin audit edilə bilən olmasını tələb edir
real vaxt. `SorafsProofHealthAlert` hadisələri indi xüsusi bir dəsti idarə edir
Prometheus göstəriciləri:

- `torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
- `torii_sorafs_proof_health_pdp_failures{provider_id}`
- `torii_sorafs_proof_health_potr_breaches{provider_id}`
- `torii_sorafs_proof_health_penalty_nano{provider_id}`
- `torii_sorafs_proof_health_cooldown{provider_id}`
- `torii_sorafs_proof_health_window_end_epoch{provider_id}`

**SoraFS PDP & PoTR Sağlamlıq** Grafana lövhəsi
(`dashboards/grafana/sorafs_pdp_potr_health.json`) indi həmin siqnalları ifşa edir:- *Trigger tərəfindən sübut edilmiş sağlamlıq xəbərdarlığı* tətik/cəza bayrağı ilə xəbərdarlıq dərəcələri qrafikləri
  Taikai/CDN operatorları yalnız PDP, yalnız PoTR və ya ikili xəbərdarlıqların olub olmadığını sübut edə bilər.
  atəş.
- *Cooldown-dakı Provayderlər* hazırda a altında olan provayderlərin canlı cəmini bildirir
  SorafsProofHealthAlert soyudulması.
- *Proof Sağlamlıq Pəncərəsi Snapshot* PDP/PoTR sayğaclarını, cərimə məbləğini birləşdirir,
  soyutma bayrağı və hər provayder üçün tətil pəncərəsinin bitmə dövrü, beləliklə idarəetmə rəyçiləri
  cədvəli insident paketlərinə əlavə edə bilər.

Runbooks DA icra sübutlarını təqdim edərkən bu panelləri əlaqələndirməlidir; onlar
CLI sübut axını uğursuzluqlarını birbaşa zəncirvari cəza metadatasına bağlayın və
yol xəritəsində göstərilən müşahidə çəngəlini təmin edin.