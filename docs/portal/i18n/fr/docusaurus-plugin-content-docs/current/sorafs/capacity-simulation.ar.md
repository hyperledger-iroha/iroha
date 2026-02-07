---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : simulation de capacité
titre : دليل تشغيل محاكاة سعة SoraFS
sidebar_label : دليل محاكاة السعة
description: Appareils de montage pour luminaires SF-2c pour luminaires pour luminaires et luminaires Prometheus ولوحات Grafana.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. حافظ على تزامن النسختين إلى أن تُنقَل مجموعة توثيق Sphinx القديمة بالكامل.
:::

يشرح هذا الدليل كيفية تشغيل مجموعة محاكاة سوق السعة SF-2c وعرض المقاييس الناتجة. Il y a des possibilités de basculement et de slashing pour les luminaires. `docs/examples/sorafs_capacity_simulation/`. Pour les charges utiles, utilisez `sorafs_manifest_stub capacity`. استخدم `iroha app sorafs toolkit pack` لتدفقات تغليف manifest/CAR.

## 1. Utiliser les artefacts pour CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

Utiliser `run_cli.sh` pour `sorafs_manifest_stub capacity` pour les charges utiles et les blobs Norito en base64 et pour les Torii et JSON à:

- ثلاث تصريحات لمزوّدين مشاركين في سيناريو تفاوض الحصص.
- أمر نسخ يوزّع المانيفست المجهز عبر أولئك المزوّدين.
- Fonctions de basculement en cas de basculement.
- charge utile نزاع يطلب slashing بعد الانقطاع المُحاكَى.

Utilisez le code `./artifacts` (يمكن الاستبدال بتمرير دليل مختلف كأول وسيط). راجع ملفات `_summary.json` للحصول على سياق مقروء.

## 2. تجميع النتائج وإصدار المقاييس

```bash
./analyze.py --artifacts ./artifacts
```

يقوم المحلل بإنتاج:- `capacity_simulation_report.json` - Fonctions de basculement et de basculement.
- `capacity_simulation.prom` - Fichier texte utilisé pour Prometheus (`sorafs_simulation_*`) utilisé pour le collecteur de fichiers texte pour l'exportateur de nœuds et le travail de scrape.

Pour gratter le Prometheus :

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

Un collecteur de fichiers texte est `capacity_simulation.prom` (un exportateur de nœuds est utilisé comme `--collector.textfile.directory`).

## 3. استيراد لوحة Grafana

1. Dans Grafana, c'est `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Utilisez l'outil `Prometheus` pour gratter les fichiers.
3. تحقق من اللوحات:
   - **Allocation de quota (GiB)** يعرض الأرصدة الملتزم بها/المخصصة لكل مزوّد.
   - **Déclencheur de basculement** يتحول إلى *Failover Active* est également disponible.
   - **Chute de disponibilité pendant une panne** يرسم نسبة الفقدان للمزوّد `alpha`.
   - **Pourcentage Slash demandé** يعرض نسبة المعالجة المستخرجة من luminaire النزاع.

## 4. الفحوصات المتوقعة

- `sorafs_simulation_quota_total_gib{scope="assigned"}` ou `600` est égal à 600.
- `sorafs_simulation_failover_triggered` et `1` et `beta`.
- `sorafs_simulation_slash_requested` ou `0.15` (barre oblique de 15 %) pour `alpha`.

Utilisez `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` pour les luminaires avec la CLI.