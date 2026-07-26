---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-rollout-plan
titre : Plan de despliegue poscuantico SNNet-16G
sidebar_label : Plan de despliegue PQ
description : Guide d'utilisation pour promouvoir la poignée de main hybride X25519+ML-KEM de SoraNet à partir de Canary avec des relais, des clients et des SDK par défaut.
---

:::note Fuente canonica
Cette page reflète `docs/source/soranet/pq_rollout_plan.md`. Manten ambas copias sincronizadas.
:::

SNNet-16G complète le déploiement possible pour le transport de SoraNet. Les contrôles `rollout_phase` permettent aux opérateurs de coordonner une promotion déterminée des besoins actuels de protection de l'étape A jusqu'à la couverture majoritaire de l'étape B et la position PQ restreinte de l'étape C sans éditer JSON/TOML en brut pour toute la surface.

Ce cube de playbook :

- Définitions des phases et des nouveaux boutons de configuration (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) connectés à la base de code (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapeo des drapeaux du SDK et de la CLI pour que chaque client puisse suivre le déploiement.
- Les attentes de planification du relais/client Canary en fonction des tableaux de bord de gouvernance qui garantissent la promotion (`dashboards/grafana/soranet_pq_ratchet.json`).
- Crochets de restauration et références au runbook de fire-drill ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Carte des phases

| `rollout_phase` | Etapa de anonymat effectif | Effet par défaut | Utilisation typique |
|-----------------|---------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Étape A) | Exigez au moins une garde PQ pour le circuit pendant que la flotte est chaude. | Baseline et premières semaines de Canary. |
| `ramp` | `anon-majority-pq` (Étape B) | Sesga la seleccion hacia relays PQ para >= dos tercios de cobertura; relais clasicos permanents como repli. | Canaries par région de relais; bascule de l'aperçu au SDK. |
| `default` | `anon-strict-pq` (Étape C) | Appliquez les circuits en solo PQ et endurez les alarmes de déclassement. | Promotion finale une fois terminée la télémétrie et l'approbation de la gouvernance. |

Si une surface définit également un `anonymity_policy` explicite, cette phase remplace ce composant. Omettre l'étape explicite maintenant différer la valeur de `rollout_phase` pour que les opérateurs puissent changer la phase una vez por entorno et laisser les clients la ici.

## Référence de configuration

### Orchestrateur (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Le chargeur de l'orchestrateur résout l'étape de secours au moment de l'exécution (`crates/sorafs_orchestrator/src/lib.rs:2229`) et l'expose via `sorafs_orchestrator_policy_events_total` et `sorafs_orchestrator_pq_ratio_*`. Voir `docs/examples/sorafs_rollout_stage_b.toml` et `docs/examples/sorafs_rollout_stage_c.toml` pour les listes d'extraits à appliquer.

### Client Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` enregistre maintenant la phase analysée (`crates/iroha/src/client.rs:2315`) pour que les assistants (par exemple `iroha_cli app sorafs fetch`) puissent signaler la phase actuelle avec la politique anonymisée par défaut.

## Automatisation

Deux assistants `cargo xtask` automatisent la génération du calendrier et la capture des artefacts.

1. **Générer l'horaire régional**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```Les durées acceptent les sufijos `s`, `m`, `h` ou `d`. La commande émet `artifacts/soranet_pq_rollout_plan.json` et un CV en Markdown (`artifacts/soranet_pq_rollout_plan.md`) que vous pouvez envoyer avec la demande de changement.

