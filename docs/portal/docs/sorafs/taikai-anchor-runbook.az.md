---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 50261b1f3173cd3916b29c81e85cc92ed8c14c38a0e0296be38397fe9b5c0596
source_last_modified: "2025-12-29T18:16:35.204852+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Taikai Anchor Müşahidə Kitabı

Bu portal nüsxəsi kanonik runbook-u əks etdirir
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md).
SoraFS/SoraNet üçün SN13-C marşrutlaşdırma-manifest (TRM) ankerlərini məşq edərkən istifadə edin
operatorlar spool artefaktlarını, Prometheus telemetriyasını və idarəetməni əlaqələndirə bilər
portalın önizləmə quruluşundan çıxmadan sübut.

## Əhatə və Sahiblər

- **Proqram:** SN13-C — Taikai manifestləri və SoraNS ankerləri.
- **Sahiblər:** Media Platforması WG, DA Proqramı, Networking TL, Docs/DevRel.
- **Məqsəd:** Sev1/Sev2 xəbərdarlığı, telemetriya üçün deterministik oyun kitabı təmin edin
  doğrulama və sübut ələ keçirərkən Taikai marşrutu irəliyə doğru hərəkət edir
  ləqəblər arasında.

## Sürətli Başlanğıc (Sev1/Sev2)

