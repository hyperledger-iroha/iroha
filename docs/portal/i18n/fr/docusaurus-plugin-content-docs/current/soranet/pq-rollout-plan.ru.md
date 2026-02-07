---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-rollout-plan
titre : Déploiement du poste de travail SNNet-16G
sidebar_label : plan de déploiement PQ
description : Fonctionnement pour la production du hybride X25519+ML-KEM handshake SoraNet de Canary par défaut pour les relais, les clients et les SDK.
---

:::note Канонический источник
:::

SNNet-16G permet le déploiement post-quantique pour le transport SoraNet. Les boutons `rollout_phase` permettent à l'opérateur de coordonner la promotion de la détection des exigences de garde de l'étape A par rapport à la couverture majoritaire de l'étape B et à la posture PQ stricte de l'étape C sans corriger l'utilisation de JSON/TOML pour tous les aspects de la sécurité.

Ce livre de jeu comprend :

- Phase de préparation et nouveaux boutons de configuration (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) dans la base de code (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Le mappage des indicateurs SDK et CLI, pour que le client puisse faciliter le déploiement.
- Gestion de la planification Canary pour les tableaux de bord relais/clients et de gouvernance, pour la promotion de la porte d'entrée (`dashboards/grafana/soranet_pq_ratchet.json`).
- Crochets de restauration et outils pour le runbook d'exercice d'incendie ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Carte du moment

| `rollout_phase` | Étape d'anonymat efficace | Effet sur l'amélioration | Utilisation typique |
|-----------------|---------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Étape A) | Требовать как один PQ guard на circuit, пока флот прогревается. | Baseline и ранние недели canari. |
| `ramp` | `anon-majority-pq` (Étape B) | Placez-vous dans le magasin Relais PQ pour >= double couverture ; Les relais classiques sont en repli. | Relais régional des canaris ; L’aperçu du SDK bascule. |
| `default` | `anon-strict-pq` (Étape C) | Activez les circuits PQ uniquement et utilisez les alarmes de déclassement. | Promotion finale après la télémétrie et la gouvernance d'approbation. |

Si vous utilisez le `anonymity_policy`, vous devez effectuer la phase préalable à ce composant. La phase actuelle doit être différée avec l'option `rollout_phase`, les opérateurs peuvent activer la phase pour passer à l'heure actuelle et communiquer avec les clients. унаследовать его.

## Configuration de référence

### Orchestrateur (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

L'orchestrateur du chargeur propose une étape de secours lors de la mise en service (`crates/sorafs_orchestrator/src/lib.rs:2229`) et l'utilise pour `sorafs_orchestrator_policy_events_total` et `sorafs_orchestrator_pq_ratio_*`. См. `docs/examples/sorafs_rollout_stage_b.toml` et `docs/examples/sorafs_rollout_stage_c.toml` pour les extraits de code.

### Client Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` теперь сохраняет разобранную phase (`crates/iroha/src/client.rs:2315`), чтобы helper команды (например `iroha_cli app sorafs fetch`) могли сообщать текущую phase Il s'agit de la politique d'anonymat par défaut.

## Automatisation

L'assistant `cargo xtask` génère automatiquement la répartition et la capture d'artefacts.

1. **Générer le calendrier régional**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   Les durées correspondent aux suffixes `s`, `m`, `h` ou `d`. La commande envoie `artifacts/soranet_pq_rollout_plan.json` et le résumé Markdown (`artifacts/soranet_pq_rollout_plan.md`), qui peut être appliqué à une demande de modification.

2. **Capturer les artefacts de forage avec les services**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```La commande copie les fichiers dans `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, en utilisant les résumés BLAKE3 pour cet artefact et en utilisant les métadonnées `rollout_capture.json` et Ed25519. подписью поверх charge utile. Utilisez cette clé privée pour créer des minutes d'exercices d'incendie, ce qui permet à la gouvernance de valider la capture.

## Drapeaux de matrice SDK et CLI

