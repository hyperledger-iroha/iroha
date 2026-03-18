---
lang: he
direction: rtl
source: docs/portal/docs/nexus/transition-notes.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-transition-notes
כותרת: Notes de transition de Nexus
תיאור: Miroir de `docs/source/nexus_transition_notes.md`, couvrant les preuves de transition Phase B, le calendrier d'audit et les mitigations.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# הערות מעבר דה Nexus

Ce journal suit le travail restant de **Phase B - Nexus Transition Foundations** jusqu'a la fin de la checklist de lancement multi-lane. אני מלא את המנות הראשונות של אבני דרך ב-`roadmap.md` et garde les preuves referencees par B1-B4 en un seul endroit afin que la governance, SRE et les leads SDK partagent la meme source de verite.

## Portee et cadence

- Couvre les audits routed-trace et les guards de telemetrie (B1/B2), l'ensemble de deltas de configuration approuve par la governance (B3) et les suivis de repetition de lancement multi-lane (B4).
- Remplace la note temporaire de cadence qui vivait ici; depuis l'audit Q1 2026, le rapport detaille reside dans `docs/source/nexus_routed_trace_audit_report_2026q1.md`, tandis que cette page maintient le calendrier courant et le registre des mitigations.
- Mettez a jour les tables apres chaque fenetre מנותב-עקבות, הצבעה על שלטון או חזרתיות. Lorsque des Artefacts bougent, refletez le nouvel emplacement dans cette page pour que les docs aval (סטטוס, לוחות מחוונים, פורטלים SDK) puissent lier un ancrage stable.

## תמונת Snapshot des preuves (2026 Q1-Q2)

| זרם עבודה | Preuves | בעלים | סטטוט | הערות |
|------------|--------|--------|--------|------|
| **B1 - ביקורת מעקב מנותב** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @ממשל | השלם (Q1 2026) | Trois fenetres d'audit registrees; le retard TLS de `TRACE-CONFIG-DELTA` s'est clos תליון שידור חוזר Q2. |
| **B2 - Remediation de telemetrie et מעקות בטיחות** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | השלם | חבילת התראה, פוליטיקה שונה וטיפה OTLP (יומן `nexus.scheduler.headroom` + פאנל Grafana מרווח ראש) ספרים; ויתור על אוברט. |
| **B3 - אישורי הגדרות תצורה** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | השלם | הצביעו GOV-2026-03-19 להירשם; le bundle signe alimente le pack de telemetrie cite plus bas. |
| **B4 - Repetition de lancement multi-lane** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | השלם (Q2 2026) | Le rerun canary de Q2 a ferme la mitigation du retard TLS; מניפסט המאמת + `.sha256` לכידת חריצים 912-936, תחילת עומס עבודה `NEXUS-REH-2026Q2`, ו-Hash du profil TLS רישום תליון להרצה חוזרת. |

## Calendrier trimestriel des audits routed-trace| מזהה מעקב | פנטר (UTC) | תוצאות | הערות |
|--------|--------------|--------|------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | רוסי | כניסה לתור P95 est reste bien en dessous de la cible <=750 ms. דרישה לפעולה Aucune. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | רוסי | OTLP replay hashes מצרף `status.md`; la parite du diff bot SDK לאשר אפס סחיפה. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | רזולו | Le retard TLS s'est ferme durant שידור חוזר Q2; le pack de telemetrie pour `NEXUS-REH-2026Q2` לרשום את ה-hash du profil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (לפי `artifacts/nexus/tls_profile_rollout_2026q2/`) ואפס מעכבים. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | רוסי | seed עומס עבודה `NEXUS-REH-2026Q2`; pack de telemetrie + manifest/digest dans `artifacts/nexus/rehearsals/2026q1/` (טווח חריצים 912-936) avec agenda dans `artifacts/nexus/rehearsals/2026q2/`. |

Les trimestres futurs doivent ajouter de nouvelles lignes et deplacer les entrees terminees vers une annexe quand la table depasse le trimestre courant. Referencez cette סעיף depuis les rapports מנותב-trace ou les minutes de gouvernance en utilisant l'ancre `#quarterly-routed-trace-audit-schedule`.

## הקלות ופריטים בצבר

