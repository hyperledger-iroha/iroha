---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-rollout-plan
titre : Playbook de déploiement pos-quantico SNNet-16G
sidebar_label : Plan de déploiement PQ
description : Guide opérationnel pour promouvoir l'hibernation de prise de contact X25519+ML-KEM de SoraNet de Canary pour les relais, les clients et les SDK par défaut.
---

:::note Fonte canonica
Cette page espelha `docs/source/soranet/pq_rollout_plan.md`. Mantenha ambas comme copies synchronisées.
:::

SNNet-16G conclut le déploiement quantitatif du transport SoraNet. Les boutons `rollout_phase` permettent aux opérateurs de coordonner une promotion déterministe des exigences actuelles de protection de l'étape A pour la couverture majoritaire de l'étape B et de la position PQ établie Stage C sans éditer JSON/TOML brut pour chaque surface.

Ce livre de jeu contient :

- Définition de la phase et des nouveaux boutons de configuration (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) câblés sans base de code (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mappage des flags SDK et CLI pour que chaque client accompagne le déploiement.
- Attentes de planification du relais/client Canary et des tableaux de bord de gouvernance qui sont générés par la promotion (`dashboards/grafana/soranet_pq_ratchet.json`).
- Crochets de restauration et références au runbook Fire-Drill ([PQ Ratchet Runbook](./pq-ratchet-runbook.md)).

## Carte des phases

| `rollout_phase` | Estagio de anonymato efetivo | Efeito padrão | Utilisation typique |
|-----------------|---------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Étape A) | Exigir ao menos um guard PQ por circuit enquanto a frota aquece. | Baseline et semaines initiales de Canary. |
| `ramp` | `anon-majority-pq` (Étape B) | Viesar a selecao para relays PQ com >= dois tercos de cover; relais classicos ficam como repli. | Canaries par région de relais ; bascule l'aperçu dans le SDK. |
| `default` | `anon-strict-pq` (Étape C) | Les circuits Forcar signalent PQ et supportent les alarmes de déclassement. | Promocao final apos telemetria et signature de gouvernance. |

Si une surface est également définie explicitement par `anonymity_policy`, elle indique la phase pour chaque composant. Omettre la scène explicitement il y a quelques instants pour reporter la valeur de `rollout_phase` afin que les opérateurs puissent effectuer une phase une fois par environnement et deixar les clients herdarem.

## Référence de configuration

### Orchestrateur (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Le chargeur fait que l'orchestrateur résout l'étape de secours dans le runtime (`crates/sorafs_orchestrator/src/lib.rs:2229`) et l'expose via `sorafs_orchestrator_policy_events_total` et `sorafs_orchestrator_pq_ratio_*`. Voir `docs/examples/sorafs_rollout_stage_b.toml` et `docs/examples/sorafs_rollout_stage_c.toml` pour des extraits de code immédiatement à appliquer.

### Client Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` vient d'enregistrer une phase analysée (`crates/iroha/src/client.rs:2315`) pour que les commandes d'assistance (par exemple `iroha_cli app sorafs fetch`) signalent une phase actuelle avec la politique de l'anonymat.

## Automatisation

Dois helpers `cargo xtask` automatise la gestion du planning et la capture des objets.

1. **Gérer l'horaire régional**

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

   Durées selon les suffixes `s`, `m`, `h` ou `d`. La commande émet `artifacts/soranet_pq_rollout_plan.json` et un résumé Markdown (`artifacts/soranet_pq_rollout_plan.md`) qui peut être envoyé avec une demande de changement.2. **Capturar artefatos do drill com assinaturas**

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

   En commandant la copie des archives fournies pour `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calculez les résumés BLAKE3 pour chaque ouvrage et écrivez `rollout_capture.json` avec les métadonnées et une sauvegarde Ed25519 sur la charge utile. Utilisez une clé privée unique qui sera utilisée pendant quelques minutes pour effectuer un exercice d'incendie afin qu'une gouvernance valide la capture rapide.

## Matrice des flags SDK et CLI

| Surfaces | Canari (stade A) | Rampe (étape B) | Par défaut (étape C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` récupérer | `--anonymity-policy stage-a` ou confier la phase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuration de l'orchestrateur JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuration du client Rust (`iroha.toml`) | `rollout_phase = "canary"` (par défaut) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Commandes signées `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, en option `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, en option `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, en option `.ANON_STRICT_PQ` |
| Aides de l'orchestrateur JavaScript | `rolloutPhase: "canary"` ou `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python`fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Rapide `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Toutes les options de mappage du SDK pour l'analyseur de scène utilisé par l'orchestrateur (`crates/sorafs_orchestrator/src/lib.rs:365`) permettent des déploiements multilingues en mode verrouillage avec une phase configurée.

