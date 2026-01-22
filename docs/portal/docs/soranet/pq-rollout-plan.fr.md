<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4a7662c7edd2f4a8c0242017b0dc45f9b46477d892a1d06f2611e223aa2e6e48
source_last_modified: "2025-11-15T19:17:38.748600+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: pq-rollout-plan
title: Playbook de deploiement post-quantique SNNet-16G
sidebar_label: Plan de deploiement PQ
description: Guide operationnel pour promouvoir le handshake hybride X25519+ML-KEM de SoraNet de canary a default sur relays, clients et SDKs.
---

:::note Source canonique
:::

SNNet-16G termine le deploiement post-quantique pour le transport SoraNet. Les knobs `rollout_phase` permettent aux operateurs de coordonner une promotion deterministe du requirement guard Stage A vers la couverture majoritaire Stage B et la posture PQ stricte Stage C sans modifier du JSON/TOML brut pour chaque surface.

Ce playbook couvre:

- Definitions de phase et nouveaux knobs de configuration (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) cables dans le codebase (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapping des flags SDK et CLI afin que chaque client puisse suivre le rollout.
- Attentes de scheduling canary relay/client plus les dashboards de governance qui gate la promotion (`dashboards/grafana/soranet_pq_ratchet.json`).
- Hooks de rollback et references au fire-drill runbook ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Carte des phases

| `rollout_phase` | Etape d'anonymat effective | Effet par defaut | Usage typique |
|-----------------|---------------------------|----------------|---------------|
| `canary`        | `anon-guard-pq` (Stage A) | Exiger au moins un guard PQ par circuit pendant que la flotte se rechauffe. | Baseline et premieres semaines de canary. |
| `ramp`          | `anon-majority-pq` (Stage B) | Favoriser la selection vers des relays PQ pour >= deux tiers de couverture; les relays classiques restent en fallback. | Canary relays par region; toggles de preview SDK. |
| `default`       | `anon-strict-pq` (Stage C) | Imposer des circuits uniquement PQ et durcir les alarmes de downgrade. | Promotion finale une fois la telemetrie et le sign-off governance completes. |

Si une surface definit aussi un `anonymity_policy` explicite, elle override la phase pour ce composant. Omettre l'etape explicite deferre maintenant a la valeur `rollout_phase` afin que les operateurs puissent basculer une fois par environnement et laisser les clients l'heriter.

## Reference de configuration

### Orchestrator (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Le loader de l'orchestrator resout l'etape fallback en runtime (`crates/sorafs_orchestrator/src/lib.rs:2229`) et l'expose via `sorafs_orchestrator_policy_events_total` et `sorafs_orchestrator_pq_ratio_*`. Voir `docs/examples/sorafs_rollout_stage_b.toml` et `docs/examples/sorafs_rollout_stage_c.toml` pour des snippets prets a appliquer.

### Rust client / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` enregistre maintenant la phase parsee (`crates/iroha/src/client.rs:2315`) afin que les commandes helper (par exemple `iroha_cli app sorafs fetch`) puissent reporter la phase actuelle aux cotes de la politique d'anonymat par defaut.

## Automatisation

Deux helpers `cargo xtask` automatisent la generation du schedule et la capture d'artefacts.

1. **Generer le schedule regional**

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

   Les durees acceptent les suffixes `s`, `m`, `h` ou `d`. La commande emet `artifacts/soranet_pq_rollout_plan.json` et un resume Markdown (`artifacts/soranet_pq_rollout_plan.md`) a joindre a la demande de changement.

2. **Capturer les artefacts du drill avec signatures**

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

   La commande copie les fichiers fournis dans `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calcule des digests BLAKE3 pour chaque artefact et ecrit `rollout_capture.json` avec metadata plus une signature Ed25519 sur le payload. Utilisez la meme private key que celle qui signe les minutes du fire-drill pour que la governance puisse valider rapidement la capture.

## Matrice des flags SDK & CLI

| Surface | Canary (Stage A) | Ramp (Stage B) | Default (Stage C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` fetch | `--anonymity-policy stage-a` ou se reposer sur la phase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Orchestrator config JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust client config (`iroha.toml`) | `rollout_phase = "canary"` (default) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` signed commands | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, optionnel `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, optionnel `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, optionnel `.ANON_STRICT_PQ` |
| JavaScript orchestrator helpers | `rolloutPhase: "canary"` ou `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Tous les toggles SDK se mappent au meme parser de stage utilise par l'orchestrator (`crates/sorafs_orchestrator/src/lib.rs:365`), de sorte que les deploiements multi-langages restent en lock-step avec la phase configuree.

## Checklist de scheduling canary

1. **Preflight (T minus 2 weeks)**

- Confirmer que le brownout rate Stage A est <1% sur les deux semaines precedentes et que la couverture PQ est >=70% par region (`sorafs_orchestrator_pq_candidate_ratio`).
   - Planifier le slot de governance review qui approuve la fenetre canary.
   - Mettre a jour `sorafs.gateway.rollout_phase = "ramp"` en staging (editer le JSON orchestrator et redeployer) et dry-run le pipeline de promotion.

2. **Relay canary (T day)**

   - Promouvoir une region a la fois en definissant `rollout_phase = "ramp"` sur l'orchestrator et les manifests de relays participants.
   - Surveiller "Policy Events per Outcome" et "Brownout Rate" dans le dashboard PQ Ratchet (qui inclut maintenant le panel rollout) pendant deux fois le TTL du guard cache.
   - Capturer des snapshots `sorafs_cli guard-directory fetch` avant et apres pour stockage d'audit.

3. **Client/SDK canary (T plus 1 week)**

   - Basculer `rollout_phase = "ramp"` dans les configs client ou passer des overrides `stage-b` pour les cohorts SDK designees.
   - Capturer les diffs de telemetrie (`sorafs_orchestrator_policy_events_total` groupe par `client_id` et `region`) et les joindre au log d'incident de rollout.

4. **Default promotion (T plus 3 weeks)**

   - Une fois la governance validee, basculer orchestrator et configs client vers `rollout_phase = "default"` et faire tourner la checklist de readiness signee dans les artefacts de release.

## Checklist governance & evidence

| Changement de phase | Gate de promotion | Evidence bundle | Dashboards & alertes |
|--------------|----------------|-----------------|---------------------|
| Canary -> Ramp *(Stage B preview)* | Brownout rate Stage A <1% sur les 14 derniers jours, `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 par region promue, Argon2 ticket verify p95 < 50 ms, et slot governance reserve. | Paire JSON/Markdown `cargo xtask soranet-rollout-plan`, snapshots appaires `sorafs_cli guard-directory fetch` (avant/apres), bundle signe `cargo xtask soranet-rollout-capture --label canary`, et minutes canary referencant [PQ ratchet runbook](./pq-ratchet-runbook.md). | `dashboards/grafana/soranet_pq_ratchet.json` (Policy Events + Brownout Rate), `dashboards/grafana/soranet_privacy_metrics.json` (SN16 downgrade ratio), references telemetrie dans `docs/source/soranet/snnet16_telemetry_plan.md`. |
| Ramp -> Default *(Stage C enforcement)* | Burn-in telemetrie SN16 de 30 jours atteint, `sn16_handshake_downgrade_total` plat au baseline, `sorafs_orchestrator_brownouts_total` a zero durant le canary client, et rehearsal du proxy toggle logge. | Transcription `sorafs_cli proxy set-mode --mode gateway|direct`, sortie `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, log `sorafs_cli guard-directory verify`, et bundle signe `cargo xtask soranet-rollout-capture --label default`. | Meme tableau PQ Ratchet plus les panneaux SN16 downgrade documentes dans `docs/source/sorafs_orchestrator_rollout.md` et `dashboards/grafana/soranet_privacy_metrics.json`. |
| Emergency demotion / rollback readiness | Declenche quand les compteurs downgrade montent, la verification guard-directory echoue, ou le buffer `/policy/proxy-toggle` enregistre des downgrade events soutenus. | Checklist `docs/source/ops/soranet_transport_rollback.md`, logs `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets d'incident et templates de notification. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` et les deux packs d'alertes (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Stockez chaque artefact sous `artifacts/soranet_pq_rollout/<timestamp>_<label>/` avec le `rollout_capture.json` genere afin que les paquets governance contiennent le scoreboard, les traces promtool et les digests.
- Attachez les digests SHA256 des preuves chargees (minutes PDF, capture bundle, guard snapshots) aux minutes de promotion afin que les approbations Parliament puissent etre rejouees sans acces au cluster staging.
- Referencez le plan de telemetrie dans le ticket de promotion pour prouver que `docs/source/soranet/snnet16_telemetry_plan.md` reste la source canonique des vocabulaires de downgrade et des seuils d'alertes.

## Mise a jour dashboards & telemetrie

`dashboards/grafana/soranet_pq_ratchet.json` inclut maintenant un panel d'annotation "Rollout Plan" qui renvoie vers ce playbook et expose la phase actuelle afin que les revues governance confirment quel stage est actif. Gardez la description du panel synchronisee avec les futures evolutions des knobs de configuration.

Pour l'alerting, assurez-vous que les regles existantes utilisent le label `stage` afin que les phases canary et default declenchent des thresholds de politique separes (`dashboards/alerts/soranet_handshake_rules.yml`).

## Hooks de rollback

### Default -> Ramp (Stage C -> Stage B)

1. Retrogradez l'orchestrator avec `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (et faites miroiter la meme phase sur les configs SDK) pour que Stage B reprenne sur toute la flotte.
2. Forcez les clients vers le profil de transport sur `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, en capturant la transcription afin que le workflow de remediation `/policy/proxy-toggle` reste auditable.
3. Executez `cargo xtask soranet-rollout-capture --label rollback-default` pour archiver les diffs guard-directory, la sortie promtool et les screenshots dashboards sous `artifacts/soranet_pq_rollout/`.

### Ramp -> Canary (Stage B -> Stage A)

1. Importez le snapshot guard-directory capture avant promotion avec `sorafs_cli guard-directory import --guard-directory guards.json` et relancez `sorafs_cli guard-directory verify` afin que le packet de demotion inclue les hashes.
2. Definissez `rollout_phase = "canary"` (ou override avec `anonymity_policy stage-a`) sur l'orchestrator et les configs client, puis rejouez le PQ ratchet drill du [PQ ratchet runbook](./pq-ratchet-runbook.md) pour prouver le pipeline downgrade.
3. Attachez les screenshots actualises PQ Ratchet et SN16 telemetrie ainsi que les resultats d'alertes au log d'incident avant notification governance.

### Rappels guardrail

- Referencez `docs/source/ops/soranet_transport_rollback.md` a chaque demotion et enregistrez toute mitigation temporaire comme item `TODO:` dans le rollout tracker pour suivi.
- Gardez `dashboards/alerts/soranet_handshake_rules.yml` et `dashboards/alerts/soranet_privacy_rules.yml` sous couverture `promtool test rules` avant et apres un rollback afin que toute derive d'alertes soit documentee avec le capture bundle.
