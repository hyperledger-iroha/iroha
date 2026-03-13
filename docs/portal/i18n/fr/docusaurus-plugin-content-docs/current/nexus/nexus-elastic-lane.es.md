---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-elastic-lane
titre : Provisionamiento de lane elastico (NX-7)
sidebar_label : Provisionamiento de lane elastico
description : Flux de bootstrap pour créer des manifestes de la voie Nexus, entrées de catalogue et preuves de déploiement.
---

:::note Fuente canonica
Cette page reflète `docs/source/nexus_elastic_lane.md`. Manten ambas copias alineadas hasta que el barrido de localisation se place al portal.
:::

# Kit de provisionamiento de voie élastique (NX-7)

> **Élément de la feuille de route :** NX-7 - outillage de provisionamiento de lane elastico  
> **État :** outillage complet - genres manifestes, extraits de catalogue, charges utiles Norito, essais d'humo,
> et l'aide du bundle de test de charge combine maintenant la latence pour le slot + les manifestes de preuve pour les couloirs de chargement des validateurs
> se publiquen sin scripting a medida.

Ce guide s'adresse aux opérateurs du nouveau helper `scripts/nexus_lane_bootstrap.sh` qui automatise la génération du manifeste de la voie, des extraits de catalogue de voie/espace de données et des preuves de déploiement. L'objectif est de faciliter la haute de nouvelles voies Nexus (publiques ou privées) sans éditer à la main plusieurs archives ni redériver la géométrie du catalogue à la main.

## 1. Prérequis1. Approbation de la gouvernance pour l'alias de lane, l'espace de données, le groupe des validateurs, la tolérance aux chutes (`f`) et la politique de règlement.
2. Une liste finale de validateurs (ID de compte) et une liste d'espaces de noms protégés.
3. Accédez au référentiel de configuration du nœud pour pouvoir ajouter les extraits de code générés.
4. Itinéraires pour le registre des manifestes de voie (ver `nexus.registry.manifest_directory` et `cache_directory`).
5. Contacts de télémétrie/poignées de PagerDuty pour la voie, de façon à ce que les alertes soient connectées lorsque la voie est en ligne.

## 2. Genres d'artefacts de voie

