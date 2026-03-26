---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-elastic-lane
titre : Provisionnement de voie élastique (NX-7)
sidebar_label : Provisionnement de voie élastique
description : Workflow de bootstrap pour créer des manifestes de la voie Nexus, des entrées de catalogue et des preuves de déploiement.
---

:::note Source canonique
Cette page reprend `docs/source/nexus_elastic_lane.md`. Gardez les deux copies alignées jusqu'à ce que la vague de traduction arrive sur le portail.
:::

# Kit de provisionnement de voie élastique (NX-7)

> **Élément du roadmap:** NX-7 - outillage de provisionnement de voie élastique  
> **Statut :** outillage complet - génération des manifestes, des snippets de catalogue, des payloads Norito, des smoke tests,
> et le helper de bundle de load-test assemble maintenant le gating de latence par slot + des manifestes de preuve afin que les runs de charge validateurs
> peut être publié sans script sur mesure.

Ce guide accompagne les opérateurs via le nouveau helper `scripts/nexus_lane_bootstrap.sh` qui automatise la génération de manifestes de voie, des extraits de catalogue voie/espace de données et les preuves de déploiement. L'objectif est de faciliter la création de nouvelles voies Nexus (publiques ou privées) sans éditeur à la main plusieurs fichiers ni re-dériver à la main la géométrie du catalogue.

## 1. Prérequis1. Approbation de gouvernance pour l'alias de lane, le dataspace, l'ensemble des validateurs, la tolérance aux pannes (`f`) et la politique de règlement.
2. Une liste finale des validateurs (IDs de compte) et une liste de namespaces protégés.
3. Accédez au dépôt de configuration des nœuds afin de pouvoir ajouter les snippets génériques.
4. Chemins pour le registre de manifestes de voie (voir `nexus.registry.manifest_directory` et `cache_directory`).
5. Contacts télémétrie/poignées PagerDuty pour la voie afin que les alertes soient connectées des que la voie est en ligne.

## 2. Générer les artefacts de lane

