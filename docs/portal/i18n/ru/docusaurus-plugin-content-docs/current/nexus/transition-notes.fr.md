---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/transition-notes.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-transition-notes
title: Notes de transition de Nexus
description: Miroir de `docs/source/nexus_transition_notes.md`, couvrant les preuves de transition Phase B, le calendrier d'audit et les mitigations.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Notes de transition de Nexus

Ce journal suit le travail restant de **Phase B - Nexus Transition Foundations** jusqu'a la fin de la checklist de lancement multi-lane. Il complete les entrees de milestones dans `roadmap.md` et garde les preuves referencees par B1-B4 en un seul endroit afin que la gouvernance, SRE et les leads SDK partagent la meme source de verite.

## Portee et cadence

- Couvre les audits routed-trace et les guardrails de telemetrie (B1/B2), l'ensemble de deltas de configuration approuve par la gouvernance (B3) et les suivis de repetition de lancement multi-lane (B4).
- Remplace la note temporaire de cadence qui vivait ici; depuis l'audit Q1 2026, le rapport detaille reside dans `docs/source/nexus_routed_trace_audit_report_2026q1.md`, tandis que cette page maintient le calendrier courant et le registre des mitigations.
- Mettez a jour les tables apres chaque fenetre routed-trace, vote de gouvernance ou repetition de lancement. Lorsque des artefacts bougent, refletez le nouvel emplacement dans cette page pour que les docs aval (status, dashboards, portails SDK) puissent lier un ancrage stable.

## Snapshot des preuves (2026 Q1-Q2)

| Workstream | Preuves | Owner(s) | Statut | Notes |
|------------|----------|----------|--------|-------|
| **B1 - Routed-trace audits** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | Complet (Q1 2026) | Trois fenetres d'audit enregistrees; le retard TLS de `TRACE-CONFIG-DELTA` s'est clos pendant le rerun Q2. |
| **B2 - Remediation de telemetrie et guardrails** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Complet | Alert pack, politique du diff bot et taille de lot OTLP (`nexus.scheduler.headroom` log + panel Grafana de headroom) livres; aucun waiver ouvert. |
| **B3 - Approbations de deltas de configuration** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Complet | Vote GOV-2026-03-19 enregistre; le bundle signe alimente le pack de telemetrie cite plus bas. |
| **B4 - Repetition de lancement multi-lane** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Complet (Q2 2026) | Le rerun canary de Q2 a ferme la mitigation du retard TLS; le validator manifest + `.sha256` capture la plage de slots 912-936, workload seed `NEXUS-REH-2026Q2`, et le hash du profil TLS enregistre pendant le rerun. |

## Calendrier trimestriel des audits routed-trace

| Trace ID | Fenetre (UTC) | Resultat | Notes |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Reussi | Queue-admission P95 est reste bien en dessous de la cible <=750 ms. Aucune action requise. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Reussi | OTLP replay hashes attaches a `status.md`; la parite du diff bot SDK a confirme zero drift. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Resolu | Le retard TLS s'est ferme durant le rerun Q2; le pack de telemetrie pour `NEXUS-REH-2026Q2` enregistre le hash du profil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (voir `artifacts/nexus/tls_profile_rollout_2026q2/`) et zero retardataires. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Reussi | Workload seed `NEXUS-REH-2026Q2`; pack de telemetrie + manifest/digest dans `artifacts/nexus/rehearsals/2026q1/` (slot range 912-936) avec agenda dans `artifacts/nexus/rehearsals/2026q2/`. |

Les trimestres futurs doivent ajouter de nouvelles lignes et deplacer les entrees terminees vers une annexe quand la table depasse le trimestre courant. Referencez cette section depuis les rapports routed-trace ou les minutes de gouvernance en utilisant l'ancre `#quarterly-routed-trace-audit-schedule`.

## Mitigations et items de backlog

