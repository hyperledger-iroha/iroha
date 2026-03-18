---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-transition-notes
titre : Nexus ٹرانزیشن نوٹس
description: `docs/source/nexus_transition_notes.md` Phase B de la phase B ہے۔
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus ٹرانزیشن نوٹس

**Phase B - Nexus Fondations de transition** pour une construction à plusieurs voies et une construction à plusieurs voies مکمل نہ ہو جائے۔ `roadmap.md` jalons de référence pour les preuves et les preuves B1-B4 رکھتا ہے تاکہ gouvernance, SRE اور SDK لیڈز ایک ہی source de vérité شیئر کر سکیں۔

## اسکوپ اور cadence

- routed-trace, garde-corps de télémétrie (B1/B2), gouvernance, configuration delta de configuration (B3), suivi des répétitions de lancement multi-voies (B4) et suivi des répétitions de lancement multi-voies (B4)
- یہ عارضی cadence نوٹ کی جگہ لیتا ہے جو پہلے یہاں تھا؛ Q1 2026 آڈٹ کے بعد تفصیلی رپورٹ `docs/source/nexus_routed_trace_audit_report_2026q1.md` میں ہے، جبکہ یہ صفحہ جاری شیڈول اور atténuation رجسٹر رکھتا ہے۔
- Routed-trace, gouvernance et répétition de lancement Les artefacts sont en cours de création de documents en aval (statut, tableaux de bord, portails SDK) مستحکم ancre سے لنک کر سکیں۔

## Aperçu des preuves (T1-T2 2026)| ورک اسٹریم | ثبوت | مالکان | اسٹیٹس | نوٹس |
|------------|----------|--------------|--------|-------|
| **B1 - Audits de trace acheminée** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @télémétrie-opérations, @gouvernance | مکمل (T1 2026) | تین آڈٹ ونڈوز ریکارڈ ہوئیں؛ `TRACE-CONFIG-DELTA` pour la réexécution du décalage TLS Q2 en fin de compte |
| **B2 - Remédiation télémétrique et garde-corps** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | مکمل | pack d'alertes, stratégie de bot diff, et taille du lot OTLP (journal `nexus.scheduler.headroom` + panneau de marge Grafana) کوئی واورز اوپن نہیں۔ |
| **B3 - Approbations delta de configuration** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | مکمل | GOV-2026-03-19 et réponse bundle signé ici et pack de télémétrie et flux ici |
| **B4 - Répétition de lancement multi-voies** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | مکمل (T2 2026) | Réexécution de Canary au deuxième trimestre pour atténuer le décalage TLS manifeste du validateur + `.sha256` pour les emplacements 912-936, graine de charge de travail `NEXUS-REH-2026Q2` pour réexécuter le hachage de profil TLS et capturer le fichier |

## سہ ماہی routed-trace آڈٹ شیڈول| Identifiant de trace | ونڈو (UTC) | نتیجہ | نوٹس |
|--------------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | پاس | Admission de file d'attente P95 ہدف <=750 ms سے کافی نیچے رہا۔ کوئی ایکشن درکار نہیں۔ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | پاس | Hachages de relecture OTLP `status.md` en cours de lecture SDK diff bot parity sans dérive nulle |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12h00-12h30 | حل شدہ | TLS lag Q2 rediffusion maintenant `NEXUS-REH-2026Q2` Pack de télémétrie avec hachage de profil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` pour les retardataires (`artifacts/nexus/tls_profile_rollout_2026q2/`) pour les retardataires |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | پاس | Graine de charge de travail `NEXUS-REH-2026Q2` ; pack de télémétrie + manifeste/digest `artifacts/nexus/rehearsals/2026q1/` noir (plage d'emplacements 912-936) et agenda `artifacts/nexus/rehearsals/2026q2/` blanc |

آنے والے سہ ماہیوں میں نئی قطاریں شامل کریں اور جب ٹیبل موجودہ سہ ماہی سے بڑا ہو جائے تو مکمل اندراجات کو annexe میں منتقل کریں۔ routed-trace رپورٹس یا minutes de gouvernance سے اس سیکشن کو `#quarterly-routed-trace-audit-schedule` Anchor کے ذریعے ریفرنس کریں۔

