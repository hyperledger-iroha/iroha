---
lang: az
direction: ltr
source: docs/settlement-router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 782429c90ac5df034fd7c8ff2c3acf4f9f11348f14f15fcd321f343b22b154b8
source_last_modified: "2025-12-29T18:16:35.914434+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Deterministik Hesablaşma Router (NX-3)

**Status:** Tamamlandı (NX-3)  
**Sahiblər:** İqtisadiyyat WG / Əsas Ledger WG / Treasury / SRE  
**Əhatə dairəsi:** Bütün zolaqlar/data məkanları tərəfindən istifadə edilən kanonik XOR hesablaşma yolu. Göndərilən marşrutlaşdırıcı qutu, zolaq səviyyəli qəbzlər, bufer qoruyucu relsləri, telemetriya və operator sübut səthləri.

## Məqsədlər
- Tək zolaqlı və Nexus qurğuları arasında XOR çevrilməsini və qəbz yaradılmasını birləşdirin.
- Operatorların hesablaşmanı təhlükəsiz şəkildə sürətləndirə bilməsi üçün mühafizə olunan buferlərlə deterministik saç düzümləri + dəyişkənlik marjaları tətbiq edin.
- Auditorların sifarişli alətlər olmadan təkrar oxuya biləcəyi qəbzləri, telemetriyanı və tablosunu ifşa edin.

## Memarlıq
| Komponent | Məkan | Məsuliyyət |
|----------|----------|----------------|
| Router primitivləri | `crates/settlement_router/` | Kölgə qiymət kalkulyatoru, saç düzümü səviyyələri, bufer siyasəti köməkçiləri, hesablaşma qəbzi növü.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】【crates/settlement_router.rs:1】
| Runtime fasad | `crates/iroha_core/src/settlement/mod.rs:1` | Router konfiqurasiyasını `SettlementEngine`-ə sarır, blokun icrası zamanı istifadə olunan `quote` + akkumulyatoru ifşa edir. |
| Blok inteqrasiyası | `crates/iroha_core/src/block.rs:120` | `PendingSettlement` qeydlərini boşaldır, hər zolaq/məlumat məkanı üçün `LaneSettlementCommitment` aqreqasiya edir, zolaq bufer metadatasını təhlil edir və telemetriya yayır. |
| Telemetriya və idarə panelləri | `crates/iroha_telemetry/src/metrics.rs:4847`, `dashboards/grafana/settlement_router_overview.json:1` | Prometheus/OTLP buferlər, variasiya, saç düzümü, konversiya sayları üçün ölçülər; SRE üçün Grafana lövhəsi. |
| İstinad sxemi | `docs/source/nexus_fee_model.md:1` | Sənədlərin hesablaşma qəbzi sahələri `LaneBlockCommitment`-də saxlanıldı. |

## Konfiqurasiya
Router düymələri `[settlement.router]` altında işləyir (`iroha_config` tərəfindən təsdiq edilmişdir):

```toml
[settlement.router]
twap_window_seconds = 60      # TWAP window used to derive local→XOR conversions
epsilon_bps = 25              # Base margin added to every quote (basis points)
buffer_alert_pct = 75         # Remaining-buffer % that opens an alert
buffer_throttle_pct = 25      # Remaining-buffer % where throttling begins
buffer_xor_only_pct = 10      # Remaining-buffer % where XOR-only mode is enforced
buffer_halt_pct = 2           # Remaining-buffer % where settlement halts
buffer_horizon_hours = 72     # Horizon (hours) represented by the XOR buffer
```

Hər bir verilənlər məkanı bufer hesabında zolaqlı metadata telləri:
- `settlement.buffer_account` — ehtiyatı saxlayan hesab (məsələn, `buffer::cbdc_treasury`).
- `settlement.buffer_asset` — aktiv tərifi başlıq boşluğu üçün debet edilir (adətən `xor#sora`).
- `settlement.buffer_capacity_micro` — mikro-XOR-da konfiqurasiya edilmiş tutum (onluq sətir).

Qeyri-mətaməlumat həmin zolaq üçün bufer snapşotunu deaktiv edir (temetriya sıfır tutum/vəziyyətə qayıdır).## Dönüşüm Boru Kəməri
1. **Sitat:** `SettlementEngine::quote` konfiqurasiya edilmiş epsilon + dəyişkənlik marjası və saç düzümü səviyyəsini TWAP kotirovkalarına tətbiq edir, `SettlementReceipt`-i `xor_due` və `xor_after_haircut` ilə qaytarır `source_id`.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】
2. **Yığım:** Blokun icrası zamanı icraçı `PendingSettlement` qeydlərini (yerli məbləğ, TWAP, epsilon, dəyişkənlik qutusu, likvidlik profili, oracle vaxt damğası) qeyd edir. `LaneSettlementBuilder` məcmuları toplayır və bloku möhürləməzdən əvvəl `(lane, dataspace)` üçün metadatanı dəyişdirin.【crates/iroha_core/src/settlement/mod.rs:34】【crates/iroha_core/src/block.rs:3460】
3. **Bufer snapshot:** Əgər zolaqlı metadata bufer elan edirsə, qurucu konfiqurasiya.【crates/iroha_3.core-dan `BufferPolicy` hədlərindən istifadə edərək `SettlementBufferSnapshot` (qalan boşluq, tutum, status) alır.
4. **Tədqiqat + telemetriya:** Qəbullar və dəyişdirmə sübutları `LaneBlockCommitment` daxilində yer alır və status snapshotlarına əks olunur. Telemetriya bufer ölçülərini, variasiyanı (`iroha_settlement_pnl_xor`), tətbiq olunan marjanı (`iroha_settlement_haircut_bp`), isteğe bağlı dəyişdirmə xətti istifadəsini və hər bir aktivə çevrilmə/saç kəsimi sayğaclarını qeyd edir, beləliklə, idarə panelləri və xəbərdarlıqlar blokla sinxronlaşdırılır. məzmun.【crates/iroha_core/src/block.rs:298】【crates/iroha_core/src/telemetry.rs:844】
5. **Sübut səthləri:** `status::set_lane_settlement_commitments` rele/DA istehlakçıları üçün öhdəlikləri dərc edir, Grafana idarə panelləri Prometheus göstəricilərini oxuyur və operatorlar `ops/runbooks/settlement-buffers.md`-dən I100 ilə yanaşı I100-dən istifadə edir doldurma/boğaz hadisələri.