Lancez le helper depuis la racine du dépôt :

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Clef de drapeaux :- `--lane-id` doit correspondre à l'index de la nouvelle entrée dans `nexus.lane_catalog`.
- `--dataspace-alias` et `--dataspace-id/hash` contrôlent l'entrée de catalogue du dataspace (par défaut, l'id du lane quand omis).
- `--validator` peut être répété ou lu depuis `--validators-file`.
- `--route-instruction` / `--route-account` emettent des règles de routage pretes a coller.
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) capture des contacts du runbook pour que les tableaux de bord affichent les bons propriétaires.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` ajoutent le hook runtime-upgrade au manifeste lorsque la voie requise des contrôles opérateurs étendus.
- `--encode-space-directory` invoque automatiquement `cargo xtask space-directory encode`. Combinez-le avec `--space-directory-out` quand vous voulez que le fichier `.to` encode aille ailleurs que le chemin par défaut.

Le script produit trois artefacts dans `--output-dir` (par défaut le répertoire courant), plus un quatrième optionnel lorsque l'encodage est actif :1. `<slug>.manifest.json` - manifest de lane contenant le quorum des validateurs, les espaces de noms protégés et des métadonnées optionnelles du hook runtime-upgrade.
2. `<slug>.catalog.toml` - un snippet TOML avec `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` et toute règle de routage demandée. Assurez-vous que `fault_tolerance` est défini sur l'entrée dataspace pour dimensionner le comité voie-relais (`3f+1`).
3. `<slug>.summary.json` - résumé d'audit décrivant la géométrie (slug, segments, métadonnées) plus les étapes de déploiement requises et la commande exacte `cargo xtask space-directory encode` (sous `space_directory_encode.command`). Joignez ce JSON au ticket d'onboarding comme preuve.
4. `<slug>.manifest.to` - émis lorsque `--encode-space-directory` est actif ; prêt pour le flux `iroha app space-directory manifest publish` de Torii.

Utilisez `--dry-run` pour prévisualiser les JSON/snippets sans écrire de fichiers, et `--force` pour effacer les artefacts existants.

## 3. Appliquer les changements1. Copiez le manifest JSON dans `nexus.registry.manifest_directory` configure (et dans le répertoire cache si le registre miroite des bundles distants). Validez le fichier si les manifestes sont versionnés dans votre dépôt de configuration.
2. Ajoutez l'extrait de catalogue à `config/config.toml` (ou au `config.d/*.toml` approprié). Assurez-vous que `nexus.lane_count` soit au moins `lane_id + 1`, et mettez à jour toute règle `nexus.routing_policy.rules` qui doit pointer vers la nouvelle voie.
3. Encodez (si vous avez sauté `--encode-space-directory`) et publiez le manifeste dans le Space Directory via la commande capturée dans le résumé (`space_directory_encode.command`). Cela produit le payload `.manifest.to` attendu par Torii et enregistre la preuve pour les audits ; soumettez via `iroha app space-directory manifest publish`.
4. Lancez `irohad --sora --config path/to/config.toml --trace-config` et archivez la trace de sortie dans le ticket de déploiement. Cela prouve que la nouvelle géométrie correspond aux segments Kura du genre slug.
5. Redemarrez les validateurs assignés à la voie une fois les changements manifestes/catalogue déployés. Conservez le résumé JSON dans le ticket pour les audits futurs.

## 4. Construire un bundle de distribution du registreEmpaquetez le manifeste générique et l'overlay afin que les opérateurs puissent distribuer les données de gouvernance des voies sans éditer les configs sur chaque hôte. L'assistant de bundling copie les manifestes dans le layout canonique, produit un overlay optionnel de catalogue de gouvernance pour `nexus.registry.cache_directory`, et peut émettre une archive tar pour les transferts hors ligne :

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Sorties :

1. `manifests/<slug>.manifest.json` - copiez-les dans `nexus.registry.manifest_directory` configure.
2. `cache/governance_catalog.json` - déposez dans `nexus.registry.cache_directory`. Chaque entrée `--module` devient une définition de module branchable, permettant des swap-outs de module de gouvernance (NX-2) en mettant à jour l'overlay de cache plutôt qu'en éditant `config.toml`.
3. `summary.json` - inclut les hachages, métadonnées d'overlay et instructions opérateur.
4. Optionnel `registry_bundle.tar.*` - prêt pour SCP, S3 ou des trackers d'artefacts.

Synchronisez le répertoire entier (ou l'archive) vers chaque validateur, extrayez sur des hotes air-gapped, et copiez les manifestes + overlay de cache dans leurs chemins de registre avant de redémarrer Torii.

## 5. Smoke tests des validateursAprès le redemarrage de Torii, lancez le nouveau helper de smoke pour vérifier que la voie rapporte `manifest_ready=true`, que les métriques exposent le nombre de voies attendu, et que la jauge scellée est vide. Les voies qui nécessitent des manifestes doivent exposer un `manifest_path` non vide ; le helper fait écho qu'il manque le chemin afin que chaque déploiement de NX-7 inclue la preuve du manifeste signé :

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
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

Ajoutez `--insecure` lorsque vous testez des environnements auto-signés. Le script sort avec un code non nul si la voie manque, est scellée, ou si les métriques/télémétrie dérivent des valeurs attendues. Utilisez les boutons `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` et `--max-headroom-events` pour maintenir la télémétrie par voie (hauteur de bloc/finalite/backlog/headroom) dans vos enveloppes opérationnelles, et couplez-les avec `--max-slot-p95` / `--max-slot-p99` (plus `--min-slot-samples`) pour imposer les objectifs de durée de slot NX-18 sans quitter le helper.

Pour les validations air-gapped (ou CI) vous pouvez rejouer une réponse Torii capturée au lieu d'interroger un endpoint live :

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
```Les luminaires enregistrés sous `fixtures/nexus/lanes/` reflètent les artefacts produits par le helper de bootstrap afin que les nouveaux manifestes puissent être lintes sans script sur mesure. La CI exécute le meme flux via `ci/check_nexus_lane_smoke.sh` et `ci/check_nexus_lane_registry_bundle.sh` (alias : `make check-nexus-lanes`) pour prouver que le helper de smoke NX-7 reste conforme au format de payload public et pour s'assurer que les digests/overlays du bundle restent reproductibles.