---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-rollout-plan
titre : Manuel de déploiement post-quantique SNNet-16G
sidebar_label : Plan de déploiement PQ
description : SoraNet et hybride X25519+ML-KEM handshake et Canary et relais par défaut, et les clients et les SDK favorisent la promotion des services de sécurité.
---

:::note Source canonique
یہ صفحہ `docs/source/soranet/pq_rollout_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانا documentation set retiré نہ ہو، دونوں کاپیاں رکھیں۔
:::

Transport SNNet-16G SoraNet pour le déploiement post-quantique `rollout_phase` opérateurs de boutons pour les coordonnées de promotion déterministes pour les exigences de garde de l'étape A et la couverture majoritaire de l'étape B et la posture PQ stricte de l'étape C pour la surface JSON/TOML brut et modification

Le playbook contient la couverture de la couverture :

- Définitions de phases et boutons de configuration (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) et base de code filaire (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mappage des indicateurs SDK et CLI pour le déploiement et le suivi du client
- Attentes de planification des relais/clients Canary et tableaux de bord de gouvernance et promotion et porte d'entrée (`dashboards/grafana/soranet_pq_ratchet.json`).
- Crochets de restauration et références du runbook d'exercices d'incendie et des références ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Carte des phases

| `rollout_phase` | Étape d'anonymat efficace | Effet par défaut | Utilisation typique |
|-----------------|---------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Étape A) | فلیٹ کے گرم ہونے تک ہر circuit کے لئے کم از کم ایک PQ guard لازم کریں۔ | Baseline اور ابتدائی canari ہفتے۔ |
| `ramp` | `anon-majority-pq` (Étape B) | Les relais PQ et la polarisation sont disponibles >= pour la couverture et la couverture relais classiques de secours | Relais canaris région par région؛ Bascule de l'aperçu du SDK۔ |
| `default` | `anon-strict-pq` (Étape C) | Les circuits PQ uniquement appliquent des alarmes de déclassement et de déclassement. | Télémétrie et approbation de la gouvernance pour la promotion |

Surface explicite `anonymity_policy` pour définir un composant et un composant pour la phase et le remplacement du composant Étape explicite pour la valeur `rollout_phase` pour différer les opérateurs et l'environnement pour le retournement de phase et les clients pour hériter

## Référence de configuration

### Orchestrateur (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

L'exécution du chargeur Orchestrator et la résolution de l'étape de repli sont disponibles (`crates/sorafs_orchestrator/src/lib.rs:2229`) et `sorafs_orchestrator_policy_events_total` et `sorafs_orchestrator_pq_ratio_*` sont disponibles en surface. `docs/examples/sorafs_rollout_stage_b.toml` et `docs/examples/sorafs_rollout_stage_c.toml` contiennent des extraits de code prêts à l'emploi

### Client Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` comprend l'enregistrement de phase analysé (`crates/iroha/src/client.rs:2315`) et les commandes d'assistance (pour `iroha_cli app sorafs fetch`) et la politique d'anonymat par défaut. ساتھ rapport کر سکیں۔

## Automatisation

Les assistants `cargo xtask` planifient la génération et automatisent la capture d'artefacts.

