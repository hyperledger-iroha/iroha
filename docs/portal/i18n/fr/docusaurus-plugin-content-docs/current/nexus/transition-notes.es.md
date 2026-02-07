---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/transition-notes.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-transition-notes
titre : Notes de transition de Nexus
description : En particulier de `docs/source/nexus_transition_notes.md`, qui cubre preuve de transition de Phase B, le calendrier des auditoires et les atténuations.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Notes de transition de Nexus

Cet enregistrement fait le travail pendant la **Phase B - Nexus Transition Foundations** jusqu'à finaliser la liste de contrôle de lancement multi-voies. Complétez les entrées de jalons en `roadmap.md` et maintenez la preuve référencée par B1-B4 dans un seul endroit pour que la gouvernance, SRE et les dirigeants du SDK partagent la même source de vérité.

## Alcance et cadence

- Cubre les auditoires acheminés et les garde-corps de télémétrie (B1/B2), le ensemble de deltas de configuration approuvé par l'administration (B3) et les suivis de l'installation de lancement multi-voies (B4).
- Remplaza la nota temporal de cadencia que antes vivia aqui; Depuis l'auditoire du premier trimestre 2026, le rapport détaillé réside dans `docs/source/nexus_routed_trace_audit_report_2026q1.md`, alors que cette page gère le calendrier opérationnel et le registre des atténuations.
- Actualisez les tableaux après chaque ventana routed-trace, voto de gobernanza ou ensayo de lanzamiento. Lorsque les objets sont utilisés, réfléchissez à la nouvelle emplacement dans cette page pour que les documents postérieurs (statut, tableaux de bord, portails SDK) puissent être affichés à une page estable.## Aperçu des preuves (T1-T2 2026)

| Flux de travail | Preuve | Propriétaire(s) | État | Notes |
|------------|----------|--------------|--------|-------|
| **B1 - Audits de trace acheminée** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @télémétrie-opérations, @gouvernance | Complet (T1 2026) | Tres ventanas de auditoria registradas; le retour TLS de `TRACE-CONFIG-DELTA` se produit lors de la rediffusion du deuxième trimestre. |
| **B2 - Remédiation de la télémétrie et des garde-corps** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Complet | Pack d'alerte, politique du bot de différence et tamano de lote OTLP (journal `nexus.scheduler.headroom` + panneau de Grafana de marge) envoyés ; le péché renonce à l'ouverture. |
| **B3 - Évaluations des deltas de configuration** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Complet | Vote GOV-2026-03-19 enregistré; le paquet fourni alimente le paquet de télémétrie cité en bas. |
| **B4 - Ensayo de lanzamiento multivoie** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Complet (T2 2026) | La réexécution du Canary du T2 concerne l'atténuation du retour TLS ; Le manifeste du validateur + `.sha256` capture la plage des emplacements 912-936, la graine de charge de travail `NEXUS-REH-2026Q2` et le hachage du profil TLS enregistré dans la réexécution. |