## Liste de contrôle de planification Canary

1. **Vérification en amont (T moins 2 semaines)**

- Confirmer que le taux de baisse de tension de l'étape A est <1 % pendant les dernières semaines et que la couverture PQ est >=70 % par région (`sorafs_orchestrator_pq_candidate_ratio`).
   - Agendar o slot de gouvernance review qui approuve a janela de canary.
   - Actualiser `sorafs.gateway.rollout_phase = "ramp"` dans le staging (éditer l'orchestrateur JSON et redéployer) et faciliter l'exécution à sec du pipeline de promotion.

2. **Relais canari (jour T)**

   - Promouvoir une région pour que vous configuriez `rollout_phase = "ramp"` sans orchestrateur et nos manifestes de participants de relais.
   - Surveiller « Événements de politique par résultat » et « Taux de baisse de tension » dans le tableau de bord PQ Ratchet (avant le panneau de déploiement) par deux fois ou TTL dans le cache de garde.
   - Fazer snapshots de `sorafs_cli guard-directory fetch` avant et après pour l'armement de l'auditoire.

3. **Canari client/SDK (T plus 1 semaine)**

   - Trocar pour `rollout_phase = "ramp"` dans les configurations du client ou passer les remplacements `stage-b` pour les cohortes de SDK désignées.
   - Capturer les différences de télémétrie (`sorafs_orchestrator_policy_events_total` agrégé par `client_id` et `region`) et annexer le journal des incidents de déploiement.

4. **Promotion par défaut (T plus 3 semaines)**

   - Après avoir approuvé la gouvernance, modifier les configurations de l'orchestrateur et du client pour `rollout_phase = "default"` et faire pivoter la liste de contrôle de préparation associée aux artefacts de publication.

## Liste de contrôle sur la gouvernance et les preuves| Changement de phase | Porte de promotion | Paquet de preuves | Tableaux de bord et alertes |
|--------------|----------------|-----------------|-----------|
| Canaries -> Rampe *(Aperçu de l'étape B)* | Taux de baisse de tension Stage-A <1 % au cours des 14 dernières journées, `sorafs_orchestrator_pq_candidate_ratio` >= 0,7 pour la promotion régionale, vérification du ticket Argon2 p95 < 50 ms, et emplacement de gouvernance pour la promotion réservée. | Par JSON/Markdown de `cargo xtask soranet-rollout-plan`, instantanés pareados de `sorafs_cli guard-directory fetch` (avant/après), bundle associé `cargo xtask soranet-rollout-capture --label canary`, et minutes de référence Canary [PQ ratchet runbook](./pq-ratchet-runbook.md). | `dashboards/grafana/soranet_pq_ratchet.json` (Événements de politique + taux de baisse de tension), `dashboards/grafana/soranet_privacy_metrics.json` (taux de déclassement SN16), références de télémétrie sur `docs/source/soranet/snnet16_telemetry_plan.md`. |
| Rampe -> Par défaut *(Application de l'étape C)* | Burn-in de la télémétrie SN16 de 30 jours conclu, `sn16_handshake_downgrade_total` plat sans base de référence, `sorafs_orchestrator_brownouts_total` zéro pendant le canari du client et la répétition du proxy basculement enregistré. | Transcription `sorafs_cli proxy set-mode --mode gateway|direct`, sortie de `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, journal de `sorafs_cli guard-directory verify`, et bundle assassiné `cargo xtask soranet-rollout-capture --label default`. | La carte mesmo PQ Ratchet et les panneaux d'exploitation de downgrade SN16 sont documentés sur `docs/source/sorafs_orchestrator_rollout.md` et `dashboards/grafana/soranet_privacy_metrics.json`. |
| Rétrogradation d'urgence/préparation à la restauration | Actionné lorsque les compteurs de déclassement sont effectués, la vérification du répertoire de garde falha, ou le tampon `/policy/proxy-toggle` enregistre les événements de déclassement soutenus. | Liste de contrôle de `docs/source/ops/soranet_transport_rollback.md`, journaux de `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets d'incidents et modèles de notification. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` et packs d'alertes supplémentaires (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Armazene cada artefato em `artifacts/soranet_pq_rollout/<timestamp>_<label>/` com o `rollout_capture.json` Gerado para que les paquets de gouvernance contiennent le tableau de bord, les traces et les résumés de Promtool.
- L'annexe digère SHA256 des preuves apportées (minutes PDF, paquet de capture, instantanés de garde) en tant que minutes de promotion pour que, comme le Parlement l'approuve, elles puissent être reproduites sans accès au cluster de préparation.
- Référence au plan de télémétrie sans ticket de promotion pour vérifier que `docs/source/soranet/snnet16_telemetry_plan.md` suit l'envoi de la police canonique pour les vocabulaires de déclassement et les seuils d'alerte.

## Mises à jour du tableau de bord et de la télémétrie

`dashboards/grafana/soranet_pq_ratchet.json` comprend maintenant un panneau d'annotations « Plan de déploiement » qui est lié à ce manuel et montre la phase actuelle pour que les examens de gouvernance confirment quelle étape est en cours. Veuillez décrire le panneau synchronisé avec les futurs boutons de configuration.

Pour l'alerte, garantissez que les règles existantes utilisent l'étiquette `stage` pour que les phases Canaries soient séparées par défaut des seuils de politique disparates (`dashboards/alerts/soranet_handshake_rules.yml`).

## Crochets de restauration

### Par défaut -> Rampe (Étape C -> Étape B)1. Rétrogradez l'orchestrator avec `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (et regardez la même phase de nos configurations SDK) pour que l'étape B les renvoie aujourd'hui à l'avant.
2. Forcer les clients à créer un profil de transport sécurisé via `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, en capturant la transcription pour que le flux de travail `/policy/proxy-toggle` continue l'audit.
3. Rode `cargo xtask soranet-rollout-capture --label rollback-default` pour enregistrer les différences du répertoire de garde, la sortie Promtool et les captures d'écran du tableau de bord dans `artifacts/soranet_pq_rollout/`.

### Rampe -> Canary (Étape B -> Étape A)

1. Importez l'instantané du répertoire de garde capturé avant la promotion avec `sorafs_cli guard-directory import --guard-directory guards.json` et utilisez `sorafs_cli guard-directory verify` récemment pour que le paquet de rétrogradation comprenne des hachages.
2. Ajustez `rollout_phase = "canary"` (ou remplacez `anonymity_policy stage-a`) dans les configurations de l'orchestrateur et du client, après avoir utilisé récemment la perceuse à cliquet PQ dans le [PQ ratchet runbook](./pq-ratchet-runbook.md) pour tester le pipeline de rétrogradation.
3. Anexe captures d'écran mises à jour de PQ Ratchet et de télémétrie SN16, plus les résultats d'alertes sur le journal des incidents avant de notifier la gouvernance.

### Rappels de garde-corps

- Référence `docs/source/ops/soranet_transport_rollback.md` toujours pour détecter une rétrogradation et enregistrer toute atténuation temporaire comme l'élément `TODO:` sans outil de suivi de déploiement pour l'accompagnement.
- Mantenha `dashboards/alerts/soranet_handshake_rules.yml` et `dashboards/alerts/soranet_privacy_rules.yml` sur la couverture de `promtool test rules` avant et après une restauration pour que la dérive d'alerte soit documentée avec le bundle de capture.