---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-default-lane-quickstart
titre : Guia rapida do lane padrao (NX-5)
sidebar_label : Guide rapide pour la voie padrao
description : Configurez et vérifiez le repli de la voie en utilisant Nexus pour que Torii et les SDK puissent omettre lane_id dans les voies publiques.
---

:::note Fonte canonica
Cette page espelha `docs/source/quickstart/default_lane.md`. Mantenha ambas as copias alinhadas ate que a revisao de localização chegue ao portal.
:::

# Guia rapida do lane padrao (NX-5)

> **Contexte de la feuille de route :** NX-5 - intégration de la voie publique. Le moment de l'exécution expose un repli `nexus.routing_policy.default_lane` pour les points de terminaison REST/gRPC de Torii et chaque SDK peut omettre la sécurité d'un `lane_id` lorsque le trafic relève de la voie publique canonique. Ce guide vous permet d'effectuer la configuration du catalogue, de vérifier ou de replier le `/status` et d'exercer le comportement du client de pont à pont.

## Prérequis

- Um build Sora/Nexus de `irohad` (execute `irohad --sora --config ...`).
- Accès au référentiel de configuration pour éditer les fichiers `nexus.*`.
- `iroha_cli` configuré pour fonctionner avec le cluster également.
- `curl`/`jq` (ou équivalent) pour inspecter la charge utile `/status` ou Torii.

## 1. Description du catalogue de voies et d'espaces de donnéesDéclarez les voies et les espaces de données qui doivent exister au réseau. Le trecho abaixo (enregistré par `defaults/nexus/config.toml`) enregistre trois voies publiques mais l'alias des correspondants de l'espace de données :

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

Chaque `index` doit être unique et contigu. Les identifiants de l'espace de données ont des valeurs de 64 bits ; os exemples acima usam os mesmos valeurs numériques que os indices de lane para major clareza.

## 2. Définir les coussins de rotation et les options de sélection

La section `nexus.routing_policy` contrôle la voie de repli et permet de surveiller le rotation pour des instructions spécifiques ou des préfixes de contact. Si vous n'avez pas de correspondant, le planificateur tourne vers le transfert pour les configurations `default_lane` et `default_dataspace`. La logique du routeur est vive dans `crates/iroha_core/src/queue/router.rs` et s'applique à la politique de forme transparente en tant que superficie REST/gRPC de Torii.

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

Lorsque vous ajoutez de nouvelles voies dans le futur, actualisez le premier catalogue et après cela comme regras de roteamento. La voie de secours doit continuer à être disponible pour la voie publique qui concentre la majeure partie du trafic des utilisateurs pour que les SDK alternatifs soient permanents et compatibles.

## 3. Démarrer un nœud avec une application politique

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Le nœud enregistre la politique de rotation dérivée lors du démarrage. Quaisquer les erreurs de validation (indices ausentes, alias duplicados, identifiants d'espace de données invalides) apparaissent avant le lancement de gossip.## 4. Confirmez l'état de gouvernance de la voie

Assurez-vous que le nœud est en ligne, utilisez l'aide de la CLI pour vérifier si la voie est fermée (manifeste chargé) et immédiatement pour le trafic. Un visa de curriculum vitae indique une ligne par voie :

```bash
iroha_cli app nexus lane-report --summary
```

Exemple de sortie :

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Si la voie doit être affichée `sealed`, consultez le runbook de gouvernance des voies avant de permettre le trafic externe. Un indicateur `--fail-on-sealed` est utilisé pour CI.

## 5. Inspecter l'état des charges utiles par Torii

La réponse `/status` expose la politique de rotation quant à l'instantané du planificateur par voie. Utilisez `curl`/`jq` pour confirmer les contrôleurs configurés et vérifier la voie de secours qui produit la télémétrie :

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

Pour inspecter les contadores vivants du planificateur pour la voie `0` :

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Cela confirme que l'instantané du TEU, les métadonnées de l'alias et les drapeaux du manifeste sont conformes à la configuration. La même charge utile est utilisée pour les tâches Grafana pour le tableau de bord d'acquisition de voie.

## 6. Exercer les parents du client- **Rust/CLI.** `iroha_cli` et le client de la caisse Rust omettent le champ `lane_id` lorsque vous ne passez pas à `--lane-id` / `LaneSelector`. Le routeur de fil, donc, cai em `default_lane`. Utilisez les drapeaux explicites comme `--lane-id`/`--dataspace-id` pour regarder la voie vers le haut.
- **JS/Swift/Android.** Dans les dernières versions, le SDK traite `laneId`/`lane_id` comme option et devient une solution de secours pour la valeur annoncée par `/status`. Il faut maintenir une politique de rotation synchronisée entre la mise en scène et la production pour que les applications se déplacent avec précision dans les reconfigurations d'émergence.
- **Tests Pipeline/SSE.** Les filtres d'événements de transaction sont conformes aux spécifications `tx_lane_id == <u32>` (voir `docs/source/pipeline.md`). Assine `/v2/pipeline/events/transactions` est un filtre pour prouver que vous avez écrit une voie explicite en utilisant l'identifiant de la voie de secours.

## 7. Observabilité et liens de gouvernance- `/status` également public `nexus_lane_governance_sealed_total` et `nexus_lane_governance_sealed_aliases` pour que le Alertmanager avise lorsqu'une voie perd son manifeste. Mantenha esses alertas habilados meme em devnets.
- La carte de télémétrie du planificateur et le tableau de bord de gouvernance des voies (`dashboards/grafana/nexus_lanes.json`) attendent les champs alias/slug du catalogue. Si vous renommez un alias, réétiquetez les directeurs des correspondants Kura pour que les auditeurs mantenham caminhos déterministes (rastreado sob NX-1).
- Les recommandations parlementaires pour les voies doivent inclure un plan de restauration. Enregistrez le hachage qui se manifeste et une preuve de gouvernance avec ce démarrage rapide dans votre runbook de l'opérateur pour que les futures rotations soient liées à l'état requis.

Une fois que ces vérifications ont été effectuées, vous pouvez traiter `nexus.routing_policy.default_lane` comme source d'information pour la configuration des SDK et désactiver les chemins de code alternatifs de manière unique sur le réseau.