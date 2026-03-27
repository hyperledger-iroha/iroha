---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/nexus/nexus-elastic-lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ff004bcd543385eb5500bdfe9c674312afffa06dd7cbd4856fe06c1da2ae1bd
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-elastic-lane
title: Provisionnement de lane elastique (NX-7)
sidebar_label: Provisionnement de lane elastique
description: Workflow de bootstrap pour creer des manifests de lane Nexus, des entrees de catalogue et des preuves de rollout.
---

:::note Source canonique
Cette page reprend `docs/source/nexus_elastic_lane.md`. Gardez les deux copies alignees jusqu'a ce que la vague de traduction arrive sur le portail.
:::

# Kit de provisionnement de lane elastique (NX-7)

> **Element du roadmap:** NX-7 - tooling de provisionnement de lane elastique  
> **Statut:** tooling complet - genere des manifests, des snippets de catalogue, des payloads Norito, des smoke tests,
> et le helper de bundle de load-test assemble maintenant le gating de latence par slot + des manifests de preuve afin que les runs de charge validateurs
> puissent etre publies sans scripting sur mesure.

Ce guide accompagne les operateurs via le nouveau helper `scripts/nexus_lane_bootstrap.sh` qui automatise la generation de manifests de lane, des snippets de catalogue lane/dataspace et les preuves de rollout. L'objectif est de faciliter la creation de nouvelles lanes Nexus (publiques ou privees) sans editer a la main plusieurs fichiers ni re-deriver a la main la geometrie du catalogue.

## 1. Prerequis

1. Approbation de gouvernance pour l'alias de lane, le dataspace, l'ensemble des validateurs, la tolerance aux pannes (`f`) et la politique de settlement.
2. Une liste finale des validateurs (IDs de compte) et une liste de namespaces proteges.
3. Acces au depot de configuration des noeuds afin de pouvoir ajouter les snippets generes.
4. Chemins pour le registry de manifests de lane (voir `nexus.registry.manifest_directory` et `cache_directory`).
5. Contacts telemetrie/handles PagerDuty pour la lane afin que les alertes soient connectees des que la lane est en ligne.

## 2. Generer les artefacts de lane

Lancez le helper depuis la racine du depot :

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Flags cle :