1. **Le calendrier régional génère کریں**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```Durées `s`, `m`, `h`, et suffixe `d` pour chaque personne Le résumé `artifacts/soranet_pq_rollout_plan.json` et le résumé Markdown (`artifacts/soranet_pq_rollout_plan.md`) émettent une demande de modification et une demande de modification est émise.

2. **Les artefacts de forage et les signatures et les captures de capture**

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

   Il s'agit d'un artefact `artifacts/soranet_pq_rollout/<timestamp>_<label>/` qui fournit un artefact de calcul des résumés BLAKE3. `rollout_capture.json` contient des métadonnées et une charge utile avec la signature Ed25519. Il s'agit d'une clé privée et d'un signe du procès-verbal d'exercice d'incendie, ainsi que d'une gouvernance et d'une validation.

## Matrice d'indicateurs SDK et CLI

| Surfaces | Canari (stade A) | Rampe (étape B) | Par défaut (étape C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` récupérer | `--anonymity-policy stage-a` phase en cours | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuration de l'orchestrateur JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuration du client Rust (`iroha.toml`) | `rollout_phase = "canary"` (par défaut) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Commandes signées `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, en option `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, en option `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, en option `.ANON_STRICT_PQ` |
| Aides de l'orchestrateur JavaScript | `rolloutPhase: "canary"` et `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python`fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Rapide `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Le SDK bascule entre l'analyseur de scène et la carte et l'orchestrateur pour la phase de verrouillage (`crates/sorafs_orchestrator/src/lib.rs:365`) et les déploiements multilingues configurés en mode étape de verrouillage. رہتے ہیں۔

## Liste de contrôle de planification Canary

1. **Vérification en amont (T moins 2 semaines)**

