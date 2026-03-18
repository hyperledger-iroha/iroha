<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: fr
direction: ltr
source: docs/source/nexus_transition_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: baf4e91fdb2c447c453711170e7df58a9a4831b15957052818736e0e7914e8a3
source_last_modified: "2025-12-13T06:33:05.401788+00:00"
translation_last_reviewed: 2026-01-01
---

# Notes de transition Nexus

Ce journal suit le travail restant de **Phase B - Fondations de transition Nexus**
jusqu'a la fin de la checklist de lancement multi-lane. Il complete les entrees de
jalons dans `roadmap.md` et garde les preuves referencees par B1-B4 au meme endroit
afin que gouvernance, SRE et leads SDK partagent la meme source de verite.

## Portee et cadence

- Couvre les audits routed-trace et les garde-fous de telemetrie (B1/B2), le
  lot de deltas de configuration approuves par la gouvernance (B3) et les suivis
  de la repetition de lancement multi-lane (B4).
- Remplace la note de cadence temporaire qui vivait ici; depuis l'audit 2026
  Q1 le rapport detaille se trouve dans
  `docs/source/nexus_routed_trace_audit_report_2026q1.md`, tandis que cette page
  conserve le planning courant et le registre des mitigations.
- Mettre a jour les tableaux apres chaque fenetre de routed-trace, vote de gouvernance
  ou repetition de lancement. Quand les artefacts bougent, refleter le nouvel emplacement
  dans cette page pour que les docs en aval (status, dashboards, portails SDK)
  puissent lier vers un ancrage stable.

## Instantane des preuves (2026 Q1-Q2)

| Flux de travail | Preuves | Responsables | Statut | Notes |
|----------------|---------|--------------|--------|-------|
| **B1 - Audits routed-trace** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | Complete (Q1 2026) | Trois fenetres d'audit enregistrees; le retard TLS de `TRACE-CONFIG-DELTA` a ete clos pendant le rerun Q2. |
| **B2 - Remediation telemetrie et garde-fous** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Complete | Pack d'alertes, politique du diff bot et taille de lot OTLP (`nexus.scheduler.headroom` log + panneau headroom Grafana) livres; aucune derogation ouverte. |
| **B3 - Approbations de delta de config** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Complete | Vote GOV-2026-03-19 capture; le bundle signe alimente le pack telemetrie note ci-dessous. |
| **B4 - Repetition de lancement multi-lane** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Complete (Q2 2026) | Le rerun canari Q2 a clos la mitigation du retard TLS; manifest de validateurs + `.sha256` capturent la plage de slots 912-936, la graine de charge `NEXUS-REH-2026Q2` et le hash du profil TLS enregistre du rerun. |

## Calendrier trimestriel des audits routed-trace

| ID de trace | Fenetre (UTC) | Resultat | Notes |
|------------|--------------|----------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Passe | Le P95 de queue-admission est reste bien en dessous de la cible <=750 ms. Aucune action requise. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Passe | Hashes de replay OTLP attaches a `status.md`; parite du diff bot SDK confirmee a zero drift. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Resolu | Le retard TLS a ete clos pendant le rerun Q2; le pack telemetrie pour `NEXUS-REH-2026Q2` enregistre le hash du profil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (voir `artifacts/nexus/tls_profile_rollout_2026q2/`) et zero retardataire. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Passe | Graine de charge `NEXUS-REH-2026Q2`; pack telemetrie + manifest/digest sous `artifacts/nexus/rehearsals/2026q1/` (plage de slots 912-936) avec l'agenda dans `artifacts/nexus/rehearsals/2026q2/`. |

Les trimestres futurs doivent ajouter de nouvelles lignes et deplacer les entrees
terminees dans une annexe quand le tableau depasse le trimestre courant. Referencer
cette section depuis les rapports routed-trace ou les minutes de gouvernance en
utilisant l'ancre `#quarterly-routed-trace-audit-schedule`.

## Elements de mitigation et backlog

