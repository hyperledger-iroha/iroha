---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : simulation de capacité
titre : Ранбук симуляции емкости SoraFS
sidebar_label : Ranbouk simulant des enregistrements
description: Vérifiez la simulation des composants SF-2c avec les configurations d'exportation Prometheus et Grafana.
---

:::note Канонический источник
Cette page indique `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Vous pouvez obtenir des copies de synchronisation si vous souhaitez créer des documents sur Sphinx qui ne sont pas disponibles.
:::

Cet objectif consiste à effectuer des simulations sur le SF-2c et à visualiser les mesures les plus pertinentes. Lors de la vérification des paramètres de sécurité, le basculement et la correction de bout en bout sont effectués en utilisant les configurations définies dans `docs/examples/sorafs_capacity_simulation/`. Les charges utiles peuvent utiliser `sorafs_manifest_stub capacity` ; используйте `iroha app sorafs toolkit pack` для потоков упаковки manifeste/CAR.

## 1. Générer des éléments CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` utilise `sorafs_manifest_stub capacity` pour utiliser les charges utiles Norito, base64-blob, comme Torii et les fichiers JSON pour :

- Trois déclarations de fournisseurs qui se rapportent aux scénarios passés par le bateau.
- Il s'agit d'une expédition de réplication, qui implique des opérations de simulation par étapes.
- Les paramètres télémétriques pour les lignes de base du serveur, l'interruption du basculement et le basculement.
- La charge utile est utilisée pour couper après la modification.Ces articles sont mis à jour dans le `./artifacts` (ils peuvent être consultés avant le catalogue des arguments). Vérifiez le fichier `_summary.json` pour votre conteneur.

## 2. Agréger les résultats et afficher les mesures

```bash
./analyze.py --artifacts ./artifacts
```

L'analyseur forme :

- `capacity_simulation_report.json` - configuration avancée, basculement en cas de panne et gestion des métadonnées.
- `capacity_simulation.prom` - mesures du fichier texte Prometheus (`sorafs_simulation_*`), adaptées au nœud-exportateur de collecteur de fichiers texte ou à un travail de scrape supplémentaire.

Configuration principale du scrape Prometheus :

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

Recherchez le collecteur de fichiers texte sur `capacity_simulation.prom` (en utilisant node-exporter, copiez-le dans le catalogue, avant `--collector.textfile.directory`).

## 3. Importation du bord Grafana

1. Dans Grafana, importez `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Sélectionnez la source de données `Prometheus` pour votre scrape.
3. Vérifiez les panneaux :
   - **Allocation de quota (GiB)** permet de valider/attribuer les soldes pour le fournisseur.
   - **Failover Trigger** est activé pour *Failover Active*, afin d'afficher les mesures correspondantes.
   - **Chute de disponibilité en cas de panne** est disponible pour le fournisseur `alpha`.
   - **Pourcentage de barre oblique demandé** визуализирует коэффициент ремедиации из фикстуры спора.

## 4. Les preuves de qualité- `sorafs_simulation_quota_total_gib{scope="assigned"}` correspond à `600`, si le commit est >=600.
- `sorafs_simulation_failover_triggered` indique `1`, une mesure fournie par le fournisseur indique `beta`.
- `sorafs_simulation_slash_requested` utilise `0.15` (barre oblique de 15 %) pour le fournisseur d'identification `alpha`.

Cliquez sur `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` pour modifier les configurations en utilisant le schéma CLI.