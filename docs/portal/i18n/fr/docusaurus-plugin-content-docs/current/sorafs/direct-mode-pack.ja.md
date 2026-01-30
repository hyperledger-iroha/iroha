---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sorafs/direct-mode-pack.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d4785c22d9925ba62f64d94edb90eb888f5e66d9d6243d3e4e066a6fe8e9c6a2
source_last_modified: "2025-11-14T04:43:21.698078+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/direct_mode_pack.md`. Gardez les deux copies synchronisées jusqu'au retrait de l'ensemble Sphinx historique.
:::

Les circuits SoraNet restent le transport par défaut pour SoraFS, mais l'item de roadmap **SNNet-5a** exige un repli régulé afin que les opérateurs puissent conserver un accès en lecture déterministe pendant que le déploiement de l'anonymat se termine. Ce pack rassemble les boutons CLI/SDK, les profils de configuration, les tests de conformité et la checklist de déploiement nécessaires pour exécuter SoraFS en mode direct Torii/QUIC sans toucher aux transports de confidentialité.

Le repli s'applique aux environnements de staging et de production régulée jusqu'à ce que SNNet-5 à SNNet-9 franchissent leurs gates de readiness. Conservez les artefacts ci-dessous avec le matériel de déploiement SoraFS habituel afin que les opérateurs puissent basculer entre les modes anonyme et direct à la demande.

## 1. Flags CLI et SDK

- `sorafs_cli fetch --transport-policy=direct-only ...` désactive l'ordonnancement des relais et impose les transports Torii/QUIC. L'aide du CLI liste désormais `direct-only` comme valeur acceptée.
- Les SDK doivent définir `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` dès qu'ils exposent un toggle "mode direct". Les bindings générés dans `iroha::ClientOptions` et `iroha_android` propagent le même enum.
- Les harnesses de gateway (`sorafs_fetch`, bindings Python) peuvent parser le toggle direct-only via les helpers Norito JSON partagés pour que l'automatisation obtienne le même comportement.

Documentez le flag dans les runbooks orientés partenaires et faites passer les toggles via `iroha_config` plutôt que par des variables d'environnement.

## 2. Profils de politique du gateway

Utilisez du JSON Norito pour persister une configuration déterministe de l'orchestrateur. Le profil d'exemple dans `docs/examples/sorafs_direct_mode_policy.json` encode :

- `transport_policy: "direct_only"` — rejette les providers qui n'annoncent que des transports relais SoraNet.
- `max_providers: 2` — limite les pairs directs aux endpoints Torii/QUIC les plus fiables. Ajustez selon les contraintes de conformité régionales.
- `telemetry_region: "regulated-eu"` — étiquette les métriques émises afin que dashboards et audits distinguent les runs de repli.
- Budgets de retry conservateurs (`retry_budget: 2`, `provider_failure_threshold: 3`) pour éviter de masquer des gateways mal configurés.

Chargez le JSON via `sorafs_cli fetch --config` (automatisation) ou via les bindings SDK (`config_from_json`) avant d'exposer la politique aux opérateurs. Persistez la sortie du scoreboard (`persist_path`) pour les pistes d'audit.

Les réglages d'application côté gateway sont capturés dans `docs/examples/sorafs_gateway_direct_mode.toml`. Le modèle reflète la sortie de `iroha app sorafs gateway direct-mode enable`, désactivant les checks d'envelope/admission, câblant les valeurs par défaut de rate-limit, et remplissant la table `direct_mode` avec des hostnames dérivés du plan et des digests de manifest. Remplacez les valeurs d'espace réservé avec votre plan de rollout avant de versionner l'extrait dans la gestion de configuration.

## 3. Suite de tests de conformité

La readiness du mode direct inclut désormais une couverture dans l'orchestrateur et dans les crates CLI :

- `direct_only_policy_rejects_soranet_only_providers` garantit que `TransportPolicy::DirectOnly` échoue rapidement lorsque chaque advert candidat ne prend en charge que les relais SoraNet.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` garantit que les transports Torii/QUIC sont utilisés lorsqu'ils sont disponibles et que les relais SoraNet sont exclus de la session.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` parse `docs/examples/sorafs_direct_mode_policy.json` pour s'assurer que la documentation reste alignée avec les helpers utilitaires.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` exerce `sorafs_cli fetch --transport-policy=direct-only` contre un gateway Torii simulé, fournissant un smoke test pour les environnements régulés qui épinglent les transports directs.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` enveloppe la même commande avec le JSON de politique et la persistance du scoreboard pour l'automatisation du rollout.

Exécutez la suite ciblée avant de publier des mises à jour :

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Si la compilation du workspace échoue à cause de changements upstream, consignez l'erreur bloquante dans `status.md` et réexécutez une fois la dépendance mise à jour.

## 4. Exécutions de smoke automatisées

La couverture CLI seule ne révèle pas les régressions spécifiques à l'environnement (par exemple dérive des politiques gateway ou inadéquations de manifest). Un helper de smoke dédié vit dans `scripts/sorafs_direct_mode_smoke.sh` et enveloppe `sorafs_cli fetch` avec la politique d'orchestrateur en mode direct, la persistance du scoreboard et la capture du résumé.

