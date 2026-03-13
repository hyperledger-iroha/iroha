---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-elastic-lane
titre : لچکدار voie پروویژنگ (NX-7)
sidebar_label : لچکدار lane پروویژنگ
description : Manifestes de voies Nexus, entrées de catalogue, et preuves de déploiement pour le bootstrap et le démarrage
---

:::note Source canonique
یہ صفحہ `docs/source/nexus_elastic_lane.md` کی عکاسی کرتا ہے۔ جب تک ترجمہ پورٹل تک نہیں پہنچتا، دونوں کاپیوں کو aligné رکھیں۔
:::

# لچکدار voie پروویژنگ ٹول کٹ (NX-7)

> **Élément de la feuille de route :** NX-7 - Outillage de construction de voies de circulation  
> **Statut :** outils - manifestes, extraits de catalogue, charges utiles Norito, tests de fumée et autres
> Un assistant de bundle de tests de charge, un contrôle de latence de slot + des manifestes de preuves, des validateurs et des exécutions de chargement
> Écriture de scripts pour tous les types de scripts

Les opérateurs d'assistance et l'assistant `scripts/nexus_lane_bootstrap.sh` sont en charge de la génération de manifeste de voie, des extraits de catalogue de voie/espace de données et des preuves de déploiement. بناتا ہے۔ Il existe plusieurs voies Nexus (publiques et privées) pour modifier la géométrie du catalogue. ہاتھ سے dériver کیے بغیر آسانی سے بنائی جا سکیں۔

## 1. Prérequis1. Alias ​​de voie, espace de données, ensemble de validateurs, tolérance aux pannes (`f`), politique de règlement et approbation de la gouvernance.
2. validateurs et espaces de noms protégés (identifiants de compte) et espaces de noms protégés
3. Le référentiel de configuration des nœuds contient des extraits générés pour chaque élément
4. Registre des manifestes de voie et chemins d'accès (دیکھیں `nexus.registry.manifest_directory` et `cache_directory`).
5. Voie avec contacts de télémétrie/PagerDuty gère les alertes de voie en ligne ou filaire ou filaire

## 2. artefacts de voie بنائیں

racine du référentiel et assistant ici :

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

Indicateurs clés :

- `--lane-id` et `nexus.lane_catalog` contiennent une entrée et un index correspondant à une correspondance
- `--dataspace-alias` et `--dataspace-id/hash` entrée de catalogue d'espace de données et contrôle des éléments (omettre l'identifiant de voie par défaut pour les utilisateurs).
- `--validator` et répéter pour le moment et `--validators-file` pour le moment.
- Les règles de routage prêtes à coller `--route-instruction` / `--route-account` émettent des mots de passe
- Les contacts du runbook `--metadata key=value` (`--telemetry-contact/channel/runbook`) capturent les tableaux de bord des propriétaires et des propriétaires.
- Crochet de mise à niveau d'exécution `--allow-runtime-upgrades` + `--runtime-upgrade-*` et manifeste pour les commandes de voie et les commandes étendues de l'opérateur.
- `--encode-space-directory` خودکار طور پر `cargo xtask space-directory encode` چلاتا ہے۔ `--space-directory-out` est par défaut et est par défaut `.to` par défaut.Il s'agit d'un `--output-dir` qui contient des artefacts en blanc (répertoire par défaut en blanc) et un encodage en haut de la page. چوتھا بھی بنتا ہے:

1. `<slug>.manifest.json` - manifeste de voie pour le quorum du validateur, les espaces de noms protégés et les métadonnées du hook de mise à niveau d'exécution pour les utilisateurs
2. `<slug>.catalog.toml` - Un extrait de code TOML pour `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]`, et des règles de routage supplémentaires pour les règles de routage. entrée d'espace de données میں `fault_tolerance` لازمی طور پر set کریں تاکہ comité de relais de voie (`3f+1`) درست سائز ہو۔
3. `<slug>.summary.json` - Résumé de l'audit et géométrie (slug, segments, métadonnées) et étapes de déploiement requises pour `cargo xtask space-directory encode` et commande exacte (`space_directory_encode.command` pour les détails) pour la commande exacte. Ajouter un ticket d'embarquement avec une preuve et joindre une pièce jointe
4. `<slug>.manifest.to` - `--encode-space-directory` pour la recherche en ligne Torii et `iroha app space-directory manifest publish` flux déjà prêt