2. **Capturer les objets du foret avec confirmation**

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

   La commande copie les archives suministrados en `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calcule les résumés BLAKE3 pour chaque artefact et écrit `rollout_capture.json` avec les métadonnées mais une entreprise Ed25519 sur la charge utile. Utilisez la même clé privée qui confirme les minutes de l'exercice d'incendie pour que la gouvernance valide la capture rapidement.

## Matrice des drapeaux du SDK et de la CLI

| Superficie | Canari (stade A) | Rampe (étape B) | Par défaut (étape C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` récupérer | `--anonymity-policy stage-a` ou confier en phase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuration de l'orchestrateur JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuration du client Rust (`iroha.toml`) | `rollout_phase = "canary"` (par défaut) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Commandes signées `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, en option `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, en option `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, en option `.ANON_STRICT_PQ` |
| Aides de l'orchestrateur JavaScript | `rolloutPhase: "canary"` ou `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python`fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Rapide `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Toutes les options du SDK correspondent à l'analyseur des étapes utilisées par l'orchestrateur (`crates/sorafs_orchestrator/src/lib.rs:365`), car les applications multilingues sont maintenues en mode verrouillage avec la phase configurée.

## Liste de contrôle de planification de Canary

1. **Vérification en amont (T moins 2 semaines)**

