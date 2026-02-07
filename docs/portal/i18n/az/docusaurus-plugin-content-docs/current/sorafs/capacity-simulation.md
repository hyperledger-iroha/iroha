---
id: capacity-simulation
lang: az
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Capacity Simulation Runbook
sidebar_label: Capacity Simulation Runbook
description: Exercising the SF-2c capacity marketplace simulation toolkit with reproducible fixtures, Prometheus exports, and Grafana dashboards.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
:::

Bu runbook SF-2c tutumlu bazar simulyasiya dəstini necə işə salmağı və nəticədə əldə edilən ölçüləri vizuallaşdırmağı izah edir. O, `docs/examples/sorafs_capacity_simulation/`-dəki deterministik qurğulardan istifadə edərək, kvota danışıqlarını, uğursuzluqla işləməyi və kəsintilərin aradan qaldırılmasını başdan sona doğrulayır. Tutumlu yüklər hələ də `sorafs_manifest_stub capacity`-dən istifadə edir; manifest/CAR qablaşdırma axınları üçün `iroha app sorafs toolkit pack` istifadə edin.

## 1. CLI artefaktları yaradın

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh`, Norito faydalı yükləri, base64 blobları, Torii sorğu gövdələrini və JSON xülasələrini yaymaq üçün `sorafs_manifest_stub capacity`-i əhatə edir:

- Kvota danışıqları ssenarisində iştirak edən üç provayder bəyannaməsi.
- Mərhələli manifesti həmin provayderlər arasında bölüşdürən replikasiya sifarişi.
- Kəsintidən əvvəl ilkin xətt, kəsilmə intervalı və uğursuzluğun bərpası üçün telemetriya snapshotları.
- Simulyasiya edilmiş kəsilmədən sonra kəsilmə tələb edən mübahisə yükü.

Bütün artefaktlar `./artifacts` altında düşür (ilk arqument kimi fərqli qovluğu keçməklə ləğv edin). `_summary.json` fayllarını insanların oxuya biləcəyi kontekst üçün yoxlayın.

## 2. Nəticələri birləşdirin və ölçüləri buraxın

```bash
./analyze.py --artifacts ./artifacts
```

Analizator istehsal edir:

- `capacity_simulation_report.json` - məcmu ayırmalar, uğursuzluq deltaları və mübahisə metadatası.
- `capacity_simulation.prom` - Prometheus mətn faylı ölçüləri (`sorafs_simulation_*`) qovşaq ixrac edən mətn faylı kollektoru və ya müstəqil kazıma işi üçün uyğundur.

Misal Prometheus kazıma konfiqurasiyası:

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

Mətn faylı kollektorunu `capacity_simulation.prom`-ə yönəldin (node-exporter istifadə edərkən onu `--collector.textfile.directory` vasitəsilə ötürülən qovluğa kopyalayın).

## 3. Grafana idarə panelini idxal edin

1. Grafana-də `dashboards/grafana/sorafs_capacity_simulation.json` idxal edin.
2. `Prometheus` məlumat mənbəyi dəyişənini yuxarıda konfiqurasiya edilmiş qırış hədəfinə bağlayın.
3. Panelləri yoxlayın:
   - **Kvota Ayrılması (GiB)** hər bir provayder üçün müəyyən edilmiş/təyin edilmiş qalıqları göstərir.
   - **Failover Trigger** kəsilmə göstəriciləri daxil olduqda *Failover Active* vəziyyətinə keçir.
   - **Kəsinti zamanı iş vaxtının azalması** provayder `alpha` üçün faiz itkisini göstərir.
   - **Tələb olunan Slash Faizi** mübahisə qurğusundan çıxarılan remediasiya nisbətini göstərir.

## 4. Gözlənilən yoxlamalar

- `sorafs_simulation_quota_total_gib{scope="assigned"}` `600`-ə bərabərdir, halbuki təhvil verilən cəmi >=600 qalır.
- `sorafs_simulation_failover_triggered` `1` hesabatını verir və əvəzedici provayder metrikası `beta` vurğulayır.
- `sorafs_simulation_slash_requested`, `alpha` provayder identifikatoru üçün `0.15` (15% kəsik) bildirir.

Qurğuların hələ də CLI sxemi tərəfindən qəbul edildiyini təsdiqləmək üçün `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit`-i işə salın.