## Telemetriya və Sübut
- `iroha_settlement_buffer_xor`, `iroha_settlement_buffer_capacity_xor`, `iroha_settlement_buffer_status` — hər zolaq/məlumat məkanı üçün bufer snapshot (mikro-XOR + kodlanmış vəziyyət).【crates/iroha_telemetry/src/metrics.rs:621】
- `iroha_settlement_pnl_xor` — blok dəsti üçün saç düzümündən sonrakı XOR ilə vaxtından əvvəl fərq aşkar edildi.【crates/iroha_telemetry/src/metrics.rs:6236】
- `iroha_settlement_haircut_bp` — partiyaya tətbiq olunan effektiv epsilon/saç kəsimi əsas nöqtələri.【crates/iroha_telemetry/src/metrics.rs:6244】
- `iroha_settlement_swapline_utilisation` — mübadilə sübutu mövcud olduqda likvidlik profilinə görə isteğe bağlı istifadə.【crates/iroha_telemetry/src/metrics.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — məskunlaşma dönüşümləri və kümülatif saç düzümü üçün hər zolaq/məlumat məkanı sayğacları (XOR vahidləri).
- Grafana lövhəsi: `dashboards/grafana/settlement_router_overview.json` (bufer boşluğu, fərq, saç düzümü) üstəgəl Nexus zolaq siqnalı paketinə daxil edilmiş Alertmanager qaydaları.
- Operator runbook: `ops/runbooks/settlement-buffers.md` (yenidən doldurma/xəbərdarlıq iş axını) və `docs/source/nexus_settlement_faq.md`-də tez-tez verilən suallar.## Tərtibatçı və SRE Yoxlama Siyahısı
- `[settlement.router]` dəyərlərini `config/config.json5` (və ya TOML)-da təyin edin və `irohad --version` jurnalları vasitəsilə doğrulayın; hədlərin `alert > throttle > xor_only > halt`-ə uyğun olmasını təmin edin.
- Zolaq metadatasını bufer hesabı/aktivi/tutumu ilə doldurun ki, bufer ölçüləri canlı ehtiyatları əks etdirsin; buferləri izləməməli olan zolaqlar üçün sahələri buraxın.
- `dashboards/grafana/settlement_router_overview.json` vasitəsilə `settlement_router_*` və `iroha_settlement_*` ölçülərini izləyin; tənzimləmə/yalnız XOR/dayanma vəziyyətlərində xəbərdarlıq.
- Qiymətləndirmə/siyasət əhatə dairəsi və `crates/iroha_core/src/block.rs`-də mövcud blok səviyyəli toplama testləri üçün `cargo test -p settlement_router`-i işə salın.
- `docs/source/nexus_fee_model.md`-də konfiqurasiya dəyişiklikləri üçün idarəetmə təsdiqlərini qeyd edin və eşiklər və ya telemetriya səthləri dəyişdikdə `status.md`-i yeniləyin.

## Yayım Planı Snapshot
- Hər quruluşda marşrutlaşdırıcı + telemetriya gəmisi; xüsusiyyət qapıları yoxdur. Zolaqlı metadata bufer snapşotlarının dərc edilib-edilməməsinə nəzarət edir.
- Defolt konfiqurasiya yol xəritəsi dəyərlərinə uyğun gəlir (60s TWAP, 25bp baza epsilon, 72h bufer horizontu); konfiqurasiya vasitəsilə tənzimləyin və tətbiq etmək üçün `irohad`-i yenidən başladın.
- Sübut paketi = zolaqlı hesablaşma öhdəlikləri + `settlement_router_*`/`iroha_settlement_*` seriyası üçün Prometheus qırıntısı + təsirlənmiş pəncərə üçün Grafana ekran görüntüsü/JSON ixracı.

## Sübut və İstinadlar
- NX-3 hesablaşma marşrutlaşdırıcısının qəbulu qeydləri: `status.md` (NX-3 bölməsi).
- Operator səthləri: `dashboards/grafana/settlement_router_overview.json`, `ops/runbooks/settlement-buffers.md`.
- Qəbz sxemi və API səthləri: `docs/source/nexus_fee_model.md`, `/v2/sumeragi/status` -> `lane_settlement_commitments`.