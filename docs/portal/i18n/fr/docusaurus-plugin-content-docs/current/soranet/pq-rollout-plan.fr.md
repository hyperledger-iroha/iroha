---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-rollout-plan
titre : Playbook de déploiement post-quantique SNNet-16G
sidebar_label : Plan de déploiement PQ
description : Guide opérationnel pour promouvoir le handshake hybride X25519+ML-KEM de SoraNet de canary par défaut sur relais, clients et SDK.
---

:::note Source canonique
:::

SNNet-16G termine le déploiement post-quantique pour le transport SoraNet. Les boutons `rollout_phase` permettent aux opérateurs de coordonner une promotion déterministe du require guard Stage A vers la couverture majoritaire Stage B et la posture PQ stricte Stage C sans modifier du JSON/TOML brut pour chaque surface.

Ce couvre-chef du playbook :

- Définitions de phase et nouveaux boutons de configuration (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) câbles dans le codebase (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapping des flags SDK et CLI afin que chaque client puisse suivre le déploiement.
- Attentes de planification canary relais/client plus les tableaux de bord de gouvernance qui portail la promotion (`dashboards/grafana/soranet_pq_ratchet.json`).
- Hooks de rollback et références au runbook fire-drill ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Carte des phases

| `rollout_phase` | Etape d'anonymat effective | Effet par défaut | Utilisation typique |
|-----------------|---------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Étape A) | Exiger au moins un guard PQ par circuit pendant que la flotte se réchauffe. | Baseline et premières semaines de Canary. |
| `ramp` | `anon-majority-pq` (Étape B) | Favoriser la sélection vers des relais PQ pour >= deux niveaux de couverture; les relais classiques restent en repli. | Relais Canaries par région ; active le SDK d'aperçu. |
| `default` | `anon-strict-pq` (Étape C) | Imposer des circuits uniquement PQ et durcir les alarmes de downgrade. | Promotion finale une fois la télémétrie et la gouvernance de signature terminée. |

Si une surface définie également un `anonymity_policy` explicite, elle remplace la phase pour ce composant. Omettre l'étape explicite différée maintenant à la valeur `rollout_phase` afin que les opérateurs puissent basculer une fois par environnement et laisser les clients l'hériter.

## Référence de configuration

### Orchestrateur (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Le chargeur de l'orchestrator génère l'étape de repli en runtime (`crates/sorafs_orchestrator/src/lib.rs:2229`) et l'expose via `sorafs_orchestrator_policy_events_total` et `sorafs_orchestrator_pq_ratio_*`. Voir `docs/examples/sorafs_rollout_stage_b.toml` et `docs/examples/sorafs_rollout_stage_c.toml` pour des extraits prêts à appliquer.

### Client Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` enregistre maintenant la phase analysée (`crates/iroha/src/client.rs:2315`) afin que les commandes helper (par exemple `iroha_cli app sorafs fetch`) puissent reporter la phase actuelle aux cotes de la politique d'anonymat par défaut.

## Automatisation

Deux helpers `cargo xtask` automatisent la génération du planning et la capture d'artefacts.

1. **Générer le planning régional**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```Les durées acceptent les suffixes `s`, `m`, `h` ou `d`. La commande emet `artifacts/soranet_pq_rollout_plan.json` et un CV Markdown (`artifacts/soranet_pq_rollout_plan.md`) a joindre à la demande de changement.

2. **Capturer les artefacts du foret avec signatures**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   La commande copie les fichiers fournis dans `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calcule des digests BLAKE3 pour chaque artefact et écrit `rollout_capture.json` avec métadonnées plus une signature Ed25519 sur le payload. Utilisez la même clé privée que celle qui signe les minutes du fire-drill pour que la gouvernance puisse valider rapidement la capture.

## Matrice des drapeaux SDK & CLI

