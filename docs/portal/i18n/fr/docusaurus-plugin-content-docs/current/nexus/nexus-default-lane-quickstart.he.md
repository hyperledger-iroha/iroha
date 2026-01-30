---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 223ee04bc761f950f62ab381de26414295171d1f3e9df33bdfc7bc0901848a24
source_last_modified: "2025-11-14T04:43:20.385903+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Source canonique
Cette page reprend `docs/source/quickstart/default_lane.md`. Gardez les deux copies alignees jusqu'a ce que le balayage de localisation arrive sur le portail.
:::

# Guide rapide de la lane par defaut (NX-5)

> **Contexte roadmap:** NX-5 - integration de la lane publique par defaut. Le runtime expose desormais un fallback `nexus.routing_policy.default_lane` afin que les endpoints REST/gRPC de Torii et chaque SDK puissent omettre en toute securite un `lane_id` lorsque le trafic appartient a la lane publique canonique. Ce guide accompagne les operateurs pour configurer le catalogue, verifier le fallback dans `/status` et tester le comportement client de bout en bout.

## Prerequis

- Un build Sora/Nexus de `irohad` (lancer `irohad --sora --config ...`).
- Un acces au depot de configuration pour pouvoir modifier les sections `nexus.*`.
- `iroha_cli` configure pour parler au cluster cible.
- `curl`/`jq` (ou equivalent) pour inspecter le payload `/status` de Torii.

## 1. Decrire le catalogue des lanes et des dataspaces

Declarez les lanes et dataspaces qui doivent exister sur le reseau. L'extrait ci-dessous (tire de `defaults/nexus/config.toml`) enregistre trois lanes publiques ainsi que les alias de dataspace correspondants :

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

Chaque `index` doit etre unique et contigu. Les ids de dataspace sont des valeurs 64 bits ; les exemples ci-dessus utilisent les memes valeurs numeriques que les index de lane pour plus de clarte.

## 2. Definir les valeurs par defaut de routage et les surcharges optionnelles

La section `nexus.routing_policy` controle la lane de fallback et permet de surcharger le routage pour des instructions specifiques ou des prefixes de compte. Si aucune regle ne correspond, le scheduler route la transaction vers `default_lane` et `default_dataspace` configures. La logique du router vit dans `crates/iroha_core/src/queue/router.rs` et applique la politique de maniere transparente aux surfaces REST/gRPC de Torii.

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

Lorsque vous ajoutez plus tard de nouvelles lanes, mettez d'abord a jour le catalogue, puis etendez les regles de routage. La lane de fallback doit continuer a pointer vers la lane publique qui porte la majorite du trafic utilisateur afin que les SDK herites continuent de fonctionner.

## 3. Demarrer un noeud avec la politique appliquee

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Le noeud journalise la politique de routage derivee au demarrage. Toute erreur de validation (index manquants, alias dupliques, ids de dataspace invalides) est remontee avant le debut du gossip.

## 4. Confirmer l'etat de gouvernance de la lane

Une fois le noeud en ligne, utilisez l'helper CLI pour verifier que la lane par defaut est scellee (manifest charge) et prete pour le trafic. La vue recap affiche une ligne par lane :

```bash
iroha_cli app nexus lane-report --summary
```

Example output:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Si la lane par defaut affiche `sealed`, suivez le runbook de gouvernance des lanes avant d'autoriser du trafic externe. Le flag `--fail-on-sealed` est utile pour la CI.

## 5. Inspecter les payloads de statut de Torii

La reponse `/status` expose la politique de routage ainsi que l'instantane du scheduler par lane. Utilisez `curl`/`jq` pour confirmer les valeurs par defaut configurees et verifier que la lane de fallback produit de la telemetrie :

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Sample output:

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

Pour inspecter les compteurs live du scheduler pour la lane `0` :

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Cela confirme que l'instantane TEU, les metadonnees d'alias et les indicateurs de manifest s'alignent avec la configuration. Le meme payload est utilise par les panneaux Grafana pour le dashboard d'ingestion des lanes.

## 6. Tester les valeurs par defaut des clients

- **Rust/CLI.** `iroha_cli` et le crate client Rust omettent le champ `lane_id` lorsque vous ne passez pas `--lane-id` / `LaneSelector`. Le router de queue retombe donc sur `default_lane`. Utilisez les flags explicites `--lane-id`/`--dataspace-id` uniquement lorsque vous ciblez une lane non par defaut.
- **JS/Swift/Android.** Les dernieres versions des SDK traitent `laneId`/`lane_id` comme optionnels et retombent sur la valeur annoncee par `/status`. Gardez la politique de routage synchronisee entre staging et production afin que les apps mobiles n'aient pas besoin de reconfigurations d'urgence.
- **Pipeline/SSE tests.** Les filtres d'evenements de transactions acceptent les predicats `tx_lane_id == <u32>` (voir `docs/source/pipeline.md`). Abonnez-vous a `/v1/pipeline/events/transactions` avec ce filtre pour prouver que les ecritures envoyees sans lane explicite arrivent sous l'id de lane de fallback.

## 7. Observabilite et points d'accroche de gouvernance

- `/status` publie aussi `nexus_lane_governance_sealed_total` et `nexus_lane_governance_sealed_aliases` afin qu'Alertmanager puisse avertir lorsqu'une lane perd son manifest. Gardez ces alertes actives meme sur les devnets.
- La carte de telemetrie du scheduler et le dashboard de gouvernance des lanes (`dashboards/grafana/nexus_lanes.json`) attendent les champs alias/slug du catalogue. Si vous renommez un alias, reetiquetez les repertoires Kura correspondants afin que les auditeurs conservent des chemins deterministes (suivi sous NX-1).
- Les approbations parlementaires pour les lanes par defaut doivent inclure un plan de rollback. Enregistrez le hash du manifest et les preuves de gouvernance a cote de ce quickstart dans votre runbook operateur afin que les rotations futures n'aient pas a deviner l'etat requis.

Une fois ces verifications terminees, vous pouvez traiter `nexus.routing_policy.default_lane` comme la source de verite pour la configuration des SDK et commencer a desactiver les chemins de code mono-lane herites sur le reseau.