- Confirmer que la charge de baisse de tension de la mer Stage A <1% pendant les deux semaines précédentes et que la couverture PQ mer >=70% par région (`sorafs_orchestrator_pq_candidate_ratio`).
   - Programmer le créneau de révision de la gouvernance qui ouvre la fenêtre des Canaries.
   - Actualiser `sorafs.gateway.rollout_phase = "ramp"` en staging (éditer le JSON de l'orchestrateur et redéployer) et exécuter à sec le pipeline de promotion.

2. **Relais canari (jour T)**

   - Promouvoir une région lorsque vous avez configuré `rollout_phase = "ramp"` dans l'orchestrateur et dans les manifestes des participants au relais.
   - Surveillez les "Événements de politique par résultat" et "Taux de baisse de tension" dans le tableau de bord PQ Ratchet (qui inclut maintenant le panneau de déploiement) pendant le double du cache de garde TTL.
   - Cortar snapshots de `sorafs_cli guard-directory fetch` avant et après l'éjection pour le stockage de l'auditoire.

3. **Canari client/SDK (T plus 1 semaine)**

   - Changer `rollout_phase = "ramp"` dans les configurations du client ou passer les remplacements `stage-b` pour les cohortes de SDK désignées.
   - Capturer les différences de télémétrie (`sorafs_orchestrator_policy_events_total` agrégé par `client_id` et `region`) et compléter le journal des incidents de déploiement.

4. **Promotion par défaut (T plus 3 semaines)**- Une fois que l'entreprise de gouvernance, modifier tant l'orchestrateur que les configurations du client sur `rollout_phase = "default"` et faire pivoter la liste de contrôle de préparation de l'entreprise jusqu'aux artefacts de publication.

## Checklist de gouvernance et de preuves

| Changement de phase | Porte de promotion | Bundle de preuves | Tableaux de bord et alertes |
|--------------|----------------|-----------------|-----------|
| Canaries -> Rampe *(Aperçu de l'étape B)* | Tasa de brownout Stage A <1% dans les 14 derniers jours, `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 pour la promotion de la région, vérification du ticket Argon2 p95 < 50 ms, et l'emplacement de gouvernance pour la promotion réservée. | Par JSON/Markdown de `cargo xtask soranet-rollout-plan`, des instantanés emparejados de `sorafs_cli guard-directory fetch` (avant/après), un bundle ferme `cargo xtask soranet-rollout-capture --label canary` et des minutes de référence Canary [PQ ratchet runbook](./pq-ratchet-runbook.md). | `dashboards/grafana/soranet_pq_ratchet.json` (Événements de politique + taux de baisse de tension), `dashboards/grafana/soranet_privacy_metrics.json` (taux de déclassement SN16), références de télémétrie et `docs/source/soranet/snnet16_telemetry_plan.md`. |
| Rampe -> Par défaut *(Application de l'étape C)* | Burn-in de la télémétrie SN16 de 30 jours effectué, `sn16_handshake_downgrade_total` plan sur la ligne de base, `sorafs_orchestrator_brownouts_total` en zéro pendant le Canary du client, et la répétition du proxy toggle enregistré. | Transcription de `sorafs_cli proxy set-mode --mode gateway|direct`, sortie de `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, journal de `sorafs_cli guard-directory verify`, et bundle ferme `cargo xtask soranet-rollout-capture --label default`. | Mismo tablero PQ Ratchet mas los paneles de downgrade SN16 documentados en `docs/source/sorafs_orchestrator_rollout.md` et `dashboards/grafana/soranet_privacy_metrics.json`. |
| Rétrogradation d'urgence/préparation à la restauration | Si vous activez lorsque les contadores de downgrade suben, la vérification du répertoire de garde ou le tampon `/policy/proxy-toggle` enregistre les événements de downgrade pris en charge. | Liste de contrôle de `docs/source/ops/soranet_transport_rollback.md`, journaux de `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets d'incidents et modèles de notification. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` et d'autres packs d'alertes (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Gardez chaque artefact sous `artifacts/soranet_pq_rollout/<timestamp>_<label>/` avec le `rollout_capture.json` généré pour que les paquets de gouvernance contiennent le tableau de bord, les tâches de Promtool et les résumés.
- Adjunta digère SHA256 de evidencia subida (minutes PDF, capture bundle, guard snapshots) aux minutes de promotion pour que les autorisations du Parlement puissent être reproduites sans accès au cluster de mise en scène.
- Référez-vous au plan de télémétrie sur le ticket de promotion pour vérifier que `docs/source/soranet/snnet16_telemetry_plan.md` est en même temps la source canonique de vocabulaire de déclassement et les parapluies d'alerte.

## Actualisations des tableaux de bord et de la télémétrie

`dashboards/grafana/soranet_pq_ratchet.json` comprend désormais un panneau d'annotations « Plan de déploiement » qui s'affiche dans ce manuel de jeu et montre la phase actuelle pour que les révisions de gouvernance confirment qu'elles sont actives. Gardez la description du panneau synchronisée avec les changements futurs dans les boutons de configuration.

Pour alerter, assurez-vous que les règles existantes utilisent l'étiquette `stage` pour que les phases Canary et les seuils politiques disparates par défaut (`dashboards/alerts/soranet_handshake_rules.yml`).

## Crochets de restauration

### Par défaut -> Rampe (Étape C -> Étape B)1. Baissez l'orchestrateur avec `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (et réfléchissez à la même phase dans les configurations du SDK) pour que Stage B puisse entrer dans l'ensemble.
2. Accédez aux clients au profil de transport sécurisé via `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, en capturant la transcription pour que le flux de travail de correction `/policy/proxy-toggle` soit auditable.
3. Exécutez `cargo xtask soranet-rollout-capture --label rollback-default` pour archiver les différences du répertoire de garde, la sortie de promtool et les captures d'écran des tableaux de bord sous `artifacts/soranet_pq_rollout/`.

### Rampe -> Canary (Étape B -> Étape A)

1. Importez l'instantané du répertoire de garde capturé avant la promotion avec `sorafs_cli guard-directory import --guard-directory guards.json` et vuelve à exécuter `sorafs_cli guard-directory verify` pour que le paquet de démo inclue des hachages.
2. Ajustez `rollout_phase = "canary"` (ou remplacez par `anonymity_policy stage-a`) dans les configurations de l'orchestrateur et du client, puis répétez l'exercice à cliquet PQ à partir du [PQ ratchet runbook](./pq-ratchet-runbook.md) pour tester le pipeline de rétrogradation.
3. Captures d'écran supplémentaires actualisées de PQ Ratchet et de la télémétrie SN16 avec les résultats d'alertes dans le journal des incidents avant de notifier une gouvernance.

### Enregistrements de garde-corps

- Référence `docs/source/ops/soranet_transport_rollback.md` chaque fois que vous effectuez une démo et enregistrez toute atténuation temporelle comme un élément `TODO:` dans le tracker de déploiement pour un travail ultérieur.
- Maintenez `dashboards/alerts/soranet_handshake_rules.yml` et `dashboards/alerts/soranet_privacy_rules.yml` sous la couverture de `promtool test rules` avant et après un rollback pour la dérive des alertes documentées avec le bundle de capture.