`--dry-run` et JSON/extraits pour afficher l'aperçu Aperçu et `--force` pour écraser les artefacts

## 3. تبدیلیاں لاگو کریں1. Le manifeste JSON est configuré `nexus.registry.manifest_directory` pour le miroir (le répertoire de cache est pour le miroir des bundles distants du registre). Il manifeste un dépôt de configuration avec une version versionnée et un commit commit
2. extrait de catalogue `config/config.toml` (pour `config.d/*.toml`) et ajouter un extrait de catalogue `nexus.lane_count` est en cours d'exécution `lane_id + 1` est en cours d'exécution dans la voie de circulation `nexus.routing_policy.rules` est en cours d'exécution
3. Encoder le fichier (`--encode-space-directory` pour le fichier) dans Space Directory et publier le manifeste. résumé de la commande de commande (`space_directory_encode.command`) استعمال کریں۔ Il s'agit de la charge utile `.manifest.to` pour les auditeurs et les preuves. `iroha app space-directory manifest publish` کے ذریعے soumettre کریں۔
4. `irohad --sora --config path/to/config.toml --trace-config` pour la sortie de trace et le ticket de déploiement et les archives Il existe de nombreux segments slug/Kura générés par la géométrie.
5. Manifeste/catalogue Déployez et redémarrez la voie et les validateurs assignés redémarrent. audits futurs et résumé JSON et ticket

## 4. bundle de distribution de registre ici

manifeste généré et superposition et package pour les opérateurs et les hôtes et les configurations modifier et distribuer les données de gouvernance de voie. bundler helper manifeste une mise en page canonique pour les transferts hors ligne `nexus.registry.cache_directory` pour la superposition du catalogue de gouvernance pour les transferts hors ligne pour l'archive tar ہے :

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Sorties :1. `manifests/<slug>.manifest.json` - Configuration configurée `nexus.registry.manifest_directory` pour la configuration
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` میں رکھیں۔ L'entrée `--module` définit la définition du module enfichable pour les échanges de modules de gouvernance (NX-2) et la superposition de cache. کرنا پڑتا ہے، `config.toml` میں ترمیم نہیں۔
3. `summary.json` - hachages, métadonnées de superposition et instructions de l'opérateur ici
4. `registry_bundle.tar.*` en option - SCP, S3 et traqueurs d'artefacts prêts à l'emploi

Répertoire (archives) et validateur, synchronisation, hôtes avec espacement d'air, extrait et redémarrage Torii, manifestes + superposition de cache et chemins de registre. میں کاپی کریں۔

## 5. Tests de fumée du validateur

Torii redémarrage pour les métriques du nombre de voies attendues et jauge scellée صاف ہو۔ جن voies کو manifestes درکار ہوں انہیں non vide `manifest_path` ظاہر کرنا چاہیے؛ helper اب path غائب ہونے پر فوراً fail ہوتا ہے تاکہ ہر Déploiement NX-7 میں preuve manifeste signée شامل ہو:

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
```Les environnements auto-signés sont basés sur `--insecure`. Voie fermée ou scellée ou métriques/télémétrie valeurs attendues ou dérive ou script sortie non nulle? `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog`, et `--max-headroom-events` pour la télémétrie de la hauteur de bloc au niveau de la voie/de la finalité/de l'arriéré/de la hauteur libre et de l'enveloppe opérationnelle Le modèle `--max-slot-p95` / `--max-slot-p99` (`--min-slot-samples`) est destiné à l'assistance aux cibles de durée de créneau NX-18 et à l'application des objectifs. کریں۔

Les validations avec espacement d'air (CI) pour le point de terminaison en direct sont prises en charge par la relecture de réponse Torii capturée:

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

`fixtures/nexus/lanes/` pour les appareils enregistrés, l'assistant d'amorçage pour les artefacts et les scripts. charpie کیا جا سکے۔ CI flux flux `ci/check_nexus_lane_smoke.sh` et `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) est un assistant de fumée NX-7 Le format de charge utile est disponible en plusieurs formats et les résumés/superpositions de paquets sont reproductibles.