Exécutez l'assistant à partir de la racine du référentiel :

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Clé des drapeaux :- `--lane-id` doit coïncider avec l'index de la nouvelle entrée et `nexus.lane_catalog`.
- `--dataspace-alias` et `--dataspace-id/hash` contrôlent l'entrée du catalogue d'espace de données (par défaut, l'identifiant de la voie est omis).
- `--validator` peut répéter ou lire à partir de `--validators-file`.
- `--route-instruction` / `--route-account` émet des règles de liste d'enregistrement pour la lecture.
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) capture les contacts du runbook pour que les tableaux de bord nécessitent les correctifs des propriétaires.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` ajoutent le hook de runtime-upgrade au manifeste lorsque la voie nécessite des contrôles étendus des opérateurs.
- `--encode-space-directory` appelle automatiquement `cargo xtask space-directory encode`. Combinez-le avec `--space-directory-out` lorsque vous souhaitez que l'archive `.to` soit codifiée à un emplacement différent par défaut.

Le script produit trois artefacts à l'intérieur du `--output-dir` (par défaut du répertoire actuel), mais un quatre facultatif lorsqu'il autorise l'encodage :1. `<slug>.manifest.json` - manifeste de la voie qui contient le quorum des validateurs, les espaces de noms protégés et les métadonnées facultatives du hook de runtime-upgrade.
2. `<slug>.catalog.toml` - un extrait TOML avec `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` et toute règle d'inscription sollicitée. Assurez-vous que `fault_tolerance` est configuré dans l'entrée de l'espace de données pour dimensionner le comité de relais de voie (`3f+1`).
3. `<slug>.summary.json` - résumé des auditoires qui décrivent la géométrie (slug, segments, métadonnées) plus les étapes de déploiement requises et la commande exacte de `cargo xtask space-directory encode` (bas `space_directory_encode.command`). Ajoutez ce JSON au ticket d’intégration comme preuve.
4. `<slug>.manifest.to` - émis lorsque `--encode-space-directory` est activé ; liste pour le flux `iroha app space-directory manifest publish` de Torii.

Utilisez `--dry-run` pour prévisualiser les JSON/extraits sans écrire des archives, et `--force` pour décrire les artefacts existants.

## 3. Appliquer les changements1. Copiez le manifeste JSON dans le `nexus.registry.manifest_directory` configuré (et dans le cache du répertoire si le registre reflète les bundles distants). Comité d'archivage si les manifestes sont versionnés dans votre dépôt de configuration.
2. Annexe à l'extrait du catalogue à `config/config.toml` (ou au correspondant `config.d/*.toml`). Assurez-vous que `nexus.lane_count` se trouve au moins `lane_id + 1` et actualisez cualquier `nexus.routing_policy.rules` qui doit apparaître dans une nouvelle voie.
3. Codifier (si vous avez omis `--encode-space-directory`) et publier le manifeste dans le Space Directory en utilisant la commande capturée dans le résumé (`space_directory_encode.command`). Ceci produit la charge utile `.manifest.to` que Torii attend et enregistre la preuve pour les auditeurs ; envoyer avec `iroha app space-directory manifest publish`.
4. Exécutez `irohad --sora --config path/to/config.toml --trace-config` et archivez la sortie de trace sur le ticket de déploiement. Il s'agit de vérifier que la nouvelle géométrie coïncide avec les segments/slugs de Kura générés.
5. Réglez les validateurs assignés au moment où les changements de manifeste/catalogue sont épuisés. Conservez le résumé JSON dans le ticket pour les futurs auditoriums.

## 4. Construire un bundle de distribution du registreUtilisez le manifeste généré et la superposition pour que les opérateurs puissent distribuer les données de gestion des voies sans modifier les configurations sur chaque hôte. L'aide au regroupement des copies se manifeste dans la mise en page canonique, produit une superposition facultative du catalogue de gestion pour `nexus.registry.cache_directory`, et peut émettre une archive tar pour les transferts hors ligne :

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Salidas:

1. `manifests/<slug>.manifest.json` - copie de ces archives dans le `nexus.registry.manifest_directory` configuré.
2. `cache/governance_catalog.json` - déjà est-ce en `nexus.registry.cache_directory`. Chaque entrée `--module` est convertie en une définition de module enfichable, permettant les échanges du module de gestion (NX-2) pour actualiser la superposition de cache à la place de l'éditeur `config.toml`.
3. `summary.json` - comprend les hachages, les métadonnées de la superposition et les instructions pour les opérateurs.
4. Optionnel `registry_bundle.tar.*` - liste pour SCP, S3 ou trackers d'artefacts.

Synchronisez tout le répertoire (ou l'archive) avec chaque validateur, extrae en hosts air-gapped et copiez les manifestes + superposition de cache sur vos routes du registre avant de réinitialiser Torii.

## 5. Pruebas de humo para validadoresAprès avoir réinitialisé Torii, lancez le nouvel assistant de fumée pour vérifier que la voie rapporte `manifest_ready=true`, que les mesures étendent le contenu attendu des voies et que la jauge scellée est propre. Les voies qui nécessitent des manifestations doivent exposer un `manifest_path` no vacio ; L'assistant tombe maintenant immédiatement lorsqu'il échoue dans la route pour que chaque fois que le NX-7 est utilisé, il inclut les preuves du manifeste confirmé :

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v2/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Agrega `--insecure` lorsque vous effectuez des tests entornos avec certificats auto-signés. Le script se termine avec un code sans zéro si la voie est fausse, esta scellé ou les métriques/télémétries sont dessalinées des valeurs espérées. Utilisez les boutons `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` et `--max-headroom-events` pour maintenir la télémétrie de la voie (haute de blocage/finalité/backlog/headroom) dans vos limites opérationnelles, et combinés avec `--max-slot-p95` / `--max-slot-p99` (plus `--min-slot-samples`) pour imposer les objectifs de durée du slot NX-18 sans utiliser l'assistant.

Pour les validations air-gapped (ou CI), vous pouvez reproduire une réponse Torii capturée à la place d'un point final vivo :

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```Les appareils ont été récupérés sous `fixtures/nexus/lanes/` en réfléchissant aux artefacts produits par l'aide de bootstrap pour que les nouveaux manifestes puissent être lint-ear sans script à moyen. CI exécute le même flux via `ci/check_nexus_lane_smoke.sh` et `ci/check_nexus_lane_registry_bundle.sh` (alias : `make check-nexus-lanes`) pour démontrer que l'assistant de Smoke NX-7 est aligné avec le format de charge utile publié et pour garantir que les résumés/superpositions du bundle sont reproductibles.