## Calendrier trimestriel des auditoires routé-trace| Identifiant de trace | Ventana (UTC) | Résultat | Notes |
|--------------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Approuvé | L'admission à la file d'attente P95 se maintient muy por debajo del objetivo <=750 ms. Aucune action n’est requise. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Approuvé | La relecture OTLP hache les compléments à `status.md` ; la parité du SDK diff bot confirme la dérive zéro. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12h00-12h30 | Résultat vers | Le retour de TLS se poursuit lors de la rediffusion du deuxième trimestre ; Le pack de télémétrie pour `NEXUS-REH-2026Q2` enregistre le hachage du profil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (ver `artifacts/nexus/tls_profile_rollout_2026q2/`) et ne coûte rien. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Approuvé | Graine de charge de travail `NEXUS-REH-2026Q2` ; pack de télémétrie + manifeste/digest en `artifacts/nexus/rehearsals/2026q1/` (plage d'emplacements 912-936) avec agenda en `artifacts/nexus/rehearsals/2026q2/`. |

Les trimestres futurs doivent agréger de nouvelles fils et déplacer les entrées terminées dans une annexe lorsque le tableau est crezca plus pendant le trimestre actuel. La référence est la section des rapports routed-trace ou des minutes de fonctionnement en utilisant l'ancla `#quarterly-routed-trace-audit-schedule`.

## Atténuations et éléments du backlog| Article | Description | Propriétaire | Objet | État/Notes |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Terminer la propagation du profil TLS qui a été retracé au cours de `TRACE-CONFIG-DELTA`, capturer les preuves de la réexécution et enregistrer le registre d'atténuation. | @release-eng, @sre-core | Ventana acheminé-trace du T2 2026 | Cerrado - hachage de profil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` capturé et `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` ; la rediffusion confirme qu'elle n'a pas été reçue. |
| `TRACE-MULTILANE-CANARY` préparation | Programmez l'essai de Q2, ajoutez les accessoires au pack de télémétrie et assurez-vous que les exploits du SDK réutilisent l'assistant validé. | @telemetry-ops, programme SDK | Appel de avion 2026-04-30 | Complété - agenda enregistré en `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` avec métadonnées d'emplacement/charge de travail ; réutilisation du harnais annotée sur le tracker. |
| Rotation du résumé du pack de télémétrie | Exécutez `scripts/telemetry/validate_nexus_telemetry_pack.py` avant chaque enregistrement/version et les résumés du registraire avec le tracker de configuration delta. | @opérations de télémétrie | Por release candidate | Completado - `telemetry_manifest.json` + `.sha256` émis et `artifacts/nexus/rehearsals/2026q1/` (plage d'emplacements `912-936`, graine `NEXUS-REH-2026Q2`) ; digère les copies du tracker et de l'indice de preuve. |

## Intégration du bundle de configuration delta- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` suit le résumé canonique des différences. Lorsque vous lisez de nouveaux `defaults/nexus/*.toml` ou changements de genèse, actualisez ce tracker en premier et réfléchissez ensuite aux points clés ici.
- Les bundles de configuration signés alimentent le pack de télémétrie de l'analyse. Le pack, validé par `scripts/telemetry/validate_nexus_telemetry_pack.py`, doit être publié avec la preuve du delta de configuration pour que les opérateurs puissent reproduire les artefacts exacts utilisés au cours de B4.
- Les bundles de Iroha 2 sans voies : les configurations avec `nexus.enabled = false` ont maintenant remplacé les remplacements de voie/espace de données/routage à moins que le profil Nexus soit autorisé (`--sora`), afin d'éliminer les sections `nexus.*` de las plantillas à voie unique.
- Gardez le registre du vote d'État (GOV-2026-03-19) inscrit tanto du tracker comme de cette note pour que les futurs votes puissent copier le format sans redescubrir le rituel d'approbation.

## Suite de l'essai de lancement- `docs/source/runbooks/nexus_multilane_rehearsal.md` capture le plan Canary, la liste des participants et les étapes de restauration ; actualisez le runbook lorsque vous modifiez la topologie des voies ou les exportateurs de télémétrie.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` lista cada artefacto revisado durante el ensayo del 9 de avril y incluye ahora notas/agenda de preparacion Q2. Ajoutez à l'avenir des informations sur le même tracker à la place de l'ouverture de trackers pour maintenir la preuve monotone.
- Extraits publics du collecteur OTLP et des exportations de Grafana (version `docs/source/telemetry.md`) lorsque vous modifiez le guide de traitement par lots de l'exportateur ; la mise à jour du premier trimestre a porté sur la taille du lot à 256 mois pour éviter les alertes de marge.
- La preuve de CI/tests multi-voies est désormais disponible en `integration_tests/tests/nexus/multilane_pipeline.rs` et correspond au flux de travail `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), en remplaçant la référence retirée `pytests/nexus/test_multilane_pipeline.py` ; Gardez le hachage pour `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) et synchronisez-le avec le tracker pour rechercher les bundles d'analyse.

## Cycle de vie des voies en runtime- Les plans de cycle de vie des voies en runtime valident désormais les liaisons de l'espace de données et interrompent la réconciliation de Kura/almacenamiento par niveaux, en laissant le catalogue sans changement. Les assistants peuvent relayer les voies en cache pour les voies retirées, de manière à ce que la synthèse du grand livre de fusion ne réutilise pas les preuves obsolètes.
- Application aux plans des assistants de configuration/cycle de vie de Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) pour agréger/retirer les voies sans reinicio ; le routage, les instantanés TEU et les registres de manifestes se rechargent automatiquement selon un plan de sortie.
- Conseils pour les opérateurs : lorsqu'un plan échoue, révisez les espaces de données erronés ou les racines de stockage qui ne peuvent pas être crearées (racine froide à plusieurs niveaux/répertoires Kura par voie). Corriger les routes de base et de réintention ; les avions exitosos réémettent le différentiel de télémétrie de voie/espace de données pour que les tableaux de bord reflètent la nouvelle topologie.

## Telemetria NPoS et preuves de contre-pression