| Item | Description | Owner | Cible | Statut / Notes |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Terminer la propagation du profil TLS qui a pris du retard pendant `TRACE-CONFIG-DELTA`, capturer les preuves du rerun et fermer le registre de mitigation. | @release-eng, @sre-core | Fenetre routed-trace Q2 2026 | Clos - hash du profil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` capture dans `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; le rerun a confirme zero retardataires. |
| `TRACE-MULTILANE-CANARY` prep | Programmer la repetition Q2, joindre des fixtures au telemetry pack et s'assurer que les SDK harnesses reutilisent le helper valide. | @telemetry-ops, SDK Program | Appel de planification 2026-04-30 | Complet - agenda stocke dans `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` avec metadata slot/workload; reutilisation du harness notee dans le tracker. |
| Telemetry pack digest rotation | Executer `scripts/telemetry/validate_nexus_telemetry_pack.py` avant chaque repetition/release et enregistrer les digests a cote du tracker de config delta. | @telemetry-ops | Par release candidate | Complet - `telemetry_manifest.json` + `.sha256` emis dans `artifacts/nexus/rehearsals/2026q1/` (slot range `912-936`, seed `NEXUS-REH-2026Q2`); digests copies dans le tracker et l'index des preuves. |

## Integration du bundle de config delta

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` reste le resume canonique des diffs. Quand de nouveaux `defaults/nexus/*.toml` ou changements de genesis arrivent, mettez a jour ce tracker d'abord puis refletez les points cle ici.
- Les signed config bundles alimentent le telemetry pack de repetition. Le pack, valide par `scripts/telemetry/validate_nexus_telemetry_pack.py`, doit etre publie avec les preuves de config delta afin que les operateurs puissent rejouer les artefacts exacts utilises pendant B4.
- Les bundles Iroha 2 restent sans lanes: les configs avec `nexus.enabled = false` rejettent maintenant les overrides de lane/dataspace/routing sauf si le profil Nexus est active (`--sora`), donc supprimez les sections `nexus.*` des templates single-lane.
- Gardez le log de vote de gouvernance (GOV-2026-03-19) lie depuis le tracker et cette note pour que les futurs votes puissent copier le format sans devoir re-decouvrir le rituel d'approbation.

## Suivis de repetition de lancement

- `docs/source/runbooks/nexus_multilane_rehearsal.md` capture le plan canary, la liste des participants et les etapes de rollback; mettez a jour le runbook quand la topologie des lanes ou les exporters de telemetrie changent.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` liste chaque artefact verifie durant la repetition du 9 avril et contient maintenant les notes/agenda de preparation Q2. Ajoutez les repetitions futures au meme tracker au lieu d'ouvrir des trackers ad-hoc pour garder les preuves monotones.
- Publiez des snippets du collecteur OTLP et des exports Grafana (voir `docs/source/telemetry.md`) quand les consignes de batching de l'exporter changent; la mise a jour Q1 a porte la taille de lot a 256 echantillons pour eviter des alertes headroom.
- Les preuves CI/tests multi-lane vivent maintenant dans `integration_tests/tests/nexus/multilane_pipeline.rs` et tournent sous le workflow `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), remplacant la reference retiree `pytests/nexus/test_multilane_pipeline.py`; gardez le hash pour `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) en sync avec le tracker lors du rafraichissement des bundles de repetition.

## Cycle de vie runtime des lanes

- Les plans de cycle de vie des lanes en runtime valident maintenant les bindings de dataspace et abortent quand la reconciliation Kura/stockage en paliers echoue, laissant le catalogue inchange. Les helpers eliminent les relays caches pour les lanes retirees afin que la synthese merge-ledger ne reutilise pas des proofs obsoletes.
- Appliquez les plans via les helpers de config/lifecycle Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) pour ajouter/retirer des lanes sans redemarrage; routing, snapshots TEU et registries de manifests se rechargent automatiquement apres un plan reussi.
- Guide operateur: quand un plan echoue, verifiez les dataspaces manquants ou storage roots impossibles a creer (tiered cold root/repertoires Kura par lane). Corrigez les chemins de base et reessayez; les plans reussis re-emettent le diff de telemetrie lane/dataspace pour que les dashboards refletent la nouvelle topologie.

## Telemetrie NPoS et preuves de backpressure

Le retro de repetition de lancement Phase B a demande des captures de telemetrie deterministes prouvant que le pacemaker NPoS et les couches de gossip restent dans leurs limites de backpressure. Le harness d'integration dans `integration_tests/tests/sumeragi_npos_performance.rs` exerce ces scenarios et emet des resumes JSON (`sumeragi_baseline_summary::<scenario>::...`) quand de nouvelles metriques arrivent. Lancez-le localement avec:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Definissez `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` ou `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` pour explorer des topologies plus stressees; les valeurs par defaut refletent le profil de collecteurs 1 s/`k=3` utilise en B4.

| Scenario / test | Couverture | Telemetrie cle |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloque 12 rounds avec le block time de repetition pour enregistrer les enveloppes de latence EMA, les profondeurs de file et les gauges de redundant-send avant de serialiser le bundle de preuves. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Inonde la file de transactions pour s'assurer que les deferrals d'admission s'activent de facon deterministe et que la file exporte les compteurs de capacite/saturation. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Echantillonne le jitter du pacemaker et les timeouts de vue jusqu'a prouver que la bande +/-125 permille est appliquee. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Pousse de gros payloads RBC jusqu'aux limites soft/hard du store pour montrer que les sessions et compteurs de bytes montent, reculent et se stabilisent sans depasser le store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Force des retransmissions pour que les gauges de ratio redundant-send et les compteurs de collectors-on-target avancent, prouvant que la telemetrie demandee par le retro est branchee end-to-end. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Laisse tomber des chunks a intervalles deterministes pour verifier que les moniteurs de backlog signalent des fautes au lieu de drainer silencieusement les payloads. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Joignez les lignes JSON imprimees par le harness avec le scrape Prometheus capture pendant l'execution chaque fois que la gouvernance demande des preuves que les alarmes de backpressure correspondent a la topologie de repetition.

## Checklist de mise a jour

1. Ajoutez de nouvelles fenetres routed-trace et retirez les anciennes lorsque les trimestres tournent.
2. Mettez a jour la table de mitigation apres chaque suivi Alertmanager, meme si l'action consiste a fermer le ticket.
3. Quand les config deltas changent, mettez a jour le tracker, cette note et la liste de digests du telemetry pack dans la meme pull request.
4. Liez ici tout nouvel artefact de repetition/telemetrie pour que les futures mises a jour du roadmap puissent referencer un seul document au lieu de notes ad-hoc dispersees.

## Index des preuves

| Actif | Emplacement | Notes |
|-------|----------|-------|
| Rapport d'audit routed-trace (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Source canonique pour les preuves Phase B1; miroir pour le portail sous `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker de config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contient les resumes de diffs TRACE-CONFIG-DELTA, initiales des reviewers et le log de vote GOV-2026-03-19. |
| Plan de remediation de telemetrie | `docs/source/nexus_telemetry_remediation_plan.md` | Documente l'alert pack, la taille de lot OTLP et les guardrails de budget d'export lies a B2. |
| Tracker de repetition multi-lane | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Liste les artefacts verifies durant la repetition du 9 avril, manifest/digest validator, notes/agenda Q2 et preuves de rollback. |
| Telemetry pack manifest/digest (latest) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Enregistre la plage 912-936, seed `NEXUS-REH-2026Q2` et les hashes d'artefacts pour les bundles de gouvernance. |
| TLS profile manifest | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash du profil TLS approuve capture pendant le rerun Q2; citez-le dans les annexes routed-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notes de planification pour la repetition Q2 (fenetre, slot range, workload seed, owners d'actions). |
| Runbook de repetition de lancement | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Checklist operationnelle pour staging -> execution -> rollback; mettre a jour quand la topologie des lanes ou les conseils d'exporters changent. |
| Telemetry pack validator | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI reference par le retro B4; archivez les digests avec le tracker chaque fois que le pack change. |
| Multilane regression | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Valide `nexus.enabled = true` pour les configs multi-lane, preserve les hashes du catalogue Sora et provisionne les chemins Kura/merge-log par lane (`blocks/lane_{id:03}_{slug}`) via `ConfigLaneRouter` avant de publier les digests d'artefacts. |
