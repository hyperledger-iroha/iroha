---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-default-lane-quickstart
titre : voie par défaut کوئیک اسٹارٹ (NX-5)
sidebar_label : voie par défaut
description : Nexus pour le repli par défaut des voies et configurer et vérifier les voies Torii pour les voies publiques des SDK et lane_id omettre les voies publiques
---

:::note Source canonique
یہ صفحہ `docs/source/quickstart/default_lane.md` کی عکاسی کرتا ہے۔ جب تک balayage de localisation پورٹل تک نہیں پہنچتی، دونوں کاپیوں کو aligné رکھیں۔
:::

# voie par défaut کوئیک اسٹارٹ (NX-5)

> **Contexte de la feuille de route :** NX-5 - intégration par défaut des voies publiques۔ runtime pour `nexus.routing_policy.default_lane` repli pour les utilisateurs et les points de terminaison Torii REST/gRPC et SDK pour `lane_id` طریقے سے omettre کر سکیں جب ٹریفک voie publique canonique سے تعلق رکھتا ہو۔ Les opérateurs de configuration et la configuration du catalogue `/status` vérifient le repli et effectuent un exercice de comportement client de bout en bout.

## Prérequis

- `irohad` pour Sora/Nexus build ( `irohad --sora --config ...` pour Sora).
- le référentiel de configuration contient les sections `nexus.*` modifier les sections
- `iroha_cli` et le cluster cible sont configurés pour être configurés
- La charge utile Torii `/status` inspecte les versions `curl`/`jq` (équivalent en یا).

## 1. voie et catalogue d'espaces de données pour les utilisateursréseau پر موجود ہونے والے voies et espaces de données کو déclarer کریں۔ Voici un extrait (`defaults/nexus/config.toml`) des voies publiques et un registre d'alias d'espace de données correspondant :

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

ہر `index` منفرد اور contigu ہونا چاہیے۔ Identifiants d'espace de données, valeurs 64 bits Il y a des index de voies et des valeurs numériques pour les index de voie et les valeurs numériques.

## 2. Valeurs par défaut du routage et remplacements facultatifs

`nexus.routing_policy` comprend une voie de repli et un contrôle de la voie ainsi que des instructions de service et des préfixes de compte pour le remplacement du routage. Il s'agit d'une correspondance de règle et d'un planificateur configuré `default_lane` et d'un itinéraire `default_dataspace` configuré. Logique du routeur `crates/iroha_core/src/queue/router.rs` pour Torii Surfaces REST/gRPC pour appliquer la logique de routeur

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


## 3. Comment démarrer le démarrage du nœud

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

démarrage du nœud et stratégie de routage dérivée Il y a des erreurs de validation (index manquants, alias dupliqués, identifiants d'espace de données invalides) et des potins.

## 4. État de gouvernance de voie کنفرم کریں

node online est disponible avec CLI helper et est prêt pour la voie par défaut scellée (manifeste chargé) et le trafic est prêt Vue récapitulative des voies et des rangées :

```bash
iroha_cli app nexus lane-report --summary
```

Exemple de sortie :

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```La voie par défaut `sealed` permet d'autoriser le trafic externe et le runbook de gouvernance de voie est également disponible. `--fail-on-sealed` drapeau CI pour le moment

## 5. Les charges utiles d'état Torii inspectent کریں

La politique de routage des réponses `/status` et l'instantané du planificateur de voies exposent les problèmes `curl`/`jq` Paramètres configurés par défaut pour la télémétrie de voie de repli produite ہے :

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

la voie `0` contient des compteurs de planificateur en direct:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Il s'agit d'un instantané TEU, de métadonnées d'alias et d'une configuration des drapeaux manifestes et d'un alignement. Panneaux de charge utile Grafana et tableau de bord d'acquisition de voie

## 6. Exercice sur les défauts de paiement des clients- **Rust/CLI.** `iroha_cli` dans le champ `lane_id` de la caisse client Rust et omettre le code de passe `--lane-id` / `LaneSelector`. کرتے۔ Utilisez le routeur de file d'attente `default_lane` pour une solution de secours Indicateurs explicites `--lane-id`/`--dataspace-id` pour une voie autre que celle par défaut et une cible et une voie non par défaut
- **JS/Swift/Android.** Les versions du SDK `laneId`/`lane_id` et la valeur de secours facultative et `/status` sont disponibles en option. کرتے ہیں۔ Politique de routage, mise en scène et production, synchronisation, applications mobiles, reconfigurations d'urgence, etc.
- **Tests Pipeline/SSE.** Filtres d'événements de transaction `tx_lane_id == <u32>` prédicats قبول کرتے ہیں (دیکھیں `docs/source/pipeline.md`). `/v1/pipeline/events/transactions` pour filtrer et s'abonner et écrire un identifiant de voie de secours تحت پہنچتی ہیں۔

## 7. Observabilité et crochets de gouvernance- `/status` `nexus_lane_governance_sealed_total` et `nexus_lane_governance_sealed_aliases` pour publier un message d'avertissement et un message d'avertissement Alertmanager sur la voie de manifeste. Les alertes et les devnets sont activés
- carte de télémétrie du planificateur et tableau de bord de gouvernance des voies (`dashboards/grafana/nexus_lanes.json`) catalogue et champs alias/slug attendus Vous pouvez renommer l'alias pour les répertoires Kura et renommer les chemins déterministes des auditeurs (NX-1 est une piste de suivi)
- voies par défaut et approbations du parlement et plan de démantèlement hachage manifeste et preuves de gouvernance et démarrage rapide et dossier d'exécution de l'opérateur et enregistrement et rotations futures et état et état.