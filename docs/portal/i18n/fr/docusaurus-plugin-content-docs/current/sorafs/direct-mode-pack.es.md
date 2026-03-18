---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pack en mode direct
titre : Paquet de contingence de mode direct de SoraFS (SNNet-5a)
sidebar_label : paquet de mode direct
description : Configuration requise, étapes d'exécution et étapes d'exécution pour utiliser SoraFS en mode direct Torii/QUIC pendant la transition SNNet-5a.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/direct_mode_pack.md`. Mantén est une copie synchronisée.
:::

Les circuits SoraNet doivent également assurer le transport prédéterminé pour SoraFS, mais l'élément de la feuille de route **SNNet-5a** nécessite un régulateur de repli pour que les opérateurs conservent un accès à la lecture déterministe pendant qu'ils complètent l'affichage anonymisé. Ce paquet reconnaît les boutons CLI / SDK, les profils de configuration, les tests de compilation et la liste de téléchargement nécessaire pour exécuter SoraFS en mode direct Torii/QUIC sans pour autant supprimer les transports de confidentialité.

La solution de secours appliquée à la mise en scène et aux processus de production est réglementée jusqu'à ce que SNNet-5 et SNNet-9 dépassent vos portes de préparation. Gardez les objets de dessous avec le matériel habituel de livraison de SoraFS pour que les opérateurs puissent alterner entre les modes anonymes et directs sous la demande.

## 1. Drapeaux de CLI et SDK- `sorafs_cli fetch --transport-policy=direct-only ...` désactive la programmation des relais et active les transports Torii/QUIC. L'aide de la CLI est maintenant listée `direct-only` comme valeur acceptée.
- Le SDK doit être établi `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` toujours en exposant une bascule de "mode direct". Les liaisons générées par `iroha::ClientOptions` et `iroha_android` renvoient la même énumération.
- Les harnais de passerelle (`sorafs_fetch`, liaisons de Python) peuvent interpréter le basculement direct uniquement au milieu des aides Norito JSON partagées pour que l'automatisation prenne le même comportement.

Documentez le drapeau dans les runbooks orientés vers les partenaires et canalisez les bascules vers le passage de `iroha_config` à la place des variables d'entrée.

## 2. Profils politiques de la passerelle

Utilisez JSON de Norito pour conserver une configuration déterminante de l'explorateur. Le profil de l'exemple dans le code `docs/examples/sorafs_direct_mode_policy.json` :

- `transport_policy: "direct_only"` — demandez des fournisseurs qui annoncent uniquement les transports du relais SoraNet.
- `max_providers: 2` — limite les pairs directs aux points de terminaison Torii/QUIC plus fiables. Ajustez les concessions de conformité régionales.
- `telemetry_region: "regulated-eu"` — étiquette des mesures émises pour que les tableaux de bord et les auditeurs distinguent les émissions de secours.
- Présupposés de réintégration des conservateurs (`retry_budget: 2`, `provider_failure_threshold: 3`) pour éviter de masquer les passerelles mal configurées.Chargez le JSON via `sorafs_cli fetch --config` (automatisation) ou les liaisons du SDK (`config_from_json`) avant d'exposer la politique aux opérateurs. Conservez la sortie du tableau de bord (`persist_path`) pour les trazas de auditoría.

Les boutons de mise en application du côté de la passerelle sont reconnus sur `docs/examples/sorafs_gateway_direct_mode.toml`. La plante reflète la sortie de `iroha app sorafs gateway direct-mode enable`, déshabilitant les transactions d'enveloppe/admission, câble les valeurs par défaut de limite de débit et publie le tableau `direct_mode` avec les noms d'hôte dérivés du plan et les résumés du manifeste. Remplacez les valeurs de marqueur de position par votre plan de déploiement avant de versionner le fragment dans la gestion de configuration.

## 3. Suite des essais de cumul

La préparation de mode direct aujourd'hui inclut une couverture aussi importante pour l'orquesteur que pour les caisses de CLI :- `direct_only_policy_rejects_soranet_only_providers` garantit que `TransportPolicy::DirectOnly` tombera rapidement lorsque chaque candidat annoncera seul un lien avec SoraNet.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` garantit que les transports Torii/QUIC sont utilisés lorsque vous les présentez et que les relais SoraNet sont exclus de la session.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` analyse `docs/examples/sorafs_direct_mode_policy.json` pour garantir que la documentation est alignée avec les assistants d'utilisation.
- `fetch_command_respects_direct_transports` ejercita `sorafs_cli fetch --transport-policy=direct-only` contre une passerelle Torii simulée, en fournissant une vérification de l'humidité pour les entreprises régulées qui transportent directement.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` inclut la même commande avec le JSON de politique et la persistance du tableau de bord pour l'automatisation du déploiement.

