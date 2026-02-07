---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pack en mode direct
titre : Pack de réponse en mode direct SoraFS (SNNet-5a)
sidebar_label : mode pack direct
description : Configuration requise, contrôles de conformité et étapes de déploiement lors de l'exploitation de SoraFS en mode direct Torii/QUIC pendant la transition SNNet-5a.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/direct_mode_pack.md`. Gardez les deux copies synchronisées jusqu'au retrait de l'ensemble Sphinx historique.
:::

Les circuits SoraNet restent le transport par défaut pour SoraFS, mais l'item de roadmap **SNNet-5a** exige une réponse régulée afin que les opérateurs puissent conserver un accès en lecture déterministe pendant que le déploiement de l'anonymat se termine. Ce pack rassemble les boutons CLI/SDK, les profils de configuration, les tests de conformité et la liste de contrôle de déploiement nécessaire pour exécuter SoraFS en mode direct Torii/QUIC sans toucher aux transports de confidentialité.

La réponse s'applique aux environnements de staging et de production régulés jusqu'à ce que SNNet-5 à SNNet-9 franchissent leurs portes de préparation. Conservez les artefacts ci-dessous avec le matériel de déploiement SoraFS habituel afin que les opérateurs puissent basculer entre les modes anonymes et directs à la demande.

## 1. Flags CLI et SDK- `sorafs_cli fetch --transport-policy=direct-only ...` désactive l'ordonnancement des relais et impose les transports Torii/QUIC. L'aide du CLI liste désormais `direct-only` comme valeur acceptée.
- Les SDK doivent définir `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` dès qu'ils exposent un basculement "mode direct". Les liaisons générées dans `iroha::ClientOptions` et `iroha_android` propagent le même enum.
- Les harnais de gateway (`sorafs_fetch`, liaisons Python) peuvent analyser le basculement directement uniquement via les helpers Norito JSON partagés pour que l'automatisation obtienne le même comportement.

Documentez le flag dans les runbooks orientés partenaires et faites passer les bascules via `iroha_config` plutôt que par des variables d'environnement.

## 2. Profils de politique du gateway

Utilisez le JSON Norito pour persister une configuration déterministe de l'orchestrateur. Le profil d'exemple dans `docs/examples/sorafs_direct_mode_policy.json` encode :

- `transport_policy: "direct_only"` — rejette les fournisseurs qui n'annoncent que des transports relais SoraNet.
- `max_providers: 2` — limite les paires directes aux extrémités Torii/QUIC les plus fiables. Ajustez selon les contraintes de conformité régionales.
- `telemetry_region: "regulated-eu"` — étiquette les métriques émises afin que les tableaux de bord et audits distinguent les exécutions de réponse.
- Budgets de nouvelle tentative conservateurs (`retry_budget: 2`, `provider_failure_threshold: 3`) pour éviter de masquer des passerelles mal configurées.Chargez le JSON via `sorafs_cli fetch --config` (automatisation) ou via les liaisons SDK (`config_from_json`) avant d'exposer la politique aux opérateurs. Persistez la sortie du scoreboard (`persist_path`) pour les pistes d'audit.

Les réglages d'application côté gateway sont capturés dans `docs/examples/sorafs_gateway_direct_mode.toml`. Le modèle reflète la sortie de `iroha app sorafs gateway direct-mode enable`, désactivant les chèques d'enveloppe/admission, câblant les valeurs par défaut de rate-limit, et remplissant la table `direct_mode` avec des noms d'hôtes dérivés du plan et des résumés de manifeste. Remplacez les valeurs d'espace réservé par votre plan de déploiement avant de versionner l'extrait dans la gestion de configuration.

## 3. Suite de tests de conformité