| Element | Description | Responsable | Cible | Statut / Notes |
|--------|-------------|------------|-------|----------------|
| `NEXUS-421` | Terminer la propagation du profil TLS en retard lors de `TRACE-CONFIG-DELTA`, capturer la preuve du rerun et fermer le registre de mitigation. | @release-eng, @sre-core | Fenetre routed-trace Q2 2026 | Clos - hash de profil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` capture dans `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; rerun confirme sans retardataires. |
| Preparation `TRACE-MULTILANE-CANARY` | Planifier la repetition Q2, joindre les fixtures au pack telemetrie et s'assurer que les harness SDK reutilisent le helper valide. | @telemetry-ops, SDK Program | Appel de planification 2026-04-30 | Complete - agenda stocke dans `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` avec metadonnees de slot/charge; reutilisation du harness notee dans le tracker. |
| Rotation des digests du pack telemetrie | Executer `scripts/telemetry/validate_nexus_telemetry_pack.py` avant chaque repetition/release et consigner les digests a cote du tracker de delta de config. | @telemetry-ops | Par release candidate | Complete - `telemetry_manifest.json` + `.sha256` emis dans `artifacts/nexus/rehearsals/2026q1/` (plage de slots `912-936`, graine `NEXUS-REH-2026Q2`); digests copies dans le tracker et l'index des preuves. |