| Surfaces | Canari (stade A) | Rampe (étape B) | Par défaut (étape C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` récupérer | `--anonymity-policy stage-a` ou se reposer sur la phase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuration de l'orchestrateur JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuration du client Rust (`iroha.toml`) | `rollout_phase = "canary"` (par défaut) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Commandes signées `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, en option `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, en option `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, en option `.ANON_STRICT_PQ` |
| Aides de l'orchestrateur JavaScript | `rolloutPhase: "canary"` ou `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python`fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Rapide `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Tous les bascules SDK sont mappées au même analyseur de scène utilisé par l'orchestrator (`crates/sorafs_orchestrator/src/lib.rs:365`), de sorte que les déploiements multi-langages restent en lock-step avec la phase configurée.

## Liste de contrôle de planification Canary

1. **Vérification en amont (T moins 2 semaines)**

- Confirmer que le taux de baisse de tension Stage A est <1% sur les deux semaines précédentes et que la couverture PQ est >=70% par région (`sorafs_orchestrator_pq_candidate_ratio`).
   - Planifier le slot de gouvernance review qui approuve la fenêtre canari.
   - Mettre à jour `sorafs.gateway.rollout_phase = "ramp"` en staging (éditer le JSON orchestrator et redéployer) et dry-run le pipeline de promotion.

2. **Relais canari (jour T)**

   - Promouvoir une région à la fois en définissant `rollout_phase = "ramp"` sur l'orchestrator et les manifestes de relais participants.
   - Surveiller "Policy Events per Outcome" et "Brownout Rate" dans le tableau de bord PQ Ratchet (qui inclut maintenant le panel rollout) pendant deux fois le TTL du guard cache.
   - Capturer des snapshots `sorafs_cli guard-directory fetch` avant et après pour stockage d'audit.

3. **Canari client/SDK (T plus 1 semaine)**

   - Basculer `rollout_phase = "ramp"` dans les configs client ou passer des overrides `stage-b` pour les cohortes SDK désignées.
   - Capturer les diffs de télémétrie (`sorafs_orchestrator_policy_events_total` groupe par `client_id` et `region`) et les joindre au journal d'incident de déploiement.4. **Promotion par défaut (T plus 3 semaines)**

   - Une fois la gouvernance validée, basculer orchestrator et configs client vers `rollout_phase = "default"` et faire tourner la checklist de readiness signée dans les artefacts de release.

## Liste de contrôle pour la gouvernance et les preuves

| Changement de phase | Porte de promotion | Paquet de preuves | Tableaux de bord et alertes |
|--------------|----------------|-----------------|-----------|
| Canaries -> Rampe *(Aperçu de l'étape B)* | Taux de baisse de tension Stage A <1% sur les 14 derniers jours, `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 par région promue, ticket Argon2 vérifier p95 < 50 ms, et réserve de gouvernance des slots. | Paire JSON/Markdown `cargo xtask soranet-rollout-plan`, instantanés appaires `sorafs_cli guard-directory fetch` (avant/après), bundle signé `cargo xtask soranet-rollout-capture --label canary` et minutes canary référençant [PQ ratchet runbook](./pq-ratchet-runbook.md). | `dashboards/grafana/soranet_pq_ratchet.json` (Policy Events + Brownout Rate), `dashboards/grafana/soranet_privacy_metrics.json` (SN16 downgrade ratio), références télémétrie dans `docs/source/soranet/snnet16_telemetry_plan.md`. |
| Rampe -> Par défaut *(Application de l'étape C)* | Burn-in telemetrie SN16 de 30 jours atteint, `sn16_handshake_downgrade_total` plat au baseline, `sorafs_orchestrator_brownouts_total` a zero durant le client canary, et rehearsal du proxy toggle logge. | Transcription `sorafs_cli proxy set-mode --mode gateway|direct`, sortie `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, log `sorafs_cli guard-directory verify`, et bundle signe `cargo xtask soranet-rollout-capture --label default`. | Meme tableau PQ Ratchet plus les panneaux SN16 downgrade documentés dans `docs/source/sorafs_orchestrator_rollout.md` et `dashboards/grafana/soranet_privacy_metrics.json`. |
| Rétrogradation d'urgence/préparation à la restauration | Declenchez lorsque les compteurs downgrade montent, la vérification guard-directory echoue, ou le buffer `/policy/proxy-toggle` enregistre des downgrade events soutenus. | Checklist `docs/source/ops/soranet_transport_rollback.md`, logs `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets d'incident et modèles de notification. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` et les deux packs d'alertes (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Stockez chaque artefact sous `artifacts/soranet_pq_rollout/<timestamp>_<label>/` avec le `rollout_capture.json` générer afin que les paquets de gouvernance contiennent le scoreboard, les traces promtool et les digests.
- Attachez les résumés SHA256 des preuves chargées (minutes PDF, capture bundle, guard snapshots) aux minutes de promotion afin que les approbations Parlement puissent être rejouées sans accès au cluster staging.
- Referencez le plan de télémétrie dans le ticket de promotion pour prouver que `docs/source/soranet/snnet16_telemetry_plan.md` reste la source canonique des vocabulaires de downgrade et des seuils d'alertes.

## Mise à jour des tableaux de bord & télémétrie

`dashboards/grafana/soranet_pq_ratchet.json` inclut maintenant un panel d'annotation "Rollout Plan" qui renvoie vers ce playbook et expose la phase actuelle afin que les revues de gouvernance confirment quelle étape est active. Gardez la description du panneau synchronisé avec les futures évolutions des boutons de configuration.

Pour l'alerte, assurez-vous que les règles existent utilisent le label `stage` afin que les phases canaries et par défaut declenchent des seuils de politique separes (`dashboards/alerts/soranet_handshake_rules.yml`).

## Crochets de restauration

### Par défaut -> Rampe (Étape C -> Étape B)1. Retrogradez l'orchestrator avec `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (et faites miroiter la même phase sur les configs SDK) pour que Stage B reprenne sur toute la flotte.
2. Forcez les clients vers le profil de transport sur `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, en capturant la transcription afin que le workflow de remédiation `/policy/proxy-toggle` reste auditable.
3. Exécutez `cargo xtask soranet-rollout-capture --label rollback-default` pour archiver les diffs guard-directory, la sortie promtool et les captures d'écran des tableaux de bord sous `artifacts/soranet_pq_rollout/`.

### Rampe -> Canary (Étape B -> Étape A)

1. Importez le snapshot guard-directory capture avant promotion avec `sorafs_cli guard-directory import --guard-directory guards.json` et relancez `sorafs_cli guard-directory verify` afin que le paquet de rétrogradation inclue les hachages.
2. Définissez `rollout_phase = "canary"` (ou override avec `anonymity_policy stage-a`) sur l'orchestrator et les configs client, puis rejouez le PQ ratchet drill du [PQ ratchet runbook](./pq-ratchet-runbook.md) pour prouver le downgrade du pipeline.
3. Attachez les captures d'écran actualisées de la télémétrie PQ Ratchet et SN16 ainsi que les résultats d'alertes au journal d'incident avant la gouvernance des notifications.

### Garde-corps de rappel

- Referencez `docs/source/ops/soranet_transport_rollback.md` à chaque rétrogradation et enregistrez toute atténuation temporaire comme item `TODO:` dans le rollout tracker pour suivi.
- Gardez `dashboards/alerts/soranet_handshake_rules.yml` et `dashboards/alerts/soranet_privacy_rules.yml` sous couverture `promtool test rules` avant et après un rollback afin que toute dérive d'alertes soit documentée avec le bundle de capture.