| Surfaces | Canari (stade A) | Rampe (étape B) | Par défaut (étape C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` récupérer | `--anonymity-policy stage-a` ou fonctionnement de la phase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuration de l'orchestrateur JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuration du client Rust (`iroha.toml`) | `rollout_phase = "canary"` (par défaut) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Commandes signées `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, en option `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, en option `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, en option `.ANON_STRICT_PQ` |
| Aides de l'orchestrateur JavaScript | `rolloutPhase: "canary"` ou `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python`fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Rapide `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Le SDK bascule vers l'analyseur de scène, à savoir l'orchestrateur (`crates/sorafs_orchestrator/src/lib.rs:365`), lorsque les déploiements multilingues s'effectuent en étape de verrouillage lors de la phase initiale.

## Liste de contrôle de planification Canary

1. **Vérification en amont (T moins 2 semaines)**

- Veuillez noter que le taux de baisse de tension de l'étape A <1 % pour les deux dernières années et la couverture PQ >=70 % dans la région (`sorafs_orchestrator_pq_candidate_ratio`).
   - Planifiez le créneau d'examen de la gouvernance, qui sera ouvert à Canary.
   - Installez `sorafs.gateway.rollout_phase = "ramp"` dans la mise en scène (activez l'orchestrateur JSON et redéployez) et activez le pipeline de promotion à sec.

2. **Relais canari (jour T)**

   - Produisez dans votre région, placez `rollout_phase = "ramp"` sur l'orchestrateur et manifestez vos relais.
   - Sélectionnez "Événements de stratégie par résultat" et "Taux de baisse de tension" sur le tableau de bord PQ Ratchet (avec panneau de déploiement) dans la technologie du cache de garde TTL.
   - Enregistrez les instantanés `sorafs_cli guard-directory fetch` avant et après le stockage d'audit.

3. **Canari client/SDK (T plus 1 semaine)**

   - Sélectionnez `rollout_phase = "ramp"` dans les configurations client ou effectuez les remplacements `stage-b` pour certaines cohortes du SDK.
   - Enregistrez les différences de télémétrie (`sorafs_orchestrator_policy_events_total`, en utilisant `client_id` et `region`) et créez le journal des incidents de déploiement.

4. **Promotion par défaut (T plus 3 semaines)**

   - Après l'approbation de la gouvernance, vérifiez les configurations de l'orchestrateur et du client sur `rollout_phase = "default"` et lancez la liste de contrôle de préparation pour la publication des artefacts.

## Liste de contrôle sur la gouvernance et les preuves| Changement de phase | Porte de promotion | Paquet de preuves | Tableaux de bord et alertes |
|--------------|----------------|-----------------|-----------|
| Canaries -> Rampe *(Aperçu de l'étape B)* | Taux de baisse de tension de l'étape A <1 % depuis 14 jours, `sorafs_orchestrator_pq_candidate_ratio` >= 0,7 pour la promotion régionale, vérification du ticket Argon2 p95 < 50 ms et la promotion du module de gouvernance des emplacements est effectuée. | Pour JSON/Markdown à partir de `cargo xtask soranet-rollout-plan`, vous pouvez ajouter des instantanés `sorafs_cli guard-directory fetch` (avant/après), ajouter le bundle `cargo xtask soranet-rollout-capture --label canary` et des minutes Canary avec le [PQ ratchet runbook](./pq-ratchet-runbook.md). | `dashboards/grafana/soranet_pq_ratchet.json` (événements de politique + taux de baisse de tension), `dashboards/grafana/soranet_privacy_metrics.json` (taux de déclassement SN16), références de télémétrie dans `docs/source/soranet/snnet16_telemetry_plan.md`. |
| Rampe -> Par défaut *(Application de l'étape C)* | Le rodage de télémétrie SN16 de 30 jours a été effectué, `sn16_handshake_downgrade_total` installé sur la ligne de base, `sorafs_orchestrator_brownouts_total` seulement dans le client Canary actuel et l'option de répétition de bascule proxy. | Transcription `sorafs_cli proxy set-mode --mode gateway|direct`, pour `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, journal `sorafs_cli guard-directory verify`, bundle `cargo xtask soranet-rollout-capture --label default`. | Il s'agit de la carte à cliquet PQ et des panneaux de déclassement SN16, décrits dans `docs/source/sorafs_orchestrator_rollout.md` et `dashboards/grafana/soranet_privacy_metrics.json`. |
| Rétrogradation d'urgence/préparation à la restauration | Déclenche l'utilisation de compteurs de rétrogradation, vérifie la vérification du répertoire de garde ou utilise les événements de rétrogradation dans le tampon `/policy/proxy-toggle`. | Liste de contrôle pour `docs/source/ops/soranet_transport_rollback.md`, journaux `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets d'incident et modèles de notification. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` et des packs d'alertes (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Créez un artefact dans `artifacts/soranet_pq_rollout/<timestamp>_<label>/` avec le générateur `rollout_capture.json`, qui contient des paquets de gouvernance inclus dans le tableau de bord, les traces et les résumés de Promtool.
- Utilisez les résumés SHA256 de vos documents (minutes PDF, paquet de capture, instantanés de garde) et les minutes de promotion, les approbations du Parlement peuvent être obtenues sans la création d'un cluster de préparation.
- Recherchez le plan de télémétrie dans le ticket promotionnel, afin de mettre à jour le `docs/source/soranet/snnet16_telemetry_plan.md`, qui permet d'établir une installation canonique pour déclasser les vocabulaires et les seuils d'alerte.

## Mises à jour du tableau de bord et de la télémétrie

`dashboards/grafana/soranet_pq_ratchet.json` vous permet d'utiliser le panneau d'annotation « Plan de déploiement », qui est conçu pour ce livre de jeu et la phase de travail, pendant laquelle les examens de gouvernance peuvent mettre à jour l'étape d'activité. Держите описание le panneau est synchronisé avec les boutons de réglage.

Pour alerter, les règles de surveillance utilisent l'étiquette `stage`, les phases Canary et par défaut déclenchant des seuils de politique supplémentaires (`dashboards/alerts/soranet_handshake_rules.yml`).

## Crochets de restauration

### Par défaut -> Rampe (Étape C -> Étape B)

1. Rétrograder l'orchestrateur à partir de `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (et remplacer cette phase dans les configurations du SDK), afin que l'étape B soit installée sur votre ordinateur.
2. Enregistrez les clients dans le profil de transport gratuit à partir de `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, en utilisant la transcription pour le flux de travail d'auditabilité `/policy/proxy-toggle`.
3. Téléchargez `cargo xtask soranet-rollout-capture --label rollback-default` pour l'archivage des différences du répertoire de garde, la sortie de Promtool et les captures d'écran du tableau de bord sous `artifacts/soranet_pq_rollout/`.

### Rampe -> Canary (Étape B -> Étape A)1. Importez l'instantané du répertoire de garde, avant la promotion, à partir de `sorafs_cli guard-directory import --guard-directory guards.json` et puis installez `sorafs_cli guard-directory verify`, pour que le paquet de rétrogradation inclut les hachages.
2. Installez `rollout_phase = "canary"` (ou remplacez `anonymity_policy stage-a`) dans les configurations de l'orchestrateur et du client, puis activez la perceuse à cliquet PQ dans [PQ Ratchet Runbook] (./pq-ratchet-runbook.md), afin de télécharger le pipeline de rétrogradation.
3. Utilisez les captures d'écran de télémétrie PQ Ratchet et SN16 ainsi que les résultats des alertes et le journal des incidents avant la gouvernance.

### Rappels de garde-corps

- Recherchez `docs/source/ops/soranet_transport_rollback.md` lors de la rétrogradation et fixez les mesures d'atténuation telles que `TODO:` dans le suivi de déploiement pour les travaux ultérieurs.
- Sélectionnez `dashboards/alerts/soranet_handshake_rules.yml` et `dashboards/alerts/soranet_privacy_rules.yml` lorsque vous activez `promtool test rules` après une restauration, pour que la dérive d'alerte soit ajoutée au package de capture.