---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-elastic-lane
titre : Voie de passage élastique (NX-7)
sidebar_label : Voie élastique
description : Programme Bootstrap pour la gestion des manifestes voie Nexus, déploiement du catalogue et de la documentation.
---

:::note Канонический источник
Cette page correspond à `docs/source/nexus_elastic_lane.md`. Veuillez sélectionner les copies de synchronisation, car vos fichiers ne seront pas affichés sur le portail.
:::

# Instruments pour la voie élastique (NX-7)

> **Feuille de route du produit :** NX-7 - Voie élastique pour les instruments  
> **État :** instruments utilisés - générateurs de manifestes, catalogue de fragments, charges utiles Norito, tests de fumée,
> Une aide pour le bundle de test de charge qui permet de gérer la latence de l'emplacement + les manifestes de documentation, les programmes de validation
> Vous pouvez publier sans scripts.

Cet article fournit aux opérateurs le nouveau helper `scripts/nexus_lane_bootstrap.sh`, qui génère automatiquement des collecteurs de voies, des catalogues de fragments de voies/espaces de données et déploiement de доказательств. Il est possible de créer de nouvelles voies Nexus (publiques ou privées) sans correction routière et sans géométrie particulière catalogue.

## 1. Pré-traitement1. Gestion de la gouvernance pour la voie d'alias, l'espace de données, les validateurs, la tolérance aux pannes (`f`) et le règlement politique.
2. Liste finale des validateurs (identifiants de compte) et liste des espaces de noms.
3. Après avoir installé les paramètres de configuration du référentiel, vous devez créer des fragments génériques.
4. Voie des collecteurs d'admission (avec `nexus.registry.manifest_directory` et `cache_directory`).
5. Contacts téléphoniques/poignées PagerDuty pour la voie, pour que les alertes soient envoyées plus tard en ligne.

## 2. Créez la voie des artéfacts

Utilisez l'assistant pour le dépôt de maïs :

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

Drapeaux clés :

