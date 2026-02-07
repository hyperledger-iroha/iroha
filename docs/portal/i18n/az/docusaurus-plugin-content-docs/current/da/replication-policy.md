---
lang: az
direction: ltr
source: docs/portal/docs/da/replication-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

başlıq: Məlumatların Əlçatanlığının Replikasiya Siyasəti
sidebar_label: Replikasiya Siyasəti
təsvir: İdarəetmə tərəfindən tətbiq edilən saxlama profilləri bütün DA qəbulu təqdimatlarına tətbiq edilir.
---

:::Qeyd Kanonik Mənbə
:::

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
bütün `/v1/da/ingest` təqdimatları. Torii manifestləri məcburi olanlarla yenidən yazır
saxlama profili və zəng edənlər uyğun olmayan dəyərlər təqdim etdikdə xəbərdarlıq verir
operatorlar köhnəlmiş SDK-ları aşkar edə bilər.

### Taikai mövcudluğu dərsləri

Taikai marşrutlaşdırma manifestləri (`taikai.trm`) `availability_class` elan edir
(`hot`, `warm` və ya `cold`). Torii parçalanmadan əvvəl uyğunluq siyasətini tətbiq edir
beləliklə, operatorlar qlobal redaktə etmədən hər bir axın üçün replikaların sayını ölçə bilər
masa. Defoltlar:

| Mövcudluq sinfi | İsti tutma | Soyuq tutma | Tələb olunan replikalar | Saxlama sinfi | İdarəetmə etiketi |
|----------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 saat | 14 gün | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 saat | 30 gün | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 saat | 180 gün | 3 | `cold` | `da.taikai.archive` |

Çatışmayan göstərişlər defolt olaraq `hot`-dir, beləliklə canlı yayımlar ən güclü siyasəti saxlayır.
vasitəsilə defoltları ləğv edin
Şəbəkəniz istifadə edirsə, `torii.da_ingest.replication_policy.taikai_availability`
müxtəlif hədəflər.

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
redaktə edin `default_retention`.

Taikai mövcudluğu sinifləri vasitəsilə müstəqil olaraq ləğv edilə bilər
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## İcra semantikası

- Torii istifadəçi tərəfindən təmin edilmiş `RetentionPolicy`-i məcburi profillə əvəz edir
  parçalanmadan və ya açıq-aşkar emissiyadan əvvəl.
- Uyğunsuz saxlama profilini elan edən əvvəlcədən qurulmuş manifestlər rədd edilir
  `400 schema mismatch` ilə köhnə müştərilər müqaviləni zəiflədə bilməz.
- Hər ləğvetmə hadisəsi qeyd olunur (`blob_class`, gözlənilən siyasətə qarşı təqdim olunur)
  yayım zamanı uyğun gəlməyən zəng edənləri üzə çıxarmaq.

Yenilənmiş qapı üçün [Data Availability Ingest Plan](ingest-plan.md) (Təsdiqləmə yoxlama siyahısı) baxın
saxlanmanın icrasını əhatə edir.

## Yenidən təkrarlama iş axını (DA-4 təqibi)

Saxlamanın tətbiqi yalnız ilk addımdır. Operatorlar da bunu sübut etməlidirlər
canlı manifestlər və replikasiya sifarişləri konfiqurasiya edilmiş siyasətə uyğun olaraq qalır
ki, SoraFS uyğun olmayan blobları avtomatik olaraq təkrarlaya bilər.

1. **Drift üçün baxın.** Torii yayır
   `overriding DA retention policy to match configured network baseline` istənilən vaxt
   zəng edən köhnə saxlama dəyərlərini təqdim edir. Həmin log ilə cütləşdirin
   `torii_sorafs_replication_*` telemetriya replika çatışmazlıqlarını və ya gecikmələrini aşkar etmək üçün
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
   `docs/examples/da_manifest_review_template.md`
   Parlament səs paketi.
3. **Replikasiyanı işə salın.** Audit çatışmazlıq barədə məlumat verdikdə, yeni
   `ReplicationOrderV1` -də təsvir edilən idarəetmə alətləri vasitəsilə
   [SoraFS yaddaş tutumu bazarı](../sorafs/storage-capacity-marketplace.md) və auditi yenidən həyata keçirin
   replika dəsti birləşənə qədər. Fövqəladə hallar üçün CLI çıxışını cütləşdirin
   `iroha app da prove-availability` ilə, beləliklə, SRE-lər eyni digestə istinad edə bilər
   və PDP sübutları.

Reqressiya əhatə dairəsi `integration_tests/tests/da/replication_policy.rs`-də yaşayır;
paket `/v1/da/ingest`-ə uyğun olmayan saxlama siyasəti təqdim edir və təsdiqləyir
ki, gətirilən manifest zəng edənin əvəzinə məcburi profili ifşa edir
niyyət.