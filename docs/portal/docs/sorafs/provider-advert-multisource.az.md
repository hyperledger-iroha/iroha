---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bb0965d4125aa2c3a3d483b63f4b36b1c6bf26406a2fd54e645e7a3c0156c264
source_last_modified: "2026-01-05T09:28:11.906678+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Çox Mənbəli Provayder Reklamları və Planlaşdırma

Bu səhifədə kanonik xüsusiyyətləri distillə edir
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Həmin sənəddən sözbəsöz Norito sxemləri və dəyişiklik qeydləri üçün istifadə edin; portal nüsxəsi
operator rəhbərliyini, SDK qeydlərini və telemetriya arayışlarını qalanlara yaxın saxlayır
SoraFS runbooks.

## Norito sxem əlavələri

### Aralıq qabiliyyəti (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – sorğuya görə ən böyük bitişik aralıq (bayt), `≥ 1`.
- `min_granularity` – həlli axtarın, `1 ≤ value ≤ max_chunk_span`.
- `supports_sparse_offsets` – bir sorğuda bitişik olmayan ofsetlərə icazə verir.
- `requires_alignment` – doğru olduqda, ofsetlər `min_granularity` ilə uyğunlaşdırılmalıdır.
- `supports_merkle_proof` – PoR şahid dəstəyini göstərir.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` kanonik kodlaşdırmanı tətbiq edir
belə ki, dedi-qodu yükləri deterministik olaraq qalır.

### `StreamBudgetV1`
- Sahələr: `max_in_flight`, `max_bytes_per_sec`, isteğe bağlı `burst_bytes`.
- Doğrulama qaydaları (`StreamBudgetV1::validate`):
  - `max_in_flight ≥ 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, mövcud olduqda, `> 0` və `≤ max_bytes_per_sec` olmalıdır.

### `TransportHintV1`
- Sahələr: `protocol: TransportProtocol`, `priority: u8` (0-15 pəncərə tərəfindən tətbiq edilir
  `TransportHintV1::validate`).
- Məlum protokollar: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Provayder üçün dublikat protokol girişləri rədd edilir.

### `ProviderAdvertBodyV1` əlavələr
- Könüllü `stream_budget: Option<StreamBudgetV1>`.
- Könüllü `transport_hints: Option<Vec<TransportHintV1>>`.
- Hər iki sahə indi `ProviderAdmissionProposalV1`, idarəetmə vasitəsilə axır
  zərflər, CLI qurğuları və telemetrik JSON.

## Qiymətləndirmə və idarəetmə məcburiyyəti

`ProviderAdvertBodyV1::validate` və `ProviderAdmissionProposalV1::validate`
səhv formalaşdırılmış metadatanı rədd edin:

- Diapazon imkanları deşifrə etməli və span/dənəvərlik məhdudiyyətlərini təmin etməlidir.
- Axın büdcələri / nəqliyyat göstərişləri uyğunluq tələb edir
  `CapabilityType::ChunkRangeFetch` TLV və boş olmayan işarə siyahısı.
- Dublikat daşıma protokolları və etibarsız prioritetlər validasiyanı artırır
  reklamlar qeybət edilməzdən əvvəl səhvlər.
- Qəbul zərfləri diapazon metadata üçün təklif/reklamları müqayisə edir
  `compare_core_fields` belə uyğunsuz qeybət yükləri erkən rədd edilir.

Reqressiya əhatə dairəsi yaşayır
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Alətlər və qurğular

- Provayderin reklam yüklərinə `range_capability`, `stream_budget` və
  `transport_hints` metadata. `/v2/sorafs/providers` cavabları vasitəsilə təsdiqləyin və
  qəbul qurğuları; JSON xülasələri təhlil edilmiş qabiliyyəti ehtiva etməlidir,
  axın büdcəsi və telemetriya qəbulu üçün göstəriş massivləri.
- `cargo xtask sorafs-admission-fixtures` axın büdcələrini və nəqliyyatı göstərir
  onun JSON artefaktları daxilində göstərişlər verir ki, idarə panelləri funksiyaların qəbulunu izləyir.
- `fixtures/sorafs_manifest/provider_admission/` altında olan qurğulara indi daxildir:
  - kanonik çoxmənbəli reklamlar,
  - `multi_fetch_plan.json` beləliklə SDK dəstləri deterministik multi-peer-i təkrarlaya bilər
    gətirmək planı.

## Orchestrator & Torii inteqrasiyası

- Torii `/v2/sorafs/providers` təhlil edilmiş diapazon qabiliyyəti metadatasını qaytarır
  `stream_budget` və `transport_hints` ilə. Endirmə xəbərdarlığı zamanı atəş
  provayderlər yeni metadatanı buraxır və şlüz aralığının son nöqtələri eyni şeyi tətbiq edir
  birbaşa müştərilər üçün məhdudiyyətlər.
- Çoxmənbəli orkestrator (`sorafs_car::multi_fetch`) indi diapazonu tətbiq edir
  iş təyin edərkən məhdudiyyətlər, imkanların uyğunlaşdırılması və axın büdcələri. Vahid
  testlər çox böyük, seyrək axtarış və tənzimləmə ssenarilərini əhatə edir.
- `sorafs_car::multi_fetch` endirmə siqnallarını verir (hizalanma uğursuzluqları,
  azaldılmış sorğular) operatorlar xüsusi provayderlərin niyə olduğunu izləyə bilsinlər
  planlaşdırma zamanı atlandı.

## Telemetriya arayışı

Torii diapazonu gətirmə cihazları **SoraFS Fetch Müşahidə Olunmasını** təmin edir.
Grafana idarə paneli (`dashboards/grafana/sorafs_fetch_observability.json`) və
qoşalaşmış xəbərdarlıq qaydaları (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Metrik | Növ | Etiketlər | Təsvir |
|--------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | Ölçü | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) Provayderlərin reklam diapazonu qabiliyyəti xüsusiyyətləri. |
| `torii_sorafs_range_fetch_throttle_events_total` | Sayğac | `reason` (`quota`, `concurrency`, `byte_rate`) | Siyasətə görə qruplaşdırılan sıxılmış diapazon gətirmə cəhdləri. |
| `torii_sorafs_range_fetch_concurrency_current` | Ölçü | — | Paylaşılan paralel büdcəni istehlak edən aktiv qorunan axınlar. |

Misal PromQL fraqmentləri:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Aktivləşdirmədən əvvəl kvota tətbiqini təsdiqləmək üçün tənzimləyici sayğacdan istifadə edin
çoxmənbəli orkestrator defoltları və paralellik yaxınlaşdıqda xəbərdar edin
donanmanız üçün maksimum büdcə axını.