- `--lane-id` doit être ajouté à l'index du nouvel emplacement dans `nexus.lane_catalog`.
- `--dataspace-alias` et `--dataspace-id/hash` permettent de stocker l'espace de données dans le catalogue (en utilisant la voie d'identification).
- `--validator` peut être activé ou sélectionné à partir de `--validators-file`.
- `--route-instruction` / `--route-account` vous permet d'accéder à la boutique en ligne.
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) fixe les contacts du runbook, les tableaux de bord fournis par les utilisateurs actuels.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` permettent la mise à niveau du temps d'exécution du crochet dans le manifeste, ainsi que la voie du contrôle des opérateurs.
- `--encode-space-directory` permet d'utiliser automatiquement `cargo xtask space-directory encode`. Utilisez votre `--space-directory-out` si vous souhaitez installer le code `.to` de votre choix.Le script contient trois éléments d'art dans `--output-dir` (dans le catalogue technique) et une conversion optionnelle en fonction de l'encodage activé :

1. `<slug>.manifest.json` - Voie de manifeste pour les validateurs de quorum, les espaces de noms et les métadonnées opérationnelles de mise à niveau du runtime du hook.
2. `<slug>.catalog.toml` - Fragment TOML avec `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` et запрошенными правилами маршрутизации. Alors, pour `fault_tolerance`, dans l'espace de données d'installation, vous devez configurer le comité de relais de voie (`3f+1`).
3. `<slug>.summary.json` - Contrôle de la géométrie (slug, segments, métadonnées), déploiement de plusieurs éléments et commande spécifique `cargo xtask space-directory encode` (après `space_directory_encode.command`). Utilisez ce JSON pour l'intégration et la documentation.
4. `<slug>.manifest.to` - compatible avec `--encode-space-directory` ; готов на потока Torii `iroha app space-directory manifest publish`.

Utilisez `--dry-run` pour activer JSON/fragments sans télécharger de fichiers et `--force` pour utiliser des fichiers supplémentaires. artéfacts.

## 3. Préparer la configuration1. Copiez le fichier JSON dans le répertoire `nexus.registry.manifest_directory` (et dans le répertoire de cache, si le registre stocke vos bundles). N'oubliez pas que si les versions des manifestes sont disponibles dans le référentiel de configuration.
2. Téléchargez le catalogue de fragments dans `config/config.toml` (ou dans le module `config.d/*.toml`). Assurez-vous que `nexus.lane_count` ne correspond pas à `lane_id + 1` et que vous ouvrez `nexus.routing_policy.rules` pour sélectionner une nouvelle voie.
3. Choisissez (avec le fournisseur `--encode-space-directory`) et ouvrez le manifeste dans Space Directory, en utilisant la commande du résumé (`space_directory_encode.command`). Cela concerne le `.manifest.to`, qui correspond au Torii, et fournit une documentation pour les auditeurs ; отправьте через `iroha app space-directory manifest publish`.
4. Cliquez sur `irohad --sora --config path/to/config.toml --trace-config` et archivez votre trace dans le déploiement de tickets. C'est ce qui explique que la nouvelle géométrie soit adaptée au segment général slug/Kura.
5. Sélectionnez les validateurs qui s'installent sur la voie, après avoir modifié le manuel/catalogue. Enregistrez le résumé JSON dans le formulaire pour les auditeurs.

## 4. Obtenez la livraison groupée

Utilisez le manifeste général et la superposition pour que les opérateurs puissent gérer la gouvernance des voies sans configuration de votre hôte. Helper permet de copier les manifestes dans la mise en page canonique, en permettant la superposition opérationnelle du catalogue de gouvernance pour `nexus.registry.cache_directory` et en permettant de créer une archive tar pour les versions officielles :

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Vous avez:1. `manifests/<slug>.manifest.json` - copiez-le dans le système `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - entrez dans `nexus.registry.cache_directory`. Vous devez installer le module `--module` pour activer le module de gouvernance (NX-2) lors de la mise à jour de la superposition de cache. вместо редактирования `config.toml`.
3. `summary.json` - pour vous aider, la superposition de métadonnées et les instructions pour les opérateurs.
4. Опционально `registry_bundle.tar.*` - objets pour SCP, S3 ou les objets de voyage.

Synchronisez votre catalogue (ou vos archives) avec le validateur, sauvegardez les hôtes à espacement aérien et copiez les manifestes + la superposition de cache dans votre réseau avant перезапуском Torii.

## 5. Validateurs de tests de fumée

Après l'utilisation du Torii, vous devez installer un nouvel assistant de fumée, qui doit vérifier la voie qui correspond au `manifest_ready=true`, les mesures permettant de déterminer la voie à suivre. количество voies et jauge scellée чист. Les voies, которым нужны manifestes, обязаны иметь непустой `manifest_path` ; helper vous permet de vous déplacer facilement lors de la mise en service, lorsque vous déployez le NX-7 en utilisant le manuel d'utilisation suivant :

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
```Ajoutez `--insecure` lors de tests auto-signés. Le script indique qu'il n'y a pas de code, si la voie est ouverte, scellée, ou les mesures/télémétries correspondent aux données des utilisateurs. Utilisez `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` et `--max-headroom-events` pour utiliser le télémètre sur la voie (vous bloc/détail final/backlog/headroom) dans les cadres appropriés, et sélectionnez `--max-slot-p95` / `--max-slot-p99` (et `--min-slot-samples`) pour la mise à jour NX-18 sur l'emplacement prévu, vous ne pouvez pas utiliser d'aide.

Pour le test à air isolé (ou CI), vous pouvez programmer la sortie Torii de votre point de terminaison :

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

La description des luminaires dans `fixtures/nexus/lanes/` fournit des éléments d'art, un assistant d'amorçage qui permet de créer de nouveaux manifestes qui peuvent être lintés sans assemblages scripts. CI indique que le pot est `ci/check_nexus_lane_smoke.sh` et `ci/check_nexus_lane_registry_bundle.sh` (alias : `make check-nexus-lanes`), qui est utilisé par Smoke Helper NX-7. La charge utile du format public et l'ensemble de résumés/superpositions sont disponibles.