- Confirmez le taux de baisse de tension de l'étape A avec une couverture inférieure à 1 % et une couverture PQ dans la région >=70 % (`sorafs_orchestrator_pq_candidate_ratio`).
   - Calendrier des créneaux d'examen de la gouvernance et approbation de la fenêtre Canary
   - Mise en scène de la mise à jour `sorafs.gateway.rollout_phase = "ramp"` (modification JSON de l'orchestrateur et redéploiement) et pipeline de promotion et essai à sec

2. **Relais canari (jour T)**

   - La région de la région fait la promotion du système `rollout_phase = "ramp"` de l'orchestrateur et des manifestes de relais participants pour définir les paramètres du système.
   - Tableau de bord PQ Ratchet avec "Événements de politique par résultat" et "Taux de baisse de tension" et cache de garde TTL et moniteur de surveillance (tableau de bord et panneau de déploiement)
   - Stockage d'audit pour les instantanés `sorafs_cli guard-directory fetch` et pour les instantanés

3. **Canari client/SDK (T plus 1 semaine)**

   - Les configurations client comme `rollout_phase = "ramp"` retournent les cohortes SDK et remplacent `stage-b`.
   - Capture des différences de télémétrie (`sorafs_orchestrator_policy_events_total` et `client_id` et `region` du groupe) et journal des incidents de déploiement ساتھ attacher کریں۔

4. **Promotion par défaut (T plus 3 semaines)**- Approbation de la gouvernance pour l'orchestrateur et les configurations du client pour `rollout_phase = "default"` pour le commutateur et la liste de contrôle de préparation signée pour la publication des artefacts et la rotation des éléments

## Liste de contrôle sur la gouvernance et les preuves

| Changement de phase | Porte de promotion | Paquet de preuves | Tableaux de bord et alertes |
|--------------|----------------|-----------------|-----------|
| Canaries -> Rampe *(Aperçu de l'étape B)* | Taux de baisse de tension de l'étape A 14 jours <1%, `sorafs_orchestrator_pq_candidate_ratio` >= 0,7 région promue, vérification du ticket Argon2 p95 < 50 ms, promotion pour un créneau de gouvernance réservé | `cargo xtask soranet-rollout-plan` Paire JSON/Markdown, `sorafs_cli guard-directory fetch`, instantanés appariés (avant/après), ensemble signé `cargo xtask soranet-rollout-capture --label canary`, minutes Canary et [runbook à cliquet PQ] (./pq-ratchet-runbook.md) voir ici ہیں۔ | `dashboards/grafana/soranet_pq_ratchet.json` (Événements de politique + taux de baisse de tension) ، `dashboards/grafana/soranet_privacy_metrics.json` (taux de déclassement SN16) ، références de télémétrie dans `docs/source/soranet/snnet16_telemetry_plan.md`. |
| Rampe -> Par défaut *(Application de l'étape C)* | Burn-in de télémétrie SN16 de 30 jours pour la ligne de base `sn16_handshake_downgrade_total` et le canari client plat pour `sorafs_orchestrator_brownouts_total` pour le journal de répétition de bascule de proxy | Transcription `sorafs_cli proxy set-mode --mode gateway|direct`, sortie `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, journal `sorafs_cli guard-directory verify`, et bundle `cargo xtask soranet-rollout-capture --label default` signé | Et la carte PQ Ratchet et les panneaux de déclassement SN16 et `docs/source/sorafs_orchestrator_rollout.md` et `dashboards/grafana/soranet_privacy_metrics.json` sont documentés ici. |
| Rétrogradation d'urgence/préparation à la restauration | Déclencheur d'un pic de compteurs de déclassement en cas d'échec de la vérification du répertoire de garde et d'un tampon `/policy/proxy-toggle` pour les événements de déclassement | `docs/source/ops/soranet_transport_rollback.md`, liste de contrôle, journaux `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets d'incident, modèles de notification | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json`, et packs d'alertes (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- L'artefact `artifacts/soranet_pq_rollout/<timestamp>_<label>/` contient un magasin généré par `rollout_capture.json` qui contient des paquets de gouvernance, un tableau de bord, des traces Promtool et des résumés.
- Preuves téléchargées (minutes PDF, ensemble de captures, instantanés de garde) et SHA256 résume les minutes de promotion et les joindre en pièce jointe pour les approbations du Parlement.
- Ticket promotionnel pour le plan de télémétrie et les seuils d'alerte pour la source canonique `docs/source/soranet/snnet16_telemetry_plan.md`.

## Mises à jour du tableau de bord et de la télémétrie

`dashboards/grafana/soranet_pq_ratchet.json` sur le panneau d'annotation "Plan de déploiement" pour le navire et le playbook et le lien pour la phase actuelle des examens de gouvernance et la phase active تصدیق کر سکیں۔ Description du panneau et boutons de configuration pour la synchronisation et la synchronisation

Alerte règles de sécurité `stage` étiquette Canary et phases par défaut seuils de politique déclencheurs (`dashboards/alerts/soranet_handshake_rules.yml`).

## Crochets de restauration

### Par défaut -> Rampe (Étape C -> Étape B)1. `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` pour orchestrateur et rétrogradation (configurations SDK et miroir de phase) pour la flotte Stage B ہو۔
2. `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` Clients et profil de transport sécurisé pour la capture de transcription et `/policy/proxy-toggle` Workflow de remédiation auditable
3. `cargo xtask soranet-rollout-capture --label rollback-default` contient les différences du répertoire de garde, la sortie de Promtool et les captures d'écran du tableau de bord et `artifacts/soranet_pq_rollout/` archive les fichiers.

### Rampe -> Canary (Étape B -> Étape A)

1. Promotion pour capturer un instantané du répertoire de garde `sorafs_cli guard-directory import --guard-directory guards.json` et importer un instantané et `sorafs_cli guard-directory verify` pour rétrograder paquet de hachages شامل ہوں۔
2. Orchestrator pour les configurations du client comme `rollout_phase = "canary"` défini comme (`anonymity_policy stage-a` remplacement) et pour [PQ Ratchet Runbook] (./pq-ratchet-runbook.md) et PQ Ratchet Drill Repeat comme Le pipeline de déclassement prouve ہو۔
3. Mise à jour des captures d'écran de télémétrie PQ Ratchet et SN16 et des résultats des alertes et du journal des incidents et de la pièce jointe de la gouvernance et de la notification.

### Rappels de garde-corps

- Pour la rétrogradation et `docs/source/ops/soranet_transport_rollback.md`, reportez-vous à l'atténuation temporaire et au suivi du déploiement et à `TODO:` pour le journal et le suivi. سکے۔
- `dashboards/alerts/soranet_handshake_rules.yml` et `dashboards/alerts/soranet_privacy_rules.yml` pour la restauration et la couverture `promtool test rules` pour l'ensemble de capture de dérive d'alerte. ساتھ document ہو۔