Exemple d'utilisation :

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- Le script respecte à la fois les flags CLI et les fichiers de configuration key=value (voir `docs/examples/sorafs_direct_mode_smoke.conf`). Renseignez le digest du manifest et les entrées d'adverts de providers avec des valeurs de production avant l'exécution.
- `--policy` pointe par défaut vers `docs/examples/sorafs_direct_mode_policy.json`, mais tout JSON d'orchestrateur produit par `sorafs_orchestrator::bindings::config_to_json` peut être fourni. Le CLI accepte la politique via `--orchestrator-config=PATH`, permettant des runs reproductibles sans ajuster les flags à la main.
- Quand `sorafs_cli` n'est pas dans `PATH`, le helper le construit depuis le crate `sorafs_orchestrator` (profil release) afin que les smokes exercent le plumbing du mode direct livré.
- Sorties :
  - Payload assemblé (`--output`, par défaut `artifacts/sorafs_direct_mode/payload.bin`).
  - Résumé de fetch (`--summary`, par défaut à côté du payload) contenant la région de télémétrie et les rapports de providers utilisés pour l'évidence de rollout.
  - Snapshot de scoreboard persisté vers le chemin déclaré dans le JSON de politique (par exemple `fetch_state/direct_mode_scoreboard.json`). Archivez-le avec le résumé dans les tickets de changement.
- Automatisation du gate d'adoption : une fois le fetch terminé, le helper invoque `cargo xtask sorafs-adoption-check` en utilisant les chemins persistés de scoreboard et de summary. Le quorum requis par défaut correspond au nombre de providers fournis sur la ligne de commande ; remplacez-le avec `--min-providers=<n>` quand vous avez besoin d'un échantillon plus large. Les rapports d'adoption sont écrits à côté du résumé (`--adoption-report=<path>` peut définir un emplacement personnalisé) et le helper passe `--require-direct-only` par défaut (aligné sur le repli) et `--require-telemetry` quand vous fournissez le flag correspondant. Utilisez `XTASK_SORAFS_ADOPTION_FLAGS` pour transmettre des arguments xtask supplémentaires (par exemple `--allow-single-source` pendant un downgrade approuvé afin que le gate tolère et impose le repli). Ne sautez le gate d'adoption avec `--skip-adoption-check` que lors de diagnostics locaux ; la roadmap exige que chaque run régulé en mode direct inclue le bundle de rapport d'adoption.

## 5. Checklist de déploiement

1. **Gel de configuration :** stockez le profil JSON du mode direct dans votre dépôt `iroha_config` et consignez le hash dans votre ticket de changement.
2. **Audit gateway :** confirmez que les endpoints Torii appliquent TLS, les TLV de capacité et la journalisation d'audit avant de basculer en mode direct. Publiez le profil de politique gateway pour les opérateurs.
3. **Validation conformité :** partagez le playbook mis à jour avec les relecteurs conformité / réglementaires et capturez les approbations pour opérer en dehors de l'overlay d'anonymat.
4. **Dry run :** exécutez la suite de conformité plus un fetch de staging contre des providers Torii de confiance. Archivez les sorties du scoreboard et les résumés CLI.
5. **Bascule production :** annoncez la fenêtre de changement, passez `transport_policy` à `direct_only` (si vous aviez opté pour `soranet-first`) et surveillez les dashboards du mode direct (latence `sorafs_fetch`, compteurs d'échec des providers). Documentez le plan de rollback pour revenir à SoraNet-first une fois que SNNet-4/5/5a/5b/6a/7/8/12/13 sont passés dans `roadmap.md:532`.
6. **Revue post-changement :** attachez les snapshots de scoreboard, les résumés de fetch et les résultats de monitoring au ticket de changement. Mettez à jour `status.md` avec la date effective et toute anomalie.

Gardez la checklist à côté du runbook `sorafs_node_ops` afin que les opérateurs puissent répéter le flux avant un basculement réel. Quand SNNet-5 passera GA, retirez le repli après avoir confirmé la parité dans la télémétrie de production.

## 6. Exigences de preuves et gate d'adoption

Les captures en mode direct doivent toujours satisfaire le gate d'adoption SF-6c. Regroupez le scoreboard, le summary, l'envelope de manifest et le rapport d'adoption pour chaque run afin que `cargo xtask sorafs-adoption-check` puisse valider la posture de repli. Les champs manquants font échouer le gate, consignez donc les métadonnées attendues dans les tickets de changement.

- **Métadonnées de transport :** `scoreboard.json` doit déclarer `transport_policy="direct_only"` (et basculer `transport_policy_override=true` lorsque vous avez forcé le downgrade). Gardez les champs de politique d'anonymat associés remplis même lorsqu'ils héritent des valeurs par défaut afin que les relecteurs voient si vous avez dévié du plan d'anonymat par étapes.
- **Compteurs de providers :** Les sessions gateway-only doivent persister `provider_count=0` et remplir `gateway_provider_count=<n>` avec le nombre de providers Torii utilisés. Évitez d'éditer le JSON à la main : le CLI/SDK dérive déjà les compteurs et le gate d'adoption rejette les captures qui omettent la séparation.
- **Preuve de manifest :** Quand des gateways Torii participent, passez le `--gateway-manifest-envelope <path>` signé (ou l'équivalent SDK) afin que `gateway_manifest_provided` et les `gateway_manifest_id`/`gateway_manifest_cid` soient enregistrés dans `scoreboard.json`. Assurez-vous que `summary.json` porte le même `manifest_id`/`manifest_cid` ; la vérification d'adoption échoue si l'un des fichiers omet la paire.
- **Attentes de télémétrie :** Quand la télémétrie accompagne la capture, exécutez le gate avec `--require-telemetry` afin que le rapport d'adoption prouve que les métriques ont été émises. Les répétitions en air-gapped peuvent omettre le flag, mais le CI et les tickets de changement doivent documenter l'absence.

Exemple :

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

Attachez `adoption_report.json` avec le scoreboard, le summary, l'envelope de manifest et le bundle de logs de smoke. Ces artefacts reflètent ce que le job d'adoption CI (`ci/check_sorafs_orchestrator_adoption.sh`) impose et rendent les downgrades en mode direct auditables.
