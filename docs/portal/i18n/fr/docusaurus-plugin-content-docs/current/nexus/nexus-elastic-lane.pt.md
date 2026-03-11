---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-elastic-lane
titre : Provisionamento de lane elastico (NX-7)
sidebar_label : Provisionamento de lane elastico
description : Flux de bootstrap pour créer des manifestes de la voie Nexus, entrées de catalogue et preuves de déploiement.
---

:::note Fonte canonica
Cette page espelha `docs/source/nexus_elastic_lane.md`. Mantenha ambas as copias alinhadas ate que o scanning de localizacao chegue ao portal.
:::

# Kit de provisionnement de voie élastique (NX-7)

> **Élément de la feuille de route :** NX-7 - outillage de provisionamento de lane elastico  
> **Statut :** outillage complet - manifestes, extraits de catalogue, charges utiles Norito, tests de fumée,
> et l'aide du bundle de test de charge il y a peu de temps pour gérer la latence par emplacement + manifestes de preuves pour les tiges de charge des validateurs
> possam être publié sans script sob medida.

Ce guide utilise les opérateurs pour le nouvel assistant `scripts/nexus_lane_bootstrap.sh` qui automatise la gestion des manifestes de voie, des extraits de catalogue de voie/espace de données et des preuves de déploiement. L'objectif est de faciliter la création de nouvelles voies Nexus (publiques ou privées) en éditant manuellement plusieurs archives en redérivant manuellement la géométrie du catalogue.

## 1. Prérequis1. Approbation de la gouvernance de l'alias de lane, de l'espace de données, du partenariat des validateurs, de la tolérance aux erreurs (`f`) et de la politique de règlement.
2. Une liste finale des validateurs (ID de contact) et une liste des espaces de noms protégés.
3. Accédez au référentiel de configuration du nœud pour pouvoir ajouter les extraits de code générés.
4. Caminhos para o registro de manifests de lane (voir `nexus.registry.manifest_directory` et `cache_directory`).
5. Contacts de télémétrie/poignées de PagerDuty pour la voie, pour que les alertes soient connectées alors que la voie est en ligne.

## 2. Gere artefatos de lane

Exécutez l'assistant à partir du référentiel :

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