| פריט | תיאור | בעלים | סיבל | תקנון / הערות |
|------|-------------|--------|--------|----------------|
| `NEXUS-421` | תאריכים להפצת פרופיל TLS qui a pris du retard תליון `TRACE-CONFIG-DELTA`, לוכד les preuves du rerun and fermer le registre de mitigation. | @release-eng, @sre-core | Fentre routed-trace Q2 2026 | Clos - hash du profil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` לכידה dans `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; להפעיל מחדש אפס מעכבים. |
| `TRACE-MULTILANE-CANARY` הכנה | מתכנת לחזרה על שאלה 2, מצטרפת לתקנים או לחבילת טלמטריה ומבטיחה שה-SDK רותמת מחדש את העוזר התקף. | @telemetry-ops, תוכנית SDK | אפל דה תכנון 2026-04-30 | השלם - סדר יום סטוק dans `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` avec חריץ מטא נתונים/עומס עבודה; ניצול מחדש של הרתם note dans le tracker. |
| סיבוב ערכת טלמטריה | המבצע `scripts/telemetry/validate_nexus_telemetry_pack.py` חזרתי/שחרור אופנתית של צ'אק ורשום מתעדכן את דלתא המעקב אחר התצורה. | @telemetry-ops | מועמד לשחרור נקוב | קומפלט - `telemetry_manifest.json` + `.sha256` emis dans `artifacts/nexus/rehearsals/2026q1/` (טווח חריצים `912-936`, זרע `NEXUS-REH-2026Q2`); מעכל עותקים dans le tracker et l'index des preuves. |

## Integration du bundle de config delta- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` reste le resume canonique des diffs. Quand de nouveaux `defaults/nexus/*.toml` או השינויים בראשית בוא, mettez a jour ce tracker d'abord puis refletez les points cle ici.
- חבילות תצורה חתומות לחבילת טלמטריה חוזרת. Le pack, valide par `scripts/telemetry/validate_nexus_telemetry_pack.py`, doit etre publie avec les preuves de config delta afin que les operators puissent rejouer les artefacts exacts משתמשת בתליון B4.
- Les bundles Iroha 2 restent sans מסלולים: les configs avec `nexus.enabled = false` תחזוקה דחופה les overrides de lane/dataspace/routing sauf si le profil Nexus est active (Prometheus) `nexus.*` des templates חד נתיב.
- Gardez le log de vote de gouvernance (GOV-2026-03-19) lie depuis le tracker et cette note pour que les futurs votes puissent copier le format sans devoir re-decouvrir le rituel d'appropation.

## Suivis de repetition de lancement