Exécutez la suite affichée avant de publier les mises à jour :

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Si la compilation de l'espace de travail a échoué par des changements en amont, enregistrez l'erreur bloquée sur `status.md` et vuelve à exécuter lorsque la dépendance est actuelle.

## 4. Éjections automatisées de fuméeLa couverture de CLI ne révèle que des incidences spécifiques de l'entreprise (par exemple, les dérives politiques de la passerelle ou les désajustements des manifestes). Un assistant de fumée dédié vivant en `scripts/sorafs_direct_mode_smoke.sh` et envuelve `sorafs_cli fetch` avec la politique de l'explorateur de mode direct, la persistance du tableau de bord et la capture des résultats.

Exemple d'utilisation :

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- Le script respecte les indicateurs de la CLI comme archives de configuration key=value (consulter `docs/examples/sorafs_direct_mode_smoke.conf`). Rellena le résumé du manifeste et les entrées des annonces du fournisseur avec les valeurs de production avant l'exécution.
- `--policy` par défaut est `docs/examples/sorafs_direct_mode_policy.json`, mais vous pouvez être responsable de tout JSON de l'explorateur produit par `sorafs_orchestrator::bindings::config_to_json`. La CLI accepte la politique via `--orchestrator-config=PATH`, permettant des émissions reproductibles sans ajuster les drapeaux à la main.
- Lorsque `sorafs_cli` n'est pas dans `PATH`, l'aide lo compile à partir de la caisse `sorafs_orchestrator` (version de profil) pour que les tests d'humo exécutent la plomberie de la manière directe qui est envoyée.
- Salidas :
  - Charge utile ensamblado (`--output`, par défaut `artifacts/sorafs_direct_mode/payload.bin`).
  - Résumé de récupération (`--summary`, par défaut avec la charge utile) qui contient la région de télémétrie et les rapports des fournisseurs utilisés comme preuves de déploiement.
  - Instantané du tableau de bord persistant dans la route déclarée dans le JSON de politique (par exemple, `fetch_state/direct_mode_scoreboard.json`). Archivage conjointement avec le CV et les tickets de changement.- Automatisation de la porte d'adoption : une fois que la récupération termine l'assistant en invoquant `cargo xtask sorafs-adoption-check` en utilisant les itinéraires persistants du tableau de bord et du résumé. Le quorum requis par défaut est le nombre de fournisseurs désignés en ligne de commande ; anúlalo con `--min-providers=<n>` cuando necesites a muestra mayor. Les informations d'adoption sont écrites conjointement avec le CV (`--adoption-report=<path>` peut fixer une localisation personnalisée) et l'aide passe `--require-direct-only` par défaut (coïncidant avec le repli) et `--require-telemetry` continue de maintenir le drapeau correspondant. Utilisez `XTASK_SORAFS_ADOPTION_FLAGS` pour réutiliser les arguments supplémentaires de xtask (par exemple `--allow-single-source` lors d'une rétrogradation approuvée pour que la porte tolère et puisse effectuer le repli). Omettez simplement la porte avec `--skip-adoption-check` pour exécuter des diagnostics locaux ; La feuille de route exige que chaque exécution soit réglementée de manière directe et qu'elle inclue le paquet d'informations d'adoption.

## 5. Liste de vérification des plis1. **Congélation de configuration :** gardez le profil JSON directement dans votre référentiel `iroha_config` et enregistrez le hachage dans votre ticket de changement.
2. **Audit de la passerelle :** confirmez que les points de terminaison Torii appliquent TLS, TLV de capacité et journalisation de l'audit avant de changer de mode direct. Publier le profil politique de la passerelle pour les opérateurs.
3. **Agrément de conformité :** partagez le playbook mis à jour avec les réviseurs de conformité/régulateurs et capturez les autorisations pour fonctionner hors de la superposition anonymisée.
4. **Dry run :** exécute la suite de compilation plus une récupération et une mise en scène contre les fournisseurs Torii de confiance. Archivage des sorties du tableau de bord et des résultats de la CLI.
5. **Corte en production:** annonce la fenêtre de changement, change de `transport_policy` à `direct_only` (si vous avez opté pour `soranet-first`) et surveillez les tableaux de bord de façon directe (latence de `sorafs_fetch`, contadores de fallos de fournisseurs). Documentez le plan de restauration pour retourner à SoraNet d'abord une fois que SNNet-4/5/5a/5b/6a/7/8/12/13 est passé à `roadmap.md:532`.
6. **Révision post-changement :** ajout d'instantanés du tableau de bord, résultats de récupération et résultats de surveillance du ticket de changement. Actualisez `status.md` avec la preuve efficace et toute anomalie.Gardez la liste de vérification avec le runbook `sorafs_node_ops` pour que les opérateurs puissent tester le flux avant un changement en vivo. Lorsque SNNet-5 arrive en GA, vous pouvez retirer le repli pour confirmer la parité dans la télémétrie de production.

## 6. Exigences de preuve et porte d'adoption

Les captures de mode direct doivent être effectuées avec la porte d'adoption SF-6c. Ajoutez le tableau de bord, le curriculum vitae, l'enveloppe du manifeste et les informations d'adoption à chaque exécution pour que `cargo xtask sorafs-adoption-check` puisse valider la position de repli. Les champs défaillants sont tombés sur la porte, ainsi que l'enregistrement des métadonnées attendues dans les tickets de changement.- **Métadonnées de transport :** `scoreboard.json` doit être déclaré `transport_policy="direct_only"` (et activer `transport_policy_override=true` lorsque vous forcez le déclassement). Gardez les champs politiques anonymisés emparejados y compris quand il s'agit de défauts pour que les réviseurs veuillent s'écarter du plan anonymisé par étapes.
- **Contadores des fournisseurs :** Les sessions solo-gateway doivent persister `provider_count=0` et afficher `gateway_provider_count=<n>` avec le numéro de fournisseurs Torii utilisé. Evitez de modifier le JSON à la main : le CLI/SDK dérive les contenus et la porte d'adoption reprend les captures qui ont omis la séparation.
- **Preuve du manifeste :** Lorsque les passerelles Torii participent, le `--gateway-manifest-envelope <path>` est confirmé (ou l'équivalent du SDK) pour que `gateway_manifest_provided` et le `gateway_manifest_id`/`gateway_manifest_cid` soient enregistrés `scoreboard.json`. Assurez-vous que `summary.json` lève le même `manifest_id`/`manifest_cid` ; la vérification de l'adoption échoue si les archives omite le par.
- **Attentes de télémétrie :** Lorsque la télémétrie accompagne la capture, exécutez la porte avec `--require-telemetry` pour que les informations vérifiées soient émises. Les ensayos en entornos aislados peuvent omettre le drapeau, mais CI et les billets de changement doivent documenter l'ausencia.

Exemple :

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```Adjoint `adoption_report.json` avec le tableau de bord, le résumé, l'enveloppe du manifeste et le paquet de bûches de fumée. Ces artefacts reflètent ceux qui appliquent le travail d'adoption en CI (`ci/check_sorafs_orchestrator_adoption.sh`) et maintiennent auditables les rétrogradations de mode direct.