- `--lane-id` doit correspondre a l'index de la nouvelle entree dans `nexus.lane_catalog`.
- `--dataspace-alias` et `--dataspace-id/hash` controlent l'entree de catalogue du dataspace (par defaut, l'id du lane quand omis).
- `--validator` peut etre repete ou lu depuis `--validators-file`.
- `--route-instruction` / `--route-account` emettent des regles de routage pretes a coller.
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) capture des contacts de runbook pour que les dashboards affichent les bons owners.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` ajoutent le hook runtime-upgrade au manifest quand la lane requiert des controles operateur etendus.
- `--encode-space-directory` invoque automatiquement `cargo xtask space-directory encode`. Combinez-le avec `--space-directory-out` quand vous voulez que le fichier `.to` encode aille ailleurs que le chemin par defaut.

Le script produit trois artefacts dans `--output-dir` (par defaut le repertoire courant), plus un quatrieme optionnel quand l'encodage est active :

1. `<slug>.manifest.json` - manifest de lane contenant le quorum des validateurs, les namespaces proteges et des metadonnees optionnelles du hook runtime-upgrade.
2. `<slug>.catalog.toml` - un snippet TOML avec `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` et toute regle de routage demandee. Assurez-vous que `fault_tolerance` est defini sur l'entree dataspace pour dimensionner le comite lane-relay (`3f+1`).
3. `<slug>.summary.json` - resume d'audit decrivant la geometrie (slug, segments, metadonnees) plus les etapes de rollout requises et la commande exacte `cargo xtask space-directory encode` (sous `space_directory_encode.command`). Joignez ce JSON au ticket d'onboarding comme preuve.
4. `<slug>.manifest.to` - emis quand `--encode-space-directory` est active; pret pour le flux `iroha app space-directory manifest publish` de Torii.

Utilisez `--dry-run` pour previsualiser les JSON/snippets sans ecrire de fichiers, et `--force` pour ecraser les artefacts existants.

## 3. Appliquer les changements

1. Copiez le manifest JSON dans `nexus.registry.manifest_directory` configure (et dans le cache directory si le registry miroite des bundles distants). Committez le fichier si les manifests sont versionnes dans votre repo de configuration.
2. Ajoutez le snippet de catalogue a `config/config.toml` (ou au `config.d/*.toml` approprie). Assurez-vous que `nexus.lane_count` soit au moins `lane_id + 1`, et mettez a jour toute regle `nexus.routing_policy.rules` qui doit pointer vers la nouvelle lane.
3. Encodez (si vous avez saute `--encode-space-directory`) et publiez le manifest dans le Space Directory via la commande capturee dans le summary (`space_directory_encode.command`). Cela produit le payload `.manifest.to` attendu par Torii et enregistre la preuve pour les audits; soumettez via `iroha app space-directory manifest publish`.
4. Lancez `irohad --sora --config path/to/config.toml --trace-config` et archivez la sortie trace dans le ticket de rollout. Cela prouve que la nouvelle geometrie correspond aux segments Kura du slug genere.
5. Redemarrez les validateurs assignes a la lane une fois les changements manifest/catalogue deployes. Conservez le summary JSON dans le ticket pour les audits futurs.

## 4. Construire un bundle de distribution du registry

Empaquetez le manifest genere et l'overlay afin que les operateurs puissent distribuer les donnees de gouvernance des lanes sans editer les configs sur chaque hote. Le helper de bundling copie les manifests dans le layout canonique, produit un overlay optionnel de catalogue de gouvernance pour `nexus.registry.cache_directory`, et peut emettre un tarball pour les transferts offline :

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
2. `cache/governance_catalog.json` - deposez dans `nexus.registry.cache_directory`. Chaque entree `--module` devient une definition de module branchable, permettant des swap-outs de module de gouvernance (NX-2) en mettant a jour l'overlay de cache plutot qu'en editant `config.toml`.
3. `summary.json` - inclut les hashes, metadonnees d'overlay et instructions operateur.
4. Optionnel `registry_bundle.tar.*` - pret pour SCP, S3 ou des trackers d'artefacts.

Synchronisez le repertoire entier (ou l'archive) vers chaque validateur, extrayez sur des hotes air-gapped, et copiez les manifests + overlay de cache dans leurs chemins de registry avant de redemarrer Torii.

## 5. Smoke tests des validateurs

Apres le redemarrage de Torii, lancez le nouveau helper de smoke pour verifier que la lane rapporte `manifest_ready=true`, que les metriques exposent le nombre attendu de lanes, et que la jauge sealed est vide. Les lanes qui requierent des manifests doivent exposer un `manifest_path` non vide; le helper echoue des qu'il manque le chemin afin que chaque deploiement NX-7 inclue la preuve du manifest signe :

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

Ajoutez `--insecure` lorsque vous testez des environnements self-signed. Le script sort avec un code non zero si la lane manque, est sealed, ou si les metriques/telemetrie derivent des valeurs attendues. Utilisez les knobs `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` et `--max-headroom-events` pour maintenir la telemetrie par lane (hauteur de bloc/finalite/backlog/headroom) dans vos enveloppes operationnelles, et couplez-les avec `--max-slot-p95` / `--max-slot-p99` (plus `--min-slot-samples`) pour imposer les objectifs de duree de slot NX-18 sans quitter le helper.

Pour les validations air-gapped (ou CI) vous pouvez rejouer une reponse Torii capturee au lieu d'interroger un endpoint live :

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
```

Les fixtures enregistres sous `fixtures/nexus/lanes/` refletent les artefacts produits par le helper de bootstrap afin que les nouveaux manifests puissent etre lintes sans scripting sur mesure. La CI execute le meme flux via `ci/check_nexus_lane_smoke.sh` et `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) pour prouver que le helper de smoke NX-7 reste conforme au format de payload publie et pour s'assurer que les digests/overlays du bundle restent reproductibles.