La disponibilité du mode direct inclut désormais une couverture dans l'orchestrateur et dans les caisses CLI :- `direct_only_policy_rejects_soranet_only_providers` garantit que `TransportPolicy::DirectOnly` échoue rapidement lorsque chaque annonce candidate ne prend en charge que les relais SoraNet.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` garantit que les transports Torii/QUIC sont utilisés lorsqu'ils sont disponibles et que les relais SoraNet sont exclus de la session.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` analyse `docs/examples/sorafs_direct_mode_policy.json` pour s'assurer que la documentation reste alignée avec les helpers utilitaires.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` exerce `sorafs_cli fetch --transport-policy=direct-only` contre une passerelle Torii simulé, fournissant un test de fumée pour les environnements régulés qui épinglent les transports directs.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` enveloppe la même commande avec le JSON de politique et la persistance du scoreboard pour l'automatisation du déploiement.

Exécutez la suite ciblée avant de publier des mises à jour :

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Si la compilation du workspace échoue à cause de changements en amont, consignez l'erreur bloquée dans `status.md` et réexécutez une fois la dépendance mise à jour.

## 4. Exécutions de fumée automatiséesLa couverture CLI seule ne révèle pas les régressions spécifiques à l'environnement (par exemple dérive des politiques passerelles ou inadéquations de manifeste). Un helper de smoke dédié vit dans `scripts/sorafs_direct_mode_smoke.sh` et enveloppe `sorafs_cli fetch` avec la politique d'orchestrateur en mode direct, la persistance du scoreboard et la capture du CV.

Exemple d'utilisation :

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- Le script respecte à la fois les flags CLI et les fichiers de configuration key=value (voir `docs/examples/sorafs_direct_mode_smoke.conf`). Renseignez le digest du manifeste et les entrées d'annonces de fournisseurs avec des valeurs de production avant l'exécution.
- `--policy` pointe par défaut vers `docs/examples/sorafs_direct_mode_policy.json`, mais tout JSON d'orchestrateur produit par `sorafs_orchestrator::bindings::config_to_json` peut être fourni. La CLI accepte la politique via `--orchestrator-config=PATH`, permettant des exécutions reproductibles sans ajuster les drapeaux à la main.
- Quand `sorafs_cli` n'est pas dans `PATH`, le helper le construit depuis le crate `sorafs_orchestrator` (profil release) afin que les fumées exercent la plomberie du mode direct livré.
- Sorties :
  - Charge utile assemblée (`--output`, par défaut `artifacts/sorafs_direct_mode/payload.bin`).
  - Résumé de fetch (`--summary`, par défaut à côté du payload) contenant la région de télémétrie et les rapports de fournisseurs utilisés pour l'évidence de déploiement.
  - Snapshot de scoreboard persisté vers le chemin déclaré dans le JSON de politique (par exemple `fetch_state/direct_mode_scoreboard.json`). Archivez-le avec le curriculum vitae dans les tickets de changement.- Automatisation du gate d'adoption : une fois le fetch terminé, le helper invoque `cargo xtask sorafs-adoption-check` en utilisant les chemins persistés de scoreboard et de summary. Le quorum requis par défaut correspond au nombre de prestataires fournis sur la ligne de commande ; Remplacez-le par `--min-providers=<n>` lorsque vous avez besoin d'un échantillon plus large. Les rapports d'adoption sont écrits à côté du curriculum vitae (`--adoption-report=<path>` peut définir un emplacement personnalisé) et le helper passe `--require-direct-only` par défaut (aligné sur le repli) et `--require-telemetry` lorsque vous fournissez le drapeau correspondant. Utilisez `XTASK_SORAFS_ADOPTION_FLAGS` pour transmettre des arguments xtask supplémentaires (par exemple `--allow-single-source` pendant un downgrade approuvé afin que le gate tolère et impose le repli). Ne sautez le portail d'adoption avec `--skip-adoption-check` que lors de diagnostics locaux ; la feuille de route exige que chaque exécution régulée en mode direct comprenne le bundle de rapport d'adoption.

## 5. Checklist de déploiement1. **Gel de configuration :** stockez le profil JSON du mode direct dans votre dépôt `iroha_config` et consignez le hash dans votre ticket de changement.
2. **Audit gateway :** confirmez que les endpoints Torii appliquent TLS, les TLV de capacité et la journalisation d'audit avant de basculer en mode direct. Publiez le profil de politique gateway pour les opérateurs.
3. **Validation conformité :** partagez le playbook mis à jour avec les rélecteurs conformité / réglementaires et capturez les approbations pour opérer en dehors de l'overlay d'anonymat.
4. **Dry run :** exécutez la suite de conformité plus un fetch de staging contre des fournisseurs Torii de confiance. Archivez les sorties du scoreboard et les CV CLI.
5. **Bascule production :** annoncez la fenêtre de changement, passez `transport_policy` à `direct_only` (si vous avez opté pour `soranet-first`) et surveillez les tableaux de bord du mode direct (latence `sorafs_fetch`, compteurs d'échec des fournisseurs). Documentez le plan de rollback pour revenir à SoraNet-first une fois que SNNet-4/5/5a/5b/6a/7/8/12/13 est passé dans `roadmap.md:532`.
6. **Revue post-changement :** attachez les instantanés de scoreboard, les résumés de fetch et les résultats de monitoring au ticket de changement. Mettez à jour `status.md` avec la date effective et toute anomalie.Gardez la checklist à côté du runbook `sorafs_node_ops` afin que les opérateurs puissent répéter le flux avant un basculement réel. Lorsque SNNet-5 passera GA, retirez le réponse après avoir confirmé la parité dans la télémétrie de production.

## 6. Exigences de preuves et porte d'adoption

Les captures en mode direct doivent toujours satisfaire la porte d'adoption SF-6c. Regroupez le tableau de bord, le résumé, l'enveloppe de manifeste et le rapport d'adoption pour chaque course afin que `cargo xtask sorafs-adoption-check` puisse valider la posture de réponse. Les champs manquants font échouer le portail, consignez donc les métadonnées attendues dans les tickets de changement.- **Métadonnées de transport :** `scoreboard.json` doit déclarer `transport_policy="direct_only"` (et basculer `transport_policy_override=true` lorsque vous avez forcé le downgrade). Gardez les champs de politique d'anonymat associés remplis même lorsqu'ils héritent des valeurs par défaut afin que les rélecteurs constatent si vous avez dévié du plan d'anonymat par étapes.
- **Compteurs de fournisseurs :** Les sessions gateway-only doivent persister `provider_count=0` et remplir `gateway_provider_count=<n>` avec le nombre de fournisseurs Torii utilisés. Évitez d'éditer le JSON à la main : le CLI/SDK dérive déjà les compteurs et la porte d'adoption rejette les captures qui omettent la séparation.
- **Preuve de manifest :** Quand des gateways Torii participent, passez le `--gateway-manifest-envelope <path>` signé (ou l'équivalent SDK) afin que `gateway_manifest_provided` et les `gateway_manifest_id`/`gateway_manifest_cid` soient enregistrés dans `scoreboard.json`. Assurez-vous que `summary.json` porte le même `manifest_id`/`manifest_cid` ; la vérification d'adoption a échoué si l'un des fichiers omet la paire.
- **Attentes de télémétrie :** Lorsque la télémétrie accompagne la capture, exécutez le gate avec `--require-telemetry` afin que le rapport d'adoption prouve que les métriques ont été émises. Les répétitions en air-gapped peuvent omettre le drapeau, mais le CI et les tickets de changement doivent documenter l'absence.

Exemple :

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```Attachez `adoption_report.json` avec le tableau de bord, le résumé, l'enveloppe de manifeste et le bundle de bûches de fumée. Ces artefacts renvoient ce que le travail d'adoption CI (`ci/check_sorafs_orchestrator_adoption.sh`) impose et rend les downgrades en mode auditables directs.