## Integration du bundle de deltas de config

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` reste le
  resume canonique des diffs. Quand de nouveaux `defaults/nexus/*.toml` ou des
  changements de genesis arrivent, mettre a jour ce tracker d'abord puis
  refleter les points cle ici.
- Les bundles de config signes alimentent le pack telemetrie de repetition. Le pack,
  valide par `scripts/telemetry/validate_nexus_telemetry_pack.py`, doit etre publie
  avec la preuve de delta de config pour que les operateurs puissent rejouer les
  artefacts exacts utilises pendant B4.
- Les bundles Iroha 2 restent sans lane: les configs avec `nexus.enabled = false`
  rejettent desormais les overrides lane/dataspace/routing sauf si le profil Nexus
  est active (`--sora`), donc retirer les sections `nexus.*` des templates single-lane.
- Garder le log de vote de gouvernance (GOV-2026-03-19) lie depuis le tracker et
  cette note pour que les votes futurs copient le format sans re-decouvrir le rituel
  d'approbation.

## Suivis de repetition de lancement

- `docs/source/runbooks/nexus_multilane_rehearsal.md` capture le plan canari,
  la liste des participants et les etapes de rollback; rafraichir le runbook
  quand la topologie des lanes ou les exporters de telemetrie changent.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` liste chaque artefact
  verifie pendant la repetition du 9 avril et contient maintenant les notes/agenda Q2.
  Ajouter les repetitions futures au meme tracker au lieu d'ouvrir des trackers
  ponctuels pour garder la preuve monotone.
- Publier les snippets du collecteur OTLP et les exports Grafana (voir `docs/source/telemetry.md`)
  quand la guidance de batching change; la mise a jour Q1 a monte la taille de lot a
  256 echantillons pour eviter les alertes headroom.
- La preuve CI/test multi-lane vit maintenant dans
  `integration_tests/tests/nexus/multilane_pipeline.rs` et tourne sous le workflow
  `Nexus Multilane Pipeline`
  (`.github/workflows/integration_tests_multilane.yml`), remplacant la reference
  retiree `pytests/nexus/test_multilane_pipeline.py`; garder le hash de
  `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b
  `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) synchronise
  avec le tracker lors du rafraichissement des bundles de repetition.

## Cycle de vie des lanes en runtime

- Les plans de cycle de vie des lanes en runtime valident maintenant les bindings
  de dataspace et abortent lorsque la reconciliation Kura/stockage en niveaux echoue,
  laissant le catalogue inchange. Les helpers prunent les relays de lane en cache
  pour les lanes retirees afin que la synthese merge-ledger ne reutilise pas des proofs
  perimees.
- Appliquer les plans via les helpers config/lifecycle Nexus (`State::apply_lane_lifecycle`,
  `Queue::apply_lane_lifecycle`) pour ajouter/retirer des lanes sans redemarrage; le routage,
  les snapshots TEU et les registries de manifest se rechargent automatiquement apres un plan reussi.
- Guidance operateur: quand un plan echoue, verifier les dataspaces manquants ou les roots
  de stockage impossibles a creer (cold root en niveaux / dossiers Kura de lane). Corriger
  les chemins et reessayer; les plans reussis re-emettent le diff telemetrie lane/dataspace
  pour que les dashboards refletent la nouvelle topologie.

## Preuves de telemetrie et backpressure NPoS

La retro de repetition de lancement Phase B a demande des captures deterministes de
telemetrie pour prouver que le pacemaker NPoS et les couches gossip restent dans leurs
limites de backpressure. Le harness d'integration dans
`integration_tests/tests/sumeragi_npos_performance.rs` exerce ces scenarios et emet
des resumes JSON (`sumeragi_baseline_summary::<scenario>::...`) a chaque ajout de
nouvelles metriques. Lancer localement avec:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Definir `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` ou
`SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` pour explorer des topologies plus chargees; les
valeurs par defaut refletent le profil collecteur 1 s/`k=3` utilise en B4.

| Scenario / test | Couverture | Telemetrie cle |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloque 12 rounds avec le block time de repetition pour enregistrer les enveloppes de latence EMA, les profondeurs de file et les gauges redundant-send avant de serialiser le bundle de preuve. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Inonde la file de transactions pour garantir que les deferrals d'admission s'activent deterministement et que la file exporte des compteurs de capacite/saturation. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Echantillonne le jitter du pacemaker et les timeouts de vue jusqu'a prouver que la bande configuree de +/-125 permille est respectee. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Pousse de gros payloads RBC jusqu'aux limites soft/hard du store pour montrer que les sessions et compteurs d'octets montent, reculent et se stabilisent sans depasser le store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Force des retransmissions pour que les gauges redundant-send et les compteurs collectors-on-target avancent, prouvant que la telemetrie demandee est cablee de bout en bout. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Abandonne des chunks espaces deterministement pour verifier que les moniteurs de backlog remontent des fautes au lieu de drainer les payloads en silence. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Joindre les lignes JSON imprimees par le harness avec le scrape Prometheus capture
pendant l'execution quand la gouvernance demande une preuve que les alarmes de
backpressure correspondent a la topologie de repetition.

## Checklist de mise a jour

1. Ajouter de nouvelles fenetres routed-trace et retirer les anciennes quand les trimestres tournent.
2. Mettre a jour le tableau de mitigation apres chaque suivi Alertmanager, meme si l'action est
   de fermer le ticket.
3. Quand les deltas de config changent, mettre a jour le tracker, cette note et la liste de digests
   du pack telemetrie dans la meme pull request.
4. Lier ici tout nouvel artefact de repetition/telemetrie afin que les futures mises a jour de status
   referencent un seul document au lieu de notes ad-hoc dispersees.

## Index des preuves

| Actif | Emplacement | Notes |
|-------|-------------|-------|
| Rapport d'audit routed-trace (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Source canonique pour la preuve Phase B1; miroir pour le portail sous `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker de delta de config | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contient les resumes TRACE-CONFIG-DELTA, initiales des relecteurs et le log de vote GOV-2026-03-19. |
| Plan de remediation telemetrie | `docs/source/nexus_telemetry_remediation_plan.md` | Documente le pack d'alertes, la taille de lot OTLP et les garde-fous de budget d'export lies a B2. |
| Tracker de repetition multi-lane | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Liste les artefacts de la repetition du 9 avril, manifest/digest validateurs, notes/agenda Q2 et preuves de rollback. |
| Manifest/digest du pack telemetrie (dernier) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Enregistre la plage de slots 912-936, la graine `NEXUS-REH-2026Q2` et les hashes d'artefacts pour les bundles de gouvernance. |
| Manifest du profil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash du profil TLS approuve capture pendant le rerun Q2; citer dans les appendices routed-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notes de planification pour la repetition Q2 (fenetre, plage de slots, graine de charge, responsables). |
| Runbook de repetition de lancement | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Checklist operationnelle pour staging -> execution -> rollback; mettre a jour quand la topologie des lanes ou la guidance exporter change. |
| Validateur du pack telemetrie | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI referencee par la retro B4; archiver les digests avec le tracker a chaque changement du pack. |
| Regression multilane | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Prouve `nexus.enabled = true` pour les configs multi-lane, preserve les hashes du catalogue Sora et provisionne les chemins Kura/merge-log locaux (`blocks/lane_{id:03}_{slug}`) via `ConfigLaneRouter` avant de publier les digests d'artefacts. |