- `docs/source/runbooks/nexus_multilane_rehearsal.md` ללכוד את תוכנית הקנרית, לה רשימה של המשתתפים ו-ההחזרה לאחור; mettez a jour le runbook quand la topology des lanes או les exporters de telemetrie changent.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` רשימת חפצי חפץ ודא את משך החזרה ב-9 באפריל ותחזוק תוכן להערות/סדר יום ההכנה Q2. Ajoutez les repetitions futures au meme tracker au lieu d'ouvrir des trackers ad-hoc pour garder les preuves מונוטונים.
- Publiez des snippets du collecteur OTLP et des exports Grafana (voir `docs/source/telemetry.md`) quand les consignes de batching de l'exporter changent; la mise a jour Q1 a porte la taille de lot a 256 echantillons pour eviter des alertes מרווח ראש.
- Les preuves CI/tests multi-lane vivent Maintenant dans `integration_tests/tests/nexus/multilane_pipeline.rs` et tournent sos le workflow `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), remplacant la Reference Pension `pytests/nexus/test_multilane_pipeline.py`; gardez le hash pour `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) en sync avec le tracker lors du rafraichissement des bundles de repetition.

## מחזור זמן ריצה של הנתיבים

- Les plans de cycle de vie des lanes עם תחזוקה תקף בזמן ריצה, les bindings de dataspace ו-abortent quand la reconciliation Kura/stockage en paliers echoue, laissant le catalog inchange. Les helpers eliminent les relays caches pour les lanes retirees afin que la synthese-mherge-ledger ne reutilise pas des proofs obsoletes.
- Appliquez les plans via les helpers de config/lifecycle Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) pour ajouter/retirer des lanes sans redemarrage; ניתוב, תצלומי Snapshot TEU ורשומות מניפסטים נטען אוטומטית לאחר תוכנית חוזרת.
- מפעיל מדריך: בדיקת תוכנית הדדית, אימות מרחבי נתונים או שורשי אחסון בלתי אפשריים (שורש קר מדורג/רפרטואר Kura par lane). Corrigez les chemins de base et reessayez; les plans reussis re-emettent le diff de telemetrie lane/dataspace pour que les dashboards refletent la nouvelle topology.

## Telemetrie NPoS ו-preuves de backpressureRetro de repetition de lancement שלב ב' ודרישת לכידת טלמטריה קביעת פרובנט que le קוצב NPoS ו-les couches de restent רכילות ב-Leurs limites de backpressure. הרתמה של אינטגרציה ב-`integration_tests/tests/sumeragi_npos_performance.rs` תרגילי תרחישים ותרחישים של JSON (`sumeragi_baseline_summary::<scenario>::...`) עם מדדים חדשים שהגיעו. Lancez-le localement avec:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Definissez `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` או `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` pour explorer des topology plus stressees; les valeurs par defaut refletent le profil de collecteurs 1 s/`k=3` use en B4.

| תרחיש / מבחן | קוברטור | טלמטרי cle |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloque 12 rounds with le block time de repetition pour enregistrer les enveloppes de latence EMA, les profondeurs de file et les gauges de overundant-send avant de serialiser le bundle de preuves. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Inonde la file de transaktions pour s'assurer que les deferrals d'admission s'activent de facon deterministe et que la file exporte les compteurs de capacite/saturation. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Echantillonne le jitter du pacemaker et les timeouts de vue jusqu'a prouver que la bande +/-125 permille est appliquee. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Pousse de gros payloads RBC jusqu'aux limites soft/hard du store pour montrer que les sessions and compteurs de bytes montent, reculent et se stabilisent sans depasser le store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Force des retransmissions pour que les gauges de ratio redundant-send and les compteurs de collectors-on-target avancent, prouvant que la telemetrie demandee par le retro est branchee מקצה לקצה. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Laisse tomber des chunks a intervalles deterministes pour verifier que les moniteurs de backlog signalent des fautes au lieu de drainer silencieusement les payloads. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Joignez les lignes JSON imprimees par le harness avec le scrape Prometheus תליון לכידה לביצוע chaque fois que la governance demande des preuves que les alarmes depressure correspondent a la topology de repetition.

## רשימת רשימת חיפוש

1. Ajoutez de nouvelles fenetres routed-trace et retirez les anciennes lorsque les trimestres tournent.
2. Mettez a jour la table de mitigation apres chaque suivi Alertmanager, meme si l'action מורכבת מכרטיס פרמר.
3. Quand les config deltas changent, mettez a jour le tracker, cette note and la list de digests du telemetry pack in la meme pull request.
.

## Index des preuves| אקטיב | מיקום | הערות |
|-------|----------------|-------|
| Rapport d'audit routed-trace (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | מקור canonique pour les preuves שלב B1; miroir pour le portail sous `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker de config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | מכילים קורות חיים שונים TRACE-CONFIG-DELTA, ראשי תיבות של סוקרים ויומן ההצבעה GOV-2026-03-19. |
| Plan de remediation de telemetrie | `docs/source/nexus_telemetry_remediation_plan.md` | חבילת ההתראה התיעודית, להלן OTLP ומעקות הבטיחות של תקציב היצוא נמצא ב-B2. |
| Tracker de repetition multi-lane | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | רשימה של חפצי אומנות מאמתת את המשך החזרה ב-9 באפריל, אימות מניפסט/עיכול, הערות/סדר יום Q2 ו-preuves de rollback. |
| מניפסט/תקציר של חבילת טלמטריה (האחרונה ביותר) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Enregistre la plage 912-936, seed `NEXUS-REH-2026Q2` et les hashes d'artefacts pour les bundles de governance. |
| מניפסט פרופיל TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash du profil TLS מאשר תליון לכידה להרצה חוזרת Q2; citez-le dans les נספחים מנותב-עקבות. |
| סדר היום TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | הערות תכנון לשאלה 2 (חדר, טווח משבצות, זרע עומס עבודה, פעולות בעלים). |
| Runbook de repetition de lancement | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Checklist operationnelle pour staging -> ביצוע -> חזרה לאחור; mettre a jour quand la topologie des lanes ou les conseils d'exporters changent. |
| אימות ערכת טלמטריה | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI reference par le retro B4; archivez les digests avec le tracker chaque fois que le pack change. |
| רגרסיה רב מסלולית | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Valide `nexus.enabled = true` pour les configs multi-lane, keep les hashes du catalog Sora et provisionne les chemins Kura/merge-log par lane (`blocks/lane_{id:03}_{slug}`) via `ConfigLaneRouter` avant de publier les digests d'artef. |