Les drapeaux portent :- `--lane-id` doit correspondre à l'index de la nouvelle entrée dans `nexus.lane_catalog`.
- `--dataspace-alias` et `--dataspace-id/hash` contrôlent l'entrée du catalogue de l'espace de données (pour utiliser l'identifiant de la voie en cas d'omission).
- `--validator` peut être répété ou lido de `--validators-file`.
- `--route-instruction` / `--route-account` émet des regras de roteamento prontas para colar.
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) capture les contacts du runbook pour que les tableaux de bord montrent les propriétaires corretos.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` ajoute le hook de runtime-upgrade au manifeste lorsque la voie demande des contrôles effectués par l'opérateur.
- `--encode-space-directory` appelle automatiquement `cargo xtask space-directory encode`. Combinez avec `--space-directory-out` lorsque vous souhaitez que l'archive `.to` soit codifiée pour un autre endroit par défaut.

Le script produit trois articles à l'intérieur du `--output-dir` (pour le répertoire actuel), mais un quart facultatif lorsque l'encodage est autorisé :1. `<slug>.manifest.json` - manifeste de la voie du quorum des validateurs, des espaces de noms protégés et des métadonnées optionnelles du hook de runtime-upgrade.
2. `<slug>.catalog.toml` - un extrait TOML avec `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` et tout ce qui concerne le transfert sollicité. Garanta que `fault_tolerance` est défini dans l'entrée de l'espace de données pour dimensionner le comité de relais de voie (`3f+1`).
3. `<slug>.summary.json` - résumé de l'auditoire décrivant la géométrie (slug, segments, métadonnées) mais les étapes de déploiement requises et la commande exécutée par `cargo xtask space-directory encode` (dans `space_directory_encode.command`). Il s'agit d'un JSON pour le ticket d'intégration comme preuve.
4. `<slug>.manifest.to` - émis lorsque `--encode-space-directory` est habilité ; Pronto para o fluxo `iroha app space-directory manifest publish` à Torii.

Utilisez `--dry-run` pour visualiser les JSON/extraits sans avoir à graver des archives et `--force` pour visualiser les artefatos existants.

## 3. Aplique comme mudancas1. Copiez le manifeste JSON pour la configuration `nexus.registry.manifest_directory` (et pour le répertoire de cache et le registre qui regroupe les télécommandes). Le Comité ou l'Arquivo se manifeste sao versionados no seu repositorio de configuracao.
2. Annexe ou extrait du catalogue dans `config/config.toml` (ou non `config.d/*.toml` approprié). Garanta que `nexus.lane_count` seja pelo moins `lane_id + 1` et actualiser quaisquer `nexus.routing_policy.rules` qui devam apontar para o novo lane.
3. Encodez (voir `--encode-space-directory`) et public ou manifestez aucun répertoire spatial en utilisant la commande capturée sans résumé (`space_directory_encode.command`). Il s'agit de produire la charge utile `.manifest.to` que la charge utile Torii attend et enregistre les preuves pour les auditeurs ; envie via `iroha app space-directory manifest publish`.
4. Exécutez `irohad --sora --config path/to/config.toml --trace-config` et archivez et ne tracez aucun ticket de déploiement. Cela prouve qu'une nouvelle géométrie correspond aux limaces/segments de Kura gerados.
5. Réinitialisez les validateurs attribués sur la voie lorsque le manifeste/catalogo est implanté. Mantenha ou summary JSON no ticket for auditoriums futurs.

## 4. Monter un bundle de distribution à partir du registre

Utilisez le manifeste et la superposition pour que les opérateurs puissent distribuer des données de gouvernance des voies sans modifier les configurations sur chaque hôte. L'assistant de regroupement de copies de manifestes pour la mise en page canonique, produit une superposition facultative du catalogue de gestion pour `nexus.registry.cache_directory` et peut émettre une archive tar pour les transferts hors ligne :```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Saïdas :

1. `manifests/<slug>.manifest.json` - copiez ces archives pour la configuration `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - coloque sur `nexus.registry.cache_directory`. Chaque entrée `--module` fera une définition du module de plug-in, permettant les échanges du module de gouvernance (NX-2) pour actualiser la superposition de cache lors de l'édition `config.toml`.
3. `summary.json` - comprend les hachages, les métadonnées de superposition et les instructions pour les opérateurs.
4. En option `registry_bundle.tar.*` - immédiatement pour SCP, S3 ou trackers d'artefatos.

Synchronisez le répertoire intérieur (ou l'archive) pour chaque validateur, extrayez les hôtes air-gapped et copiez les manifestes + superposition de cache pour vos chemins de registre avant de réinitialiser le Torii.

## 5. Tests de fumée des validateurs

Après que le Torii soit réinitialisé, exécutez le nouvel assistant de fumée pour vérifier le rapport de voie `manifest_ready=true`, comme mesure métrique du contagem attendu des voies et voyez la jauge scellée est vide. Lanes qui exigem manifeste devem expor um `manifest_path` nao vazio; L'assistant vient de tomber immédiatement lorsque le chemin échoue pour que chaque déploiement du NX-7 inclue une preuve de l'assassinat manifeste :

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
```Adicione `--insecure` pour tester l'environnement auto-signé. Le script sai avec le code nao zéro se o lane estiver ausente, scellé ou se metricas/telemetria divergirem dos valores esperados. Utilisez les boutons du système d'exploitation `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` et `--max-headroom-events` pour gérer la télémétrie de la voie (haute de bloc/finalité/backlog/headroom) dans vos limites opérationnelles et combiner avec `--max-slot-p95` / `--max-slot-p99` (mais `--min-slot-samples`) est important pour la durée de vie du slot NX-18 comme assistant.

Pour valider air-gapped (ou CI), vous pouvez reproduire une réponse Torii capturée lorsque vous accédez à un point de terminaison en direct :

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

Les luminaires sont gravés dans `fixtures/nexus/lanes/` et reflètent les artefatos produits par l'aide au bootstrap pour que les nouveaux manifestes puissent être lintados dans le script à moyen terme. Un CI exécute le même flux via `ci/check_nexus_lane_smoke.sh` et `ci/check_nexus_lane_registry_bundle.sh` (alias : `make check-nexus-lanes`) pour vérifier que l'aide de Smoke NX-7 continue à être compatible avec le format de charge utile publié et garantit que les résumés/superpositions font l'objet d'une reproduction fidèle.