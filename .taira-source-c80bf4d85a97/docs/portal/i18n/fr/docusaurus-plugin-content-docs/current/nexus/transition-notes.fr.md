---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/transition-notes.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-transition-notes
titre : Notes de transition de Nexus
description : Miroir de `docs/source/nexus_transition_notes.md`, comprenant les preuves de transition Phase B, le calendrier d'audit et les atténuations.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Notes de transition de Nexus

Ce journal suit le travail restant de **Phase B - Nexus Transition Foundations** jusqu'à la fin de la checklist de lancement multi-lane. Il complète les entrées de jalons dans `roadmap.md` et garde les preuves référencées par B1-B4 en un seul endroit afin que la gouvernance, SRE et les leads SDK partagent la même source de vérité.

## Portee et cadence

- Couvrir les audits routed-trace et les garde-corps de télémétrie (B1/B2), l'ensemble de deltas de configuration approuvés par la gouvernance (B3) et les suivis de répétition de lancement multi-voies (B4).
- Remplacer la note temporaire de cadence qui vivait ici ; depuis l'audit Q1 2026, le rapport détaillé réside dans `docs/source/nexus_routed_trace_audit_report_2026q1.md`, tandis que cette page maintient le calendrier courant et le registre des atténuations.
- Mettez à jour les tables après chaque fenêtre acheminée-trace, vote de gouvernance ou répétition de lancement. Lorsque des artefacts bougent, reflètez le nouvel emplacement dans cette page pour que les docs aval (statuts, tableaux de bord, portails SDK) puissent lier une ancrage stable.

## Snapshot des preuves (T1-T2 2026)| Flux de travail | Préuvés | Propriétaire(s) | Statuts | Remarques |
|------------|----------|--------------|--------|-------|
| **B1 - Audits de trace acheminée** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @télémétrie-opérations, @gouvernance | Complet (T1 2026) | Trois fenêtres d'audit enregistrées; le retard TLS de `TRACE-CONFIG-DELTA` s'est clos pendant le rerun Q2. |
| **B2 - Remédiation de télémétrie et garde-corps** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Complet | Pack d'alerte, politique du diff bot et taille de lot OTLP (`nexus.scheduler.headroom` log + panel Grafana de headroom) livres; Aucune renonciation ouverte. |
| **B3 - Approbations de deltas de configuration** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Complet | Vote GOV-2026-03-19 enregistré; le bundle signe alimente le pack de télémétrie cite plus bas. |
| **B4 - Répétition de lancement multi-voies** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Complet (T2 2026) | Le rerun canary de Q2 a ferme la atténuation du retard TLS; le validateur manifest + `.sha256` capture la plage de slots 912-936, la graine de charge de travail `NEXUS-REH-2026Q2` et le hachage du profil TLS enregistre pendant le rerun. |

## Calendrier trimestriel des audits routé-trace| Identifiant de trace | Fenêtre (UTC) | Résultat | Remarques |
|--------------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Réussi | Queue-admission P95 est reste bien en dessous de la cible <=750 ms. Aucune action requise. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Réussi | Les hachages de relecture OTLP attachent un `status.md` ; la parite du diff bot SDK a confirmé la dérive zéro. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12h00-12h30 | Résolu | Le retard TLS s'est ferme lors de la rediffusion Q2; le pack de télémétrie pour `NEXUS-REH-2026Q2` enregistre le hash du profil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (voir `artifacts/nexus/tls_profile_rollout_2026q2/`) et zéro retardataires. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Réussi | Graine de charge de travail `NEXUS-REH-2026Q2` ; pack de télémétrie + manifest/digest dans `artifacts/nexus/rehearsals/2026q1/` (slot range 912-936) avec agenda dans `artifacts/nexus/rehearsals/2026q2/`. |

Les trimestres futurs doivent ajouter de nouvelles lignes et déplacer les entrées terminées vers une annexe lorsque la table dépasse le courant. Référencez cette section depuis les rapports routed-trace ou les minutes de gouvernance en utilisant l'ancre `#quarterly-routed-trace-audit-schedule`.

## Atténuations et éléments de backlog| Article | Descriptif | Propriétaire | Câble | Statuts / Notes |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Terminer la propagation du profil TLS qui a pris du retard pendant `TRACE-CONFIG-DELTA`, capturer les preuves du rerun et fermer le registre de mitigation. | @release-eng, @sre-core | Fenêtre routé-trace Q2 2026 | Clos - hash du profil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` capture dans `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; le rerun a confirmé zéro retardataires. |
| `TRACE-MULTILANE-CANARY` préparation | Programmer la répétition Q2, joindre des appareils au pack de télémétrie et s'assurer que les SDK exploitent réutiliser le helper valide. | @telemetry-ops, programme SDK | Appel de planification 2026-04-30 | Complet - agenda stocké dans `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` avec emplacement de métadonnées/charge de travail ; réutilisation du harnais noté dans le tracker. |
| Rotation du résumé du pack de télémétrie | Executer `scripts/telemetry/validate_nexus_telemetry_pack.py` avant chaque répétition/libération et enregistrer les résumés a cote du tracker de config delta. | @opérations de télémétrie | Candidat à la version Par | Complet - `telemetry_manifest.json` + `.sha256` émis dans `artifacts/nexus/rehearsals/2026q1/` (slot range `912-936`, seed `NEXUS-REH-2026Q2`) ; digère les copies dans le tracker et l'index des preuves. |

## Intégration du bundle de config delta- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` reste le résumé canonique des diffs. Quand de nouveaux `defaults/nexus/*.toml` ou changements de genèse arrivent, mettez à jour ce tracker d'abord puis reflètez les points clés ici.
- Les bundles de configuration signés alimentent le pack de répétition de télémétrie. Le pack, valide par `scripts/telemetry/validate_nexus_telemetry_pack.py`, doit être publié avec les preuves de config delta afin que les opérateurs puissent rejouer les artefacts exacts utilise pendant B4.
- Les bundles Iroha 2 restent sans voies : les configs avec `nexus.enabled = false` rejettent maintenant les overrides de lane/dataspace/routing sauf si le profil Nexus est actif (`--sora`), supprime donc les sections `nexus.*` des templates single-lane.
- Gardez le log de vote de gouvernance (GOV-2026-03-19) depuis le tracker et cette note pour que les futurs votes puissent copier le format sans devoir re-découvrir le rituel d'approbation.

## Suivis de répétition de lancement- `docs/source/runbooks/nexus_multilane_rehearsal.md` capture le plan canari, la liste des participants et les étapes de rollback ; Mettez à jour le runbook lorsque la topologie des voies ou les exportateurs de télémétrie changent.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` liste chaque artefact vérifié durant la répétition du 9 avril et contient maintenant les notes/agenda de préparation Q2. Ajoutez les répétitions futures au meme tracker au lieu d'ouvrir des trackers ad-hoc pour garder les preuves monotones.
- Publiez des snippets du collecteur OTLP et des exports Grafana (voir `docs/source/telemetry.md`) lorsque les consignes de batching de l'exportateur changent ; la mise à jour Q1 a porte la taille de lot à 256 echantillons pour éviter les alertes headroom.
- Les preuves CI/tests multi-voies vivent maintenant dans `integration_tests/tests/nexus/multilane_pipeline.rs` et tournent sous le workflow `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), remplaçant la référence retraitée `pytests/nexus/test_multilane_pipeline.py`; conservez le hash pour `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) en synchronisation avec le tracker lors du rafraichissement des bundles de répétition.

## Cycle de vie runtime des voies- Les plans de cycle de vie des voies en runtime valident maintenant les liaisons de dataspace et avortent quand la réconciliation Kura/stockage en paliers fait écho, laissant le catalogue inchangé. Les helpers éliminent les relais caches pour les voies retraités afin que la synthèse-grand livre ne réutilise pas des preuves obsolètes.
- Appliquez les plans via les helpers de config/lifecycle Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) pour ajouter/retirer des voies sans redémarrage ; routage, instantanés TEU et registres de manifestes se rechargent automatiquement après un plan de réussi.
- Guide de l'opérateur : lorsqu'un plan fait écho, vérifiez les espaces de données manquants ou les racines de stockage impossibles à créer (roots froids/répertoires Kura par voie). Corrigez les chemins de base et réessayez; les plans réussis ré-emettent le diff de télémétrie voie/dataspace pour que les tableaux de bord reflètent la nouvelle topologie.

## Télémétrie NPoS et preuves de contre-pression

Le rétro de répétition de lancement Phase B a demande des captures de télémétrie déterministes prouvant que le stimulateur NPoS et les couches de gossip restent dans leurs limites de contre-pression. Le harnais d'intégration dans `integration_tests/tests/sumeragi_npos_performance.rs` exerce ces scénarios et emet des CV JSON (`sumeragi_baseline_summary::<scenario>::...`) quand de nouvelles métriques arrivent. Lancez-le localement avec:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```Définissez `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` ou `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` pour explorer des topologies plus contraintes ; les valeurs par défaut reflètent le profil des collecteurs 1 s/`k=3` utiliser en B4.| Scénario / test | Couverture | Clé de télémétrie |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloque 12 tours avec le bloc temps de répétition pour enregistrer les enveloppes de latence EMA, les profondeurs de fichier et les jauges de redundant-send avant de sérialiser le bundle de preuves. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Inonde le fichier de transactions pour s'assurer que les reports d'admission s'activent de facon déterministe et que le fichier exporte les compteurs de capacité/saturation. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Échantillonne le jitter du stimulateur cardiaque et les timeouts de vue jusqu'à prouver que la bande +/-125 pour mille est appliquée. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Pousse de gros payloads RBC jusqu'aux limites soft/hard du store pour montrer que les sessions et compteurs de bytes montent, reculent et se stabilisent sans passer le store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Force des retransmissions pour que les jauges de ratio redondant-envoi et les compteurs de collecteurs-sur-cible avancent, prouvant que la télémétrie demandée par le rétro est branchée de bout en bout. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. || `npos_rbc_chunk_loss_fault_reports_backlog` | Laisse tomber des chunks à intervalles déterministes pour vérifier que les moniteurs de backlog signalent des fautes au lieu de drainer silencieusement les payloads. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Joignez les lignes JSON imprimées par le harnais avec le scrape Prometheus capture pendant l'exécution chaque fois que la gouvernance exige des preuves que les alarmes de contre-pression correspondent à la topologie de répétition.

## Checklist de mise à jour

1. Ajouter de nouvelles fenêtres acheminées-trace et supprimer les anciennes lorsque les trimestres tournent.
2. Mettez à jour la table d'atténuation après chaque suivi Alertmanager, même si l'action consiste à fermer le ticket.
3. Quand les config deltas changent, mettez à jour le tracker, cette note et la liste des résumés du pack de télémétrie dans la meme pull request.
4. Liez ici tout nouvel artefact de répétition/télémétrie pour que les futures mises à jour du roadmap puissent référencer un seul document au lieu de notes ad hoc dispersées.

## Index des preuves| Actif | Emplacement | Remarques |
|-------|----------|-------|
| Rapport d'audit routed-trace (T1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Source canonique pour les preuves Phase B1; miroir pour le portail sous `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker de configuration delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contient les curriculum vitae de diffs TRACE-CONFIG-DELTA, les initiales des reviewers et le log de vote GOV-2026-03-19. |
| Plan de remédiation de télémétrie | `docs/source/nexus_telemetry_remediation_plan.md` | Documentez le pack d'alerte, la taille du lot OTLP et les garde-fous de budget d'exportation au format B2. |
| Tracker de répétition multivoie | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Liste les artefacts vérifiés durant la répétition du 9 avril, manifest/digest validator, notes/agenda Q2 et preuves de rollback. |
| Manifeste/résumé du pack de télémétrie (dernier) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Enregistrez la plage 912-936, seed `NEXUS-REH-2026Q2` et les hashs d'artefacts pour les bundles de gouvernance. |
| Manifeste de profil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash du profil TLS approuve la capture pendant le rerun Q2; citez-le dans les annexes routé-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notes de planification pour la répétition Q2 (fenêtre, slot range, workload seed, propriétaires d'actions). || Runbook de répétition de lancement | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Checklist opérationnelle pour staging -> exécution -> rollback; mettre à jour quand la topologie des voies ou les conseils d'exportateurs changent. |
| Validateur de pack télémétrie | `scripts/telemetry/validate_nexus_telemetry_pack.py` | Référence CLI par le rétro B4 ; archivez les résumés avec le tracker chaque fois que le pack change. |
| Régression multivoie | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Valide `nexus.enabled = true` pour les configs multi-lane, conserve les hashes du catalogue Sora et provisionne les chemins Kura/merge-log par lane (`blocks/lane_{id:03}_{slug}`) via `ConfigLaneRouter` avant de publier les résumés d'artefacts. |