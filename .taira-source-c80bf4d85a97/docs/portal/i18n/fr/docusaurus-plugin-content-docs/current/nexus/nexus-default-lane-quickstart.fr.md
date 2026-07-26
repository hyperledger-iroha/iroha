---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-default-lane-quickstart
titre : Guide rapide de la voie par défaut (NX-5)
sidebar_label : Guide rapide de la voie par défaut
description : Configurer et vérifier le repli de la voie par défaut de Nexus afin que Torii et les SDK puissent omettre lane_id sur les voies publiques.
---

:::note Source canonique
Cette page reprend `docs/source/quickstart/default_lane.md`. Gardez les deux copies alignées jusqu'à ce que le balayage de localisation arrive sur le portail.
:::

# Guide rapide de la voie par défaut (NX-5)

> **Feuille de route contextuelle :** NX-5 - intégration de la voie publique par défaut. Le runtime expose désormais un fallback `nexus.routing_policy.default_lane` afin que les endpoints REST/gRPC de Torii et chaque SDK puissent omettre en toute sécurité un `lane_id` lorsque le trafic appartient à la voie publique canonique. Ce guide accompagne les opérateurs pour configurer le catalogue, vérifier le repli dans `/status` et tester le comportement client de bout en bout.

## Prérequis

- Un build Sora/Nexus de `irohad` (lancer `irohad --sora --config ...`).
- Un accès au dépôt de configuration pour pouvoir modifier les sections `nexus.*`.
- `iroha_cli` configurer pour parler au cluster cible.
- `curl`/`jq` (ou équivalent) pour inspecter le payload `/status` de Torii.

## 1. Décrire le catalogue des voies et des espaces de donnéesDéclarez les voies et espaces de données qui doivent exister sur le réseau. L'extrait ci-dessous (tire de `defaults/nexus/config.toml`) enregistre trois voies publiques ainsi que les alias de dataspace correspondants :

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

Chaque `index` doit être unique et contigu. Les identifiants de l'espace de données sont des valeurs 64 bits ; les exemples ci-dessus utilisent les mèmes valeurs numériques que les index de voie pour plus de clarté.

## 2. Définir les valeurs par défaut de routage et les surcharges optionnelles

La section `nexus.routing_policy` contrôle la voie de repli et permet de surcharger le routage pour des instructions spécifiques ou des préfixes de compte. Si aucune règle ne correspond, le planificateur route la transaction vers `default_lane` et `default_dataspace` configure. La logique du routeur vit dans `crates/iroha_core/src/queue/router.rs` et applique la politique de manière transparente aux surfaces REST/gRPC de Torii.

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

Lorsque vous ajoutez plus tard de nouvelles voies, mettez d'abord un jour le catalogue, puis étendez les règles de routage. La voie de secours doit continuer à pointer vers la voie publique qui porte la majorité du trafic utilisateur afin que les SDK continuent de fonctionner.

## 3. Démarrer un nœud avec la politique appliquée

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```Le noeud journalise la politique de routage dérivée au démarrage. Toute erreur de validation (index manquants, alias dupliques, ids de dataspace invalides) est remontée avant le début du gossip.

## 4. Confirmer l'état de gouvernance de la voie

Une fois le noeud en ligne, utilisez l'helper CLI pour vérifier que la voie par défaut est sclée (manifest charge) et prête pour le trafic. La vue récapitulative affiche une ligne par voie :

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

Si la voie par défaut affiche `sealed`, suivez le runbook de gouvernance des voies avant d'autoriser le trafic externe. Le drapeau `--fail-on-sealed` est utile pour la CI.

## 5. Inspecter les charges utiles de statut de Torii

La réponse `/status` expose la politique de routage ainsi que l'instantane du planificateur par voie. Utilisez `curl`/`jq` pour confirmer les valeurs par défaut configurées et vérifier que la voie de repli produit de la télémétrie :

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

Pour inspecter les compteurs live du planning pour la voie `0` :

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Cela confirme que l'instantane TEU, les métadonnées d'alias et les indicateurs de manifeste s'alignent avec la configuration. Le meme payload est utilisé par les panneaux Grafana pour le tableau de bord d'ingestion des voies.

## 6. Tester les valeurs par défaut des clients- **Rust/CLI.** `iroha_cli` et le crate client Rust omettent le champ `lane_id` lorsque vous ne passez pas `--lane-id` / `LaneSelector`. Le routeur de file d'attente retombe donc sur `default_lane`. Utilisez les drapeaux explicites `--lane-id`/`--dataspace-id` uniquement lorsque vous ciblez une voie non par défaut.
- **JS/Swift/Android.** Les dernières versions du SDK traitent `laneId`/`lane_id` comme optionnelles et retombent sur la valeur annoncée par `/status`. Gardez la politique de routage synchronisée entre staging et production afin que les applications mobiles n'aient pas besoin de reconfigurations d'urgence.
- **Tests Pipeline/SSE.** Les filtres d'événements de transactions acceptent les prédicats `tx_lane_id == <u32>` (voir `docs/source/pipeline.md`). Abonnez-vous à `/v1/pipeline/events/transactions` avec ce filtre pour prouver que les écritures envoyées sans voie explicite arrivent sous l'id de voie de repli.

## 7. Observabilité et points d'accroche de gouvernance- `/status` publie aussi `nexus_lane_governance_sealed_total` et `nexus_lane_governance_sealed_aliases` afin qu'Alertmanager puisse éviter lorsqu'une voie perd son manifeste. Gardez ces alertes actives meme sur les devnets.
- La carte de télémétrie du planificateur et le tableau de bord de gouvernance des voies (`dashboards/grafana/nexus_lanes.json`) attend les champs alias/slug du catalogue. Si vous renommez un alias, réétiquetez les répertoires Kura correspondants afin que les auditeurs conservent des chemins déterministes (suivi sous NX-1).
- Les approbations parlementaires pour les voies par défaut doivent inclure un plan de démantèlement. Enregistrez le hash du manifeste et les preuves de gouvernance à côté de ce quickstart dans votre opérateur runbook afin que les rotations futures n'aient pas à deviner l'état requis.

Une fois ces vérifications terminées, vous pouvez traiter `nexus.routing_policy.default_lane` comme la source de vérité pour la configuration du SDK et commencer à désactiver les chemins de code mono-lane hérités sur le réseau.