## Atténuations et éléments du backlog| آئٹم | تفصیل | مالک | ہدف | اسٹیٹس / نوٹس |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | `TRACE-CONFIG-DELTA` L'analyse du profil TLS et la propagation permettent de réexécuter la capture des preuves et le journal d'atténuation du journal | @release-eng, @sre-core | Trace acheminée du deuxième trimestre 2026 | Par exemple - Hachage de profil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` et `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` pour capturer une image relancez zéro traînards کی تصدیق کی۔ |
| `TRACE-MULTILANE-CANARY` préparation | Répétition du deuxième trimestre pour le pack de télémétrie et les luminaires du SDK pour valider la réutilisation des assistants. | @telemetry-ops, programme SDK | 2026-04-30 appel de planification | مکمل - agenda `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` میں محفوظ ہے جس میں métadonnées d'emplacement/charge de travail شامل ہے؛ tracker de réutilisation des harnais میں نوٹ ہے۔ |
| Rotation du résumé du pack de télémétrie | Répétition/sortie du `scripts/telemetry/validate_nexus_telemetry_pack.py` pour les résumés et le suivi delta de configuration. | @opérations de télémétrie | ہر candidat à la sortie | مکمل - `telemetry_manifest.json` + `.sha256` `artifacts/nexus/rehearsals/2026q1/` میں جاری ہوئے (plage d'emplacements `912-936`, graine `NEXUS-REH-2026Q2`) ; digests tracker et index des preuves |

## Intégration du bundle delta de configuration- Résumé des différences canoniques `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` ہے۔ Il s'agit d'un `defaults/nexus/*.toml` qui concerne les changements de genèse et le tracker.
- Pack de télémétrie de répétition de bundles de configuration signés et flux de données Le pack est pour `scripts/telemetry/validate_nexus_telemetry_pack.py` pour valider la configuration delta preuve et publier les opérateurs B4 pour les opérateurs B4. بالکل وہی replay des artefacts کر سکیں۔
- Iroha 2 faisceaux de voies pour les remplacements de voies : `nexus.enabled = false` et les configurations pour les remplacements de voie/espace de données/routage et rejeter les remplacements Le profil Nexus (`--sora`) permet de créer des modèles à une seule voie et des sections `nexus.*` pour les utilisateurs
- journal des votes sur la gouvernance (GOV-2026-03-19) et suivi des votes des votes de gouvernance (GOV-2026-03-19) دریافت کیے بغیر استعمال کر سکیں۔

## Suivis des répétitions de lancement- `docs/source/runbooks/nexus_multilane_rehearsal.md` Canary liste des participants et étapes de restauration pour le processus de mise à jour Topologie des voies et exportateurs de télémétrie pour le runbook et les exportateurs de télémétrie
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` 9 étapes de répétition et d'artefact pour les notes de préparation/ordre du jour du deuxième trimestre مستقبل کے répétitions اسی tracker میں شامل کریں تاکہ preuve monotone رہے۔
- Extraits de collecteur OTLP pour les exportations Grafana (`docs/source/telemetry.md`) pour publier des instructions de traitement par lots pour l'exportateur Mise à jour du premier trimestre avec alertes de marge pour une taille de lot de 256 échantillons par jour
- CI/preuve de test multivoies `integration_tests/tests/nexus/multilane_pipeline.rs` et flux de travail `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`) et `pytests/nexus/test_multilane_pipeline.py` ریفرنس کو ریٹائر کیا؛ `defaults/nexus/config.toml` hash (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) tracker de synchronisation pour les bundles de répétition

## Cycle de vie des voies d'exécution- Les plans de cycle de vie des voies d'exécution et les liaisons d'espace de données sont validées et la réconciliation Kura/stockage hiérarchisé échoue et l'abandon est supprimé du catalogue de stockage. ہے۔ assistants voies de circulation relais mis en cache tâches d'élagage synthèse de fusion et grand livre preuves réutilisation
- Aides à la configuration/cycle de vie Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) Les plans de démarrage s'appliquent aux voies de démarrage et au redémarrage pour ajouter/retirer le système. سکے؛ routage, instantanés TEU et registres de manifestes, plan et plan de rechargement
- Opérateurs en ligne : échec du plan, espaces de données manquants, racines de stockage et répertoires de racine froide à plusieurs niveaux/répertoires Kura Lane. chemins de base Plans de diff de télémétrie de voie/espace de données pour émettre des tableaux de bord et topologie de base

## Télémétrie NPoS et preuve de contre-pression

La répétition de lancement de la phase B rétro et la télémétrie déterministe capturent des captures d'écran d'un stimulateur cardiaque NPoS et de couches de potins d'une contre-pression d'un stimulateur cardiaque NPoS. `integration_tests/tests/sumeragi_npos_performance.rs` et les scénarios de harnais d'intégration et les résumés JSON (`sumeragi_baseline_summary::<scenario>::...`) émettent des métriques لوکل چلانے کے لئے:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
````SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` et `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` sont des topologies de stress et de topologies différentes. valeurs par défaut B4 میں استعمال ہونے والے 1 s/`k=3` collecteur پروفائل کو reflet کرتے ہیں۔| Scénario / test | Couverture | Télémétrie clé |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | temps de bloc de répétition est de 12 tours et enveloppes de latence EMA, profondeurs de file d'attente et jauges d'envoi redondant sont disponibles pour le paquet de preuves sérialisé. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | la file d'attente des transactions déclenche de manière déterministe l'exportation des compteurs de capacité/saturation de la file d'attente | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | gigue du stimulateur cardiaque et affichage des délais d'attente échantillon de +/-125 pour mille en termes de temps d'attente | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Les charges utiles RBC stockent les données soft/hard pour pousser les sessions et les compteurs d'octets pour stabiliser les débordements دکھایا جا سکے۔ | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | retransmet la force et les jauges de rapport d'envoi redondant et les compteurs collecteurs sur cible, les compteurs et les compteurs rétro et la télémétrie de bout en bout. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | les morceaux espacés de manière déterministe tombent et les arriérés surveillent les fuites et les défauts augmentent | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |Pour la gouvernance, les alarmes de contre-pression, la topologie de répétition et les correspondances avec le harnais, les lignes JSON et le scrape Prometheus. بھی منسلک کریں۔

## Mettre à jour la liste de contrôle

1. Routeed-trace est une application de trace routée et un outil de recherche de trace routée.
2. Suivi d'Alertmanager avec tableau d'atténuation et ticket d'action pour chaque client
3. Les deltas de configuration sont un tracker et un pack de télémétrie résume les demandes d'extraction et les demandes d'extraction.
4. Les artefacts de répétition/télémétrie sont en cours de préparation pour les mises à jour de la feuille de route et les mises à jour de la feuille de route. کریں، کھری ہوئی ad hoc نوٹس نہیں۔

## Index des preuves| اثاثہ | مقام | نوٹس |
|-------|----------|-------|
| Rapport d'audit de trace acheminée (T1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Preuve de phase B1 comme source canonique پورٹل پر `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` miroir noir |
| Suivi delta de configuration | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Résumés des différences TRACE-CONFIG-DELTA, réviseurs et initiales, et journal des votes GOV-2026-03-19 ici |
| Plan de remédiation de télémétrie | `docs/source/nexus_telemetry_remediation_plan.md` | pack d'alertes, taille du lot OTLP, B2 ou garde-corps budgétaires d'exportation |
| Tracker de répétition multi-voies | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | 9 artefacts de répétition, manifeste/résumé du validateur, notes/agenda du deuxième trimestre et preuves d'annulation |
| Manifeste/résumé du pack de télémétrie (dernier) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | plage d'emplacements 912-936, graine `NEXUS-REH-2026Q2` pour les bundles de gouvernance et les hachages d'artefacts |
| Manifeste de profil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Réexécution du deuxième trimestre pour capturer le hachage du profil TLS les annexes de trace acheminée citent کریں۔ |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Planification des répétitions du deuxième trimestre (plage de créneaux horaires, graine de charge de travail, propriétaires d'action) |
| Runbook de répétition de lancement | `docs/source/runbooks/nexus_multilane_rehearsal.md` | mise en scène -> exécution -> restauration et liste de contrôle topologie des voies et conseils pour l'exportateur |
| Validateur de pack télémétrie | `scripts/telemetry/validate_nexus_telemetry_pack.py` | B4 retro میں حوالہ دیا گیا CLIÉ pack de résumés et de tracker pour les lecteurs de journaux || Régression multivoie | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | configurations multi-voies comme `nexus.enabled = true` pour les hachages de catalogue Sora et `ConfigLaneRouter` comme pour la voie locale Les chemins Kura/merge-log (`blocks/lane_{id:03}_{slug}`) fournissent des résumés d'artefacts et des résumés d'artefacts. |