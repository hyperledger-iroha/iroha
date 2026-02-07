---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : simulation de capacité
titre : SoraFS کپیسٹی سمیولیشن رَن بُک
sidebar_label : کپیسٹی سمیولیشن رَن بُک
description: luminaires reproductibles, exports Prometheus, et tableaux de bord Grafana pour SF-2c چلانا۔
---

:::note ماخذِ مستند
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` کی آئینہ دار ہے۔ Il s'agit d'un Sphinx en pleine forme qui s'est avéré être un bon moyen de le faire. رکھیں۔
:::

Il s'agit d'un modèle SF-2c de type SF-2c. اور حاصل شدہ میٹرکس کیسے دیکھیں۔ `docs/examples/sorafs_capacity_simulation/` propose des appareils déterministes et des quotas pour le basculement et la réduction des corrections de bout en bout. کرتی ہے۔ Charges utiles pour `sorafs_manifest_stub capacity` Charges utiles pour `sorafs_manifest_stub capacity` manifest/CAR پیکجنگ کے لیے `iroha app sorafs toolkit pack` استعمال کریں۔

## 1. CLI آرٹی فیکٹس تیار کریں

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh`, `sorafs_manifest_stub capacity` pour les charges utiles Norito, les blobs base64, les corps de requête Torii et les résumés JSON pour les détails برائے:

- quotas de fournisseurs et de déclarations
- L'ordre de réplication et les manifestes par étapes et les fournisseurs et les fournisseurs de services de réplication.
- Ligne de base avant panne, intervalle de panne, récupération après basculement et instantanés de télémétrie
- panne simulée pour réduire la charge utile des litigesتمام آرٹی فیکٹس `./artifacts` کے تحت جمع ہوتے ہیں (پہلے آرگومنٹ میں مختلف ڈائریکٹری دے کر اووررائیڈ کر سکتے ہیں)۔ انسانی سمجھ کے لیے `_summary.json` فائلیں دیکھیں۔

## 2. Les métriques de votre choix et celles de votre choix

```bash
./analyze.py --artifacts ./artifacts
```

اینالائزر تیار کرتا ہے:

- `capacity_simulation_report.json` - Allocations supplémentaires, deltas de basculement et métadonnées de litige
- `capacity_simulation.prom` - Métriques de fichier texte Prometheus (`sorafs_simulation_*`) et collecteur de fichiers texte d'exportateur de nœuds et travail de grattage autonome

Configuration de grattage Prometheus ici :

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

collecteur de fichiers texte `capacity_simulation.prom` est un outil d'exportation (node-exporter est un outil d'exportation de nœud `--collector.textfile.directory`. میں کاپی کریں)۔

## 3. Grafana est en cours de réalisation

1. Grafana et `dashboards/grafana/sorafs_capacity_simulation.json` امپورٹ کریں۔
2. Source de données `Prometheus` et source de données pour la cible de grattage et la cible de grattage
3. پینلز چیک کریں:
   - **Allocation de quota (GiB)** pour le fournisseur et les soldes engagés/attribués
   - **Failover Trigger** métriques de panne en plus de *Failover Active* en ligne
   - ** Chute de disponibilité pendant une panne ** fournisseur `alpha` en cours de fonctionnement.
   - **Pourcentage Slash demandé** fixation des litiges et taux de remédiation élevé

## 4. متوقع چیکس- `sorafs_simulation_quota_total_gib{scope="assigned"}` pour `600` pour un total engagé >=600
- `sorafs_simulation_failover_triggered` et `1` pour la métrique du fournisseur de remplacement `beta` pour le fournisseur de remplacement.
- Fournisseur `sorafs_simulation_slash_requested` `alpha` pour `0.15` (barre oblique de 15 %)

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` Le schéma CLI est disponible pour les appareils et le schéma CLI.