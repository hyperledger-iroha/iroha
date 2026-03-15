---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-default-lane-quickstart
titre : Guia rapida del lane predeterminado (NX-5)
sidebar_label : Guide rapide de la voie prédéfinie
description : Configurez et vérifiez le repli de la voie prédéfinie de Nexus pour que Torii et le SDK puissent omettre lane_id dans les voies publiques.
---

:::note Fuente canonica
Cette page reflète `docs/source/quickstart/default_lane.md`. Manten ambas copias alineadas hasta que el barrido de localisation se place al portal.
:::

# Guide rapide de la voie prédéfinie (NX-5)

> **Contexte de la feuille de route :** NX-5 - intégration de la voie publique prédéfinie. Le runtime expose désormais un repli `nexus.routing_policy.default_lane` pour les points de terminaison REST/gRPC de Torii et chaque SDK peut omettre avec sécurité un `lane_id` lorsque le trafic perd la voie publique canonique. Ce guide aidera les opérateurs à configurer le catalogue, à vérifier le repli sur `/status` et à modifier le comportement du client de l'extrême à l'extrême.

## Prérequis

- Un build de Sora/Nexus de `irohad` (exécuté `irohad --sora --config ...`).
- Accédez au référentiel de configuration pour pouvoir éditer les sections `nexus.*`.
- `iroha_cli` configuré pour travailler avec l'objet cluster.
- `curl`/`jq` (ou équivalent) pour inspecter la charge utile `/status` de Torii.

## 1. Décrire le catalogue de voies et d'espaces de donnéesDéclarez les voies et les espaces de données qui doivent exister dans le rouge. Le fragment suivant (enregistré par `defaults/nexus/config.toml`) enregistre trois voies publiques mais les alias des correspondants de l'espace de données :

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

Chaque `index` doit être unique et contigu. Les identifiants de l'espace de données ont des valeurs de 64 bits ; Les exemples antérieurs utilisent les mêmes valeurs numériques que les indices de voie pour plus de clarté.

## 2. Configurer les valeurs prédéterminées d'inscription et les valeurs optionnelles

La section `nexus.routing_policy` contrôle la voie de secours et permet de décrire l'instruction pour des instructions spécifiques ou des paramètres de compte. Si cela ne coïncide pas, le planificateur organise la transaction entre les paramètres `default_lane` et `default_dataspace`. La logique du routeur est présente dans `crates/iroha_core/src/queue/router.rs` et applique la politique de forme transparente à la surface REST/gRPC de Torii.

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

Lorsque plus adelante agrée de nouvelles voies, actualise d'abord le catalogue et ensuite étend les règles d'enrutamiento. La voie de secours doit être suivie par la voie publique qui concentre la plus grande partie du trafic des utilisateurs pour que les SDK hérités fonctionnent.

## 3. Arranca un nœud avec la politique appliquée

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```Le nœud enregistre la politique d’engagement dérivée pendant l’arranque. Toute erreur de validation (indices erronés, alias dupliqués, identifiants d'espace de données invalides) se produit avant de commencer les potins.

## 4. Confirmer l'état de gouvernement de la voie

Une fois que le nœud est en ligne, utilisez l'aide de la CLI pour vérifier que la voie prédéterminée est vendue (manifeste chargé) et répertoriée pour le trafic. La vue de reprise imprime un fil par voie :

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

Si la voie prédéfinie doit être `sealed`, consultez le runbook de gestion des voies avant d'autoriser le trafic externe. Le drapeau `--fail-on-sealed` est utilisé pour CI.

## 5. Inspection des charges utiles de l'état Torii

La réponse `/status` expose tant la politique d'engagement que l'instantané du planificateur par voie. Utilisez `curl`/`jq` pour confirmer les valeurs prédéfinies configurées et vérifier que la voie de secours produit des télémétries :

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

Pour inspecter les contadores en vivo du planificateur pour la voie `0` :

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Ceci confirme que l'instantané de TEU, les métadonnées d'alias et les drapeaux du manifeste sont alignés avec la configuration. La même charge utile est celle qui utilise les panneaux Grafana pour le tableau de bord d'acquisition de voie.## 6. Éjecter les valeurs prédéterminées du client

- **Rust/CLI.** `iroha_cli` et le client de la caisse de Rust omettent le champ `lane_id` lorsqu'ils ne passent pas `--lane-id` / `LaneSelector`. Le routeur de colas pour cela revient à `default_lane`. Utilisez les drapeaux explicites `--lane-id`/`--dataspace-id` seulement lorsqu'ils s'appliquent à une voie non prédéterminée.
- **JS/Swift/Android.** Les dernières versions du SDK sont `laneId`/`lane_id` en option et ont la valeur de secours annoncée par `/status`. Gardez la politique de recrutement synchronisée entre la mise en scène et la production pour que les applications mobiles ne nécessitent pas de reconfigurations d'émergence.
- **Tests Pipeline/SSE.** Les filtres d'événements de transactions acceptent les prédicats `tx_lane_id == <u32>` (ver `docs/source/pipeline.md`). Abonnez-vous à `/v1/pipeline/events/transactions` avec ce filtre pour démontrer que les écritures envoyées sans une voie explicite utilisent l'identifiant de la voie de repli.

## 7. Observabilité et ganchos de gobernanza- `/status` également publica `nexus_lane_governance_sealed_total` et `nexus_lane_governance_sealed_aliases` pour qu'Alertmanager puisse aviser lorsqu'une voie traverse votre manifeste. Gardez ces alertes habilitées également dans les réseaux de développement.
- La carte de télémétrie du planificateur et le tableau de bord de gestion des voies (`dashboards/grafana/nexus_lanes.json`) attendent les champs alias/slug du catalogue. Si vous renommez un alias, regardez l'étiquette des directeurs Kura correspondants pour que les auditeurs suivent des itinéraires déterministes (en suivant sous NX-1).
- Les autorisations parlementaires pour les voies prédéterminées doivent inclure un plan de restauration. Enregistrez le hachage du manifeste et les preuves de gouvernance avec ce démarrage rapide dans votre runbook de l'opérateur pour que les rotations futures ne puissent pas déterminer l'état requis.

Une fois que ces transactions ne peuvent pas être effectuées, `nexus.routing_policy.default_lane` est la source de sécurité pour la configuration du SDK et vous permet de désactiver les routes de codage héritées de la voie unique sur le rouge.