1. **Makara artefaktlarını ələ keçirin** — ən sonuncunu kopyalayın
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json` və
   `taikai-lineage-*.json` faylları
   `config.da_ingest.manifest_store_dir/taikai/` işçiləri yenidən işə başlamazdan əvvəl.
2. **Damp `/status` telemetriya** — qeyd edin
   Hansı manifest pəncərəsi olduğunu sübut etmək üçün `telemetry.taikai_alias_rotations` massivi
   aktiv:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **İdarə panellərini və xəbərdarlıqları yoxlayın** — yükləyin
   `dashboards/grafana/taikai_viewer.json` (klaster + axın filtrləri) və qeyd edin
   hər hansı bir qaydada olub-olmaması
   `dashboards/alerts/taikai_viewer_rules.yml` atəşə tutulub (`TaikaiLiveEdgeDrift`,
   `TaikaiIngestFailure`, `TaikaiCekRotationLag`, SoraFS sağlamlıq hadisələri).
4. **Inspect Prometheus** — təsdiq etmək üçün §“Metrik arayış”da sorğuları icra edin
   ingest latency/drift və alias-fırlanma sayğacları gözlənildiyi kimi davranır. Artırmaq
   `taikai_trm_alias_rotations_total` bir neçə pəncərə üçün dayanırsa və ya əgər
   səhv sayğacları artır.

## Metrik istinad

| Metrik | Məqsəd |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | CMAF klaster/axın üçün gecikmə histoqramını qəbul edir (hədəf: p95<750ms, p99<900ms). |
| `taikai_ingest_live_edge_drift_ms` | Kodlayıcı və lövbər işçiləri arasında canlı kənar sürüşmə (səhifələr p99>1,5s 10 dəqiqə ərzində). |
| `taikai_ingest_segment_errors_total{reason}` | Səbəbinə görə səhv sayğacları (`decode`, `manifest_mismatch`, `lineage_replay`, …). İstənilən artım `TaikaiIngestFailure`-i tetikler. |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | `/v2/da/ingest` ləqəb üçün yeni TRM qəbul etdikdə artır; fırlanma kadansını yoxlamaq üçün `rate()` istifadə edin. |
| `/status → telemetry.taikai_alias_rotations[]` | `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total` və sübut paketləri üçün vaxt ştampları ilə JSON snapshot. |
| `taikai_viewer_*` (rebufer, CEK fırlanma yaşı, PQ sağlamlığı, xəbərdarlıqlar) | CEK fırlanma + PQ dövrələrinin lövbər zamanı sağlam qalmasını təmin etmək üçün izləyici tərəfi KPI-lər. |

### PromQL fraqmentləri

```promql
histogram_quantile(
  0.99,
  sum by (le) (
    rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
)
```

```promql
sum by (reason) (
  rate(taikai_ingest_segment_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
)
```

```promql
rate(
  taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
)
```

## İdarə panelləri və xəbərdarlıqlar

- **Grafana izləyici lövhəsi:** `dashboards/grafana/taikai_viewer.json` — p95/p99
  gecikmə, canlı kənar sürüşmə, seqment səhvləri, CEK fırlanma yaşı, izləyici xəbərdarlıqları.
- **Grafana keş lövhəsi:** `dashboards/grafana/taikai_cache.json` — isti/isti/soyuq
  ləqəb pəncərələri fırlananda promosyonlar və QoS inkarları.
- **Alertmanager qaydaları:** `dashboards/alerts/taikai_viewer_rules.yml` — drift
  səhifələmə, qəbul etmə uğursuzluğu xəbərdarlığı, CEK fırlanma ləngiməsi və SoraFS sağlamlığının sübutu
  cəzalar/soyutma müddəti. Hər istehsal klasteri üçün qəbuledicilərin mövcud olduğundan əmin olun.

## Sübut paketinin yoxlama siyahısı

- Spool artefaktları (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`).
- Gözləyən/çatdırılmış zərflərin imzalanmış JSON inventarını buraxmaq üçün `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>`-i işə salın və sorğu/SSM/TRM/nəsil fayllarını qazma paketinə köçürün. Defolt spool yolu `torii.toml`-dən `storage/da_manifests/taikai`-dir.
- `telemetry.taikai_alias_rotations` əhatə edən `/status` snapshot.
- Hadisə pəncərəsi üzərində yuxarıdakı ölçülər üçün Prometheus ixracı (JSON/CSV).
- Filtrləri görünən Grafana ekran görüntüləri.
- Müvafiq qayda yanğınlara istinad edən Alertmanager ID-ləri.
- `docs/examples/taikai_anchor_lineage_packet.md`-i təsvir edən keçid
  kanonik sübut paketi.

## İdarə panelinin əks olunması və qazma kadansı

SN13-C yol xəritəsi tələbini yerinə yetirmək Taikai olduğunu sübut etmək deməkdir
baxıcı/keş panelləri lövbərin **və** portalında əks olunur
sübut qazma proqnozlaşdırıla bilən bir kadans üzərində işləyir.

1. **Portalın əks olunması.** İstənilən vaxt `dashboards/grafana/taikai_viewer.json` və ya
   `dashboards/grafana/taikai_cache.json` dəyişiklikləri, deltaları ümumiləşdirin
   `sorafs/taikai-monitoring-dashboards` (bu portal) və JSON-u qeyd edin
   portalın PR təsvirində yoxlama məbləğləri. Yeni panelləri/həddləri vurğulayın
   rəyçilər idarə olunan Grafana qovluğu ilə əlaqələndirə bilər.
2. **Aylıq məşq.**
   - Təlimi hər ayın ilk çərşənbə axşamı saat 15:00UTC-də yerinə yetirin
     SN13 idarəetmə sinxronizasiyasından əvvəl torpaqlar.
   - İçəridə spool artefaktları, `/status` telemetriya və Grafana ekran görüntülərini çəkin
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - İcranı qeyd edin
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
3. **Nəzərdən keçirin və dərc edin.** 48 saat ərzində xəbərdarlıqları/yalançı pozitivləri nəzərdən keçirin.
   DA Proqramı + NetOps, qazma jurnalında təqib elementlərini qeyd edin və əlaqə saxlayın
   `docs/source/sorafs/runbooks-index.md`-dən idarəetmə paketinin yüklənməsi.

Əgər alətlər panelləri və ya matkaplar geridə qalırsa, SN13-C çıxa bilməz 🈺; bunu saxla
kadans və ya sübut gözləntiləri dəyişdikdə bölmə yenilənir.

## Faydalı əmrlər

```bash
# Snapshot alias rotation telemetry to an artefact directory
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# List spool entries for a specific alias/event
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# Inspect TRM mismatch reasons from the spool log
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

Taikai zaman bu portal nüsxəsini kanonik runbook ilə sinxron saxlayın
ankraj telemetriyası, idarə panelləri və ya idarəetmə sübut tələbləri dəyişir.