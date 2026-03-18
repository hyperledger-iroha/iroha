---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-default-lane-quickstart
title: Быстрый старт default lane (NX-5)
sidebar_label: Быстрый старт default lane
description : Enregistrez et vérifiez la voie par défaut de secours dans Nexus, tandis que Torii et le SDK peuvent ouvrir lane_id dans les voies publiques.
---

:::note Канонический источник
Эта страница отражает `docs/source/quickstart/default_lane.md`. Il est possible de copier des copies de synchronisation si la localisation n'est pas disponible sur le portail.
:::

# Voie par défaut de démarrage rapide (NX-5)

> **Контекст roadmap:** NX-5 - интеграция default public lane. L'option de secours `nexus.routing_policy.default_lane` est disponible pour l'entrée REST/gRPC Torii et le SDK peut être utilisé sans problème. `lane_id`, когда трафик относится к канонической public lane. Ce sont les opérateurs de fournisseurs à partir du catalogue qui fournissent une solution de secours dans `/status` et fournissent une réponse client à leur compte.

## Предварительные требования

- Сборка Sora/Nexus для `irohad` (запуск `irohad --sora --config ...`).
- Доступ к репозиторию конфигураций, чтобы редактировать секции `nexus.*`.
- `iroha_cli`, настроенный на целевой кластер.
- `curl`/`jq` (или эквивалент) для просмотра payload `/status` в Torii.

## 1. Описать каталог lane и dataspaceCréez des voies et des espaces de données qui doivent être utilisés dans ce domaine. Ce fragment (via `defaults/nexus/config.toml`) est enregistré sur une voie publique et un alias approprié pour l'espace de données :

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

Le boîtier `index` doit être unique et non disponible. Espaces de données d'identification - c'est une taille de 64 bits ; En premier lieu, vous utilisez les règles de sécurité, celles et les voies d'indexation, pour la navigation.

## 2. Supprimer les marchés et les opérations opérationnelles

La section `nexus.routing_policy` active la voie de secours et permet de pré-démarrer la mise en œuvre des instructions de construction ou des spécifications. аккаунтов. Si ce n'est pas possible, le planificateur effectue la transition entre `default_lane` et `default_dataspace`. Le routeur logique est installé dans `crates/iroha_core/src/queue/router.rs` et est probablement basé sur la politique de gestion Torii REST/gRPC.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```


## 3. Запустить ноду с примененной политикой

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Il n'y a plus de place pour le marché politique avant le début. Les autorisations de validation (d'indexation, de double alias, d'espaces de données d'identifiants non reconnus) révèlent les potins.

## 4. Améliorer la gouvernance de la voie

Ensuite, lorsque vous êtes en ligne, utilisez l'assistant CLI pour détecter la voie par défaut (manifest affiché) et le trafic. La vidéo s'applique à votre course sur la voie :

```bash
iroha_cli app nexus lane-report --summary
```

Exemple de sortie :

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```Si la voie par défaut utilise `sealed`, placez le runbook sur la gouvernance des voies avant ce thème, afin de gérer le trafic local. Le drapeau `--fail-on-sealed` est utilisé pour CI.

## 5. Vérifier l'état de la charge utile Torii

La réponse `/status` a permis de créer un trafic politique et un planificateur d'écrans sur les voies. Utilisez `curl`/`jq` pour mettre à jour les paramètres de sauvegarde et de vérification afin de publier la voie de secours. télémétrie :

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Exemple de sortie :

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

Comment activer le planificateur de séquences pour la voie `0` :

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Il s'agit d'un instantané TEU, d'un alias de métadonnées et d'un drapeau manifeste qui correspondent à la configuration. Cette charge utile utilise les panneaux Grafana pour l'absorption des voies du tableau de bord.

## 6. Vérifier la disponibilité du client- **Rust/CLI.** `iroha_cli` et la caisse client Rust utilisent le pôle `lane_id`, mais vous ne devez pas remplacer `--lane-id` / `LaneSelector`. Le routeur de file d'attente est connecté à `default_lane`. Utilisez vos drapeaux `--lane-id`/`--dataspace-id` uniquement pour activer une voie qui n'est pas celle par défaut.
- **JS/Swift/Android.** Les versions ultérieures du SDK incluent `laneId`/`lane_id`, options et solutions de secours disponibles dans `/status`. Les politiques de marketing de synchronisation ne permettent pas de faire de la mise en scène et de la production des projets mobiles.
- **Tests Pipeline/SSE.** Les filtres sont basés sur les prédicats `tx_lane_id == <u32>` (avec `docs/source/pipeline.md`). Ajoutez le filtre `/v1/pipeline/events/transactions` à ce filtre pour obtenir un identifiant de voie de repli.

## 7. Observabilité et crochets de gouvernance- `/status` publie `nexus_lane_governance_sealed_total` et `nexus_lane_governance_sealed_aliases`, ce qui permet à Alertmanager de se présenter lorsque la voie est affichée. Obtenez ces alertes directement sur Devnet.
- Le planificateur télémétrique et la gouvernance du tableau de bord pour les voies (`dashboards/grafana/nexus_lanes.json`) indiquent l'alias/slug du catalogue. Si vous choisissez un alias, choisissez et contactez le directeur Kura, les auditeurs sont responsables de la détection des choses (ensuite sur NX-1).
- L'évolution des voies par défaut permet de restaurer le plan. Si vous souhaitez télécharger le manifeste de hachage et préparer le processus de gouvernance avec ce démarrage rapide dans votre runbook d'opérateur, vous n'avez pas vraiment besoin de gérer la rotation.

Lors de ces vérifications, vous pouvez choisir `nexus.routing_policy.default_lane` pour installer les fichiers de configuration du SDK et désactiver l'option de configuration. les copropriétés à voie unique sont situées à Seti.