Le rétro-essai de lancement de la phase B capture des données télémétriques déterministes qui testent le stimulateur cardiaque NPoS et les capacités de potins se maintiennent à l'intérieur de nos limites de contre-pression. Le faisceau d'intégration en `integration_tests/tests/sumeragi_npos_performance.rs` crée ces scénarios et émet des résumés JSON (`sumeragi_baseline_summary::<scenario>::...`) lorsqu'il ajoute de nouvelles mesures. Exécuté localement avec :

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```Configurez `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` ou `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` pour explorer les topologies les plus importantes ; les valeurs par défaut reflètent le profil des récolecteurs 1 s/`k=3` utilisé en B4.| Scénario / test | Couverture | Clé de télémétrie |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Blocage de 12 ronds avec le temps de bloc de l'envoi pour enregistrer les enveloppes de latence EMA, les profondeurs de cola et les jauges d'envoi redondant avant de sérialiser le paquet de preuves. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Inondez le colis de transactions pour garantir que les différences d'admission soient actives de manière déterministe et que le colis exporte des contadores de capacité/saturation. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Vérifiez la gigue du stimulateur cardiaque et les délais d'attente jusqu'à démontrer que la bande +/-125 pour mille est appliquée. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Empuja payloads RBC grandes jusqu'aux limites soft/hard du magasin pour montrer que les sessions et les contadores d'octets suben, rétrocédés et établis sans ouvrir le magasin. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Il s'agit de retransmissions pour que les jauges de rapport d'envoi redondant et les contadores de collecteurs sur cible avancent, démontrant que la télémétrie est effectuée pour le rétro est connecté de bout en bout. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. || `npos_rbc_chunk_loss_fault_reports_backlog` | Téléchargez des morceaux à intervalles déterminés pour vérifier que les moniteurs de backlog tombent en lieu et place de l'analyse silencieuse des charges utiles. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Ajoutez les lignes JSON qui impriment le harnais avec le grattage de Prometheus capturé lors de l'exécution siempre que la gouvernance sollicite la preuve que les alarmes de contre-pression coïncident avec la topologie de l'essai.

## Checklist de mise à jour

1. Agrega nuevas ventanas routé-trace et retrace les antiguas cuando roten los trimestres.
2. Actualisez le tableau des mesures d'atténuation après chaque suivi d'Alertmanager, y compris si l'action est de supprimer le ticket.
3. Lorsque vous modifiez les deltas de configuration, actualisez le tracker, cette note et la liste des résumés du pack de télémétrie dans la même demande d'extraction.
4. Découvrez ici tout nouvel artefact d'analyse/télémétrie pour que les futures actualisations de la feuille de route puissent référencer un seul document à la place de notes dispersées ad hoc.

## Indice de preuve| Actif | Emplacement | Notes |
|-------|----------|-------|
| Rapport d'audience routé-trace (T1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Source canonique pour la preuve de la phase B1 ; réfléchi pour le portail en `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker de configuration delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contiens les résumés des différences TRACE-CONFIG-DELTA, les premières révisions et le journal des votes GOV-2026-03-19. |
| Plan de remédiation de télémétrie | `docs/source/nexus_telemetry_remediation_plan.md` | Documentez le pack d'alertes, le tamano de lote OTLP et les garde-corps de présupposé d'exportation vinculados à B2. |
| Tracker d'essai multi-voies | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Liste des artefacts de l'essai du 9 avril, manifeste/résumé du validateur, notes/agenda Q2 et preuves de restauration. |
| Manifeste/résumé du pack de télémétrie (dernier) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Enregistrez la plage de slots 912-936, la graine `NEXUS-REH-2026Q2` et les hachages d'artefacts pour les bundles de gouvernance. |
| Manifeste de profil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash du profil TLS approuvé capturé lors de la rediffusion du deuxième trimestre ; citar en annexes route-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notes de planification pour l'essai Q2 (ventana, plage de slots, graine de charge de travail, propriétaires d'actions). |
| Runbook d'essai de lancement | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Liste de contrôle opérationnelle pour la mise en scène -> exécution -> restauration ; actualiser lorsque vous modifiez la topologie des voies ou le guide des exportateurs. || Validateur de pack télémétrie | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI référencé par le rétro B4 ; archiva digère conjointement avec le tracker lorsque le pack change. |
| Régression multivoie | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Essayez `nexus.enabled = true` pour les configurations multi-voies, conservez les hachages du catalogue Sora et prévoyez des itinéraires Kura/merge-log par voie (`blocks/lane_{id:03}_{slug}`) via `ConfigLaneRouter` avant de publier les résumés des artefacts. |