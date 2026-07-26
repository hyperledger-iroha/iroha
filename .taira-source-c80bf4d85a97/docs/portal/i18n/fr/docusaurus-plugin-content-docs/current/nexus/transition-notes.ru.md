---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-transition-notes
titre : Notes sur l'ancien Nexus
description : Zerkalo `docs/source/nexus_transition_notes.md`, охватывающее доказательства перехода Phase B, график аудита и митигации.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Notes du précédent Nexus

Ce journal décrit l'installation du projet **Phase B - Nexus Fondations de transition** pour la liste de contrôle de chaque étape. Nous avons ajouté des instructions d'étape dans `roadmap.md` et avons fourni des documents pour les projets B1-B4, dans notre domaine, sur la gouvernance, SRE et les SDK suivants. делиться единым источником истины.

## Область и cadence

- Effectuer des audits de trace acheminée et des garde-corps de télémétrie (B1/B2), pour le delta de configuration, la gouvernance améliorée (B3) et le suivi de la répétition de lancement multi-voies (B4).
- Заменяет временную note de cadence, которая была здесь раньше ; L'Audi Q1 2026 s'ouvrira sur `docs/source/nexus_routed_trace_audit_report_2026q1.md`, cette page montre un graphique et une restauration.
- Affichez les tableaux après l'opération de trace acheminée, la gouvernance ou la répétition de lancement. Lorsque les articles sont créés, vous pouvez accéder à un nouvel emplacement, les documents en aval (statut, tableaux de bord, portails SDK) peuvent être mis à jour pour établir un projet.

## Images documentaires (T1-T2 2026)| Potots | Доказательства | Propriétaire(s) | Statut | Première |
|------------|----------|--------------|--------|-------|
| **B1 - Audits de trace acheminée** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @télémétrie-opérations, @gouvernance | Завершено (T1 2026) | Les pharmacies à trois niveaux d'audition ; TLS a été fermé par `TRACE-CONFIG-DELTA` lors de la réexécution du deuxième trimestre. |
| **B2 - Remédiation télémétrique et garde-corps** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Завершено | Pack d'alertes, bot de comparaison politique et lot OTLP (journal `nexus.scheduler.headroom` + marge du panneau Grafana) postés ; открытых renonciation нет. |
| **B3 - Approbations delta de configuration** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Завершено | La décision GOV-2026-03-19 a été publiée ; подписанный bundle питает pack de télémétrie ниже. |
| **B4 - Répétition de lancement multi-voies** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Завершено (T2 2026) | La réexécution de Canary au deuxième trimestre a mis fin à la période TLS ; manifeste du validateur + `.sha256` fixe l'emplacement de connexion 912-936, graine de charge de travail `NEXUS-REH-2026Q2` et profil de hachage TLS lors de la réexécution. |

## Квартальный график route-trace audio| Identifiant de trace | Okno (UTC) | Résultat | Première |
|--------------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Proydéno | L'admission à la file d'attente P95 оставался значительно ниже цели <=750 ms. Je ne t'attends pas. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Proydéno | Les hachages de relecture OTLP sont utilisés avec `status.md` ; Le bot diff du SDK de parité prend en charge la dérive. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12h00-12h30 | Résolu | TLS a pris du retard lors de la réexécution du deuxième trimestre ; pack de télémétrie pour `NEXUS-REH-2026Q2` fixe le profil de hachage TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (avec `artifacts/nexus/tls_profile_rollout_2026q2/`) et seulement disponible. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Proydéno | Graine de charge de travail `NEXUS-REH-2026Q2` ; pack de télémétrie + manifeste/résumé dans `artifacts/nexus/rehearsals/2026q1/` (plage d'emplacements 912-936) et agenda dans `artifacts/nexus/rehearsals/2026q2/`. |

Les bureaux doivent pouvoir créer de nouveaux mouvements et améliorer les performances lors de l'utilisation, ainsi que la table des technologies. квартал. Recherchez cette section dans les résultats de trace acheminée ou dans les minutes de gouvernance à partir de l'article `#quarterly-routed-trace-audit-schedule`.

## Révision et backlog| Article | Description | Propriétaire | Цель | Statut / Première |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Assurez-vous de créer un profil TLS, qui est enregistré dans `TRACE-CONFIG-DELTA`, afin de réexécuter les preuves et d'effectuer des atténuations journalières. | @release-eng, @sre-core | Q2 2026 routé-trace OK | Fermeture - profil de hachage TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` attribué à `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` ; réexécuter подтвердил отсутствие отстающих. |
| `TRACE-MULTILANE-CANARY` préparation | Planifiez la répétition du deuxième trimestre, installez les appareils du pack de télémétrie et assurez-vous que les harnais du SDK fournissent une aide de validation. | @telemetry-ops, programme SDK | Planificateur 2026-04-30 | Завершено - agenda хранится в `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` avec emplacement de métadonnées/charge de travail ; réutiliser le harnais отмечен в tracker. |
| Rotation du résumé du pack de télémétrie | Téléchargez `scripts/telemetry/validate_nexus_telemetry_pack.py` avant la répétition/sortie et corrigez les résumés avec le delta de configuration du tracker. | @opérations de télémétrie | На каждый release candidate | Завершено - `telemetry_manifest.json` + `.sha256` выпущены в `artifacts/nexus/rehearsals/2026q1/` (plage d'emplacements `912-936`, graine `NEXUS-REH-2026Q2`) ; digère скопированы в tracker и индекс доказательств. |

## Intégration du bundle delta de configuration- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` остается каноническим résumé diff. Lorsque vous arrivez au nouveau `defaults/nexus/*.toml` ou à la création de Genesis, vous devez ouvrir le tracker pour ouvrir les touches disponibles.
- Les bundles de configuration incluent le pack de télémétrie de répétition. Pack, validé `scripts/telemetry/validate_nexus_telemetry_pack.py`, doit être publié avec les documents de configuration delta, que les opérateurs peuvent utiliser pour tous les articles d'art, utilisé en B4.
- Les bundles Iroha 2 sont situés en dehors des voies : les configurations avec `nexus.enabled = false` permettent d'ouvrir les remplacements de voie/espace de données/routage, sans activer le profil Nexus. (`--sora`), vous pouvez alors utiliser les sections `nexus.*` pour les pistes à voie unique.
- Vérifiez le journal de gouvernance (GOV-2026-03-19) avec votre tracker et cet outil pour que vous puissiez copier le formulaire de gouvernance. без повторного поиска ритуала одобрения.

## Suivis de la répétition de lancement- `docs/source/runbooks/nexus_multilane_rehearsal.md` fixe le plan Canary, la liste des utilisateurs et la restauration de la liste ; Mettez à jour le Runbook pour la définition des voies de topologie ou les télémètres d'exportation.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` met en œuvre tous les articles vérifiés lors de la répétition du 9 avril et inclut les notes de préparation/l'agenda du deuxième trimestre. Ajoutez des répétitions à votre tracker en utilisant le tracker d'odeurs, que vous recherchez par des monotones.
- Publier les extraits de collecteur OTLP et les exportations Grafana (avec `docs/source/telemetry.md`) pour l'exportateur de conseils de traitement par lots ; La mise à jour du premier trimestre a permis d'obtenir une taille de lot de 256 échantillons, afin de prévenir les alertes de marge.
- La recherche CI/tests multivoies se déroule dans `integration_tests/tests/nexus/multilane_pipeline.rs` et prend en charge le flux de travail `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), pour votre utilisation. ссылку `pytests/nexus/test_multilane_pipeline.py`; Ajoutez le hash `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) synchronisé avec le tracker pour la mise à jour des bundles de répétition.

## Cycle de vie des voies d'exécution- Les plans du cycle de vie de la voie d'exécution permettent de valider les liaisons d'espace de données et de préparer la réconciliation Kura/stockage hiérarchisé, en établissant un catalogue sans modification. Les assistants s'occupent des relais de voie pour les voies retirées, ce qui permet la synthèse des registres de fusion et ne fournit pas de preuves.
- Programmes d'aide à la configuration/cycle de vie Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) pour la création/démarrage des voies avant le redémarrage ; le routage, les instantanés TEU et les registres de manifestes sont automatiques selon le plan prévu.
- Opérateur de recherche : lors de votre plan, vérifiez les espaces de données d'origine ou les racines de stockage, qui ne sont pas disponibles (répertoires racine froide/Kura lane à plusieurs niveaux). Исправьте базовые пути и повторите; Les plans disponibles émettent actuellement une voie de diff/espace de données de télémétrie, ainsi que des tableaux de bord offrant une nouvelle topologie.

## NPoS телеметрия и доказательства contre-pression

La répétition de lancement de la phase B rétro a permis de déterminer les captures télémétriques, de détecter le stimulateur cardiaque NPoS et les potins qui se sont produits lors de la contre-pression précédente. L'intégration du faisceau dans `integration_tests/tests/sumeragi_npos_performance.rs` permet de créer ces scénarios et d'afficher des résumés JSON (`sumeragi_baseline_summary::<scenario>::...`) pour la mise en œuvre de nouvelles mesures. Localisation locale :

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Installez `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` ou `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` pour résoudre toutes les topologies de contrainte ; значения по умолчанию отражают профиль 1 s/`k=3`, utilisé dans B4.| Scénario / test | Achat | Clés de télémétrie |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Il bloque 12 tours pendant le temps de bloc de répétition, ce qui permet d'économiser les enveloppes de latence EMA, les jauges d'envoi et les jauges d'envoi redondant avant la mise en série des preuves. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Il faut veiller à ce que les transferts garantissent la détermination des reports d'admission et de la capacité/saturation des exportations. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Pour utiliser la gigue du stimulateur cardiaque et afficher les délais d'attente, vous ne pouvez pas utiliser de valeurs de +/-125 pour mille. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Les charges utiles RBC sont enregistrées dans le magasin de limites logicielles/dures, permettant de localiser les fichiers, de récupérer et de stabiliser les sessions/octets avant la mise en magasin. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Pour les retours, les jauges du taux d'envoi redondant et les compteurs des collecteurs sur la cible sont fournis, en fournissant une analyse télémétrique de bout en bout. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | En déterminant la suppression de morceaux, ces backlog surveillent les défauts de certaines charges utiles. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |Utilisez la ligne JSON pour créer un harnais, avec le scrape Prometheus, pour une utilisation en cours, dans le cadre de la gouvernance du projet, quelle est la contre-pression alarmes сооответствуют répétition топологии.

## Чеклист обновления

1. Créez une nouvelle trace de routage et enregistrez les étoiles dans quelques zones.
2. Ouvrez le tableau d'atténuation après le suivi d'Alertmanager, afin de créer un ticket.
3. Une fois le menu deltas de configuration activé, activez le tracker, puis indiquez et digérez le pack de télémétrie dans une demande d'extraction classique.
4. Étudiez de nouveaux objets de répétition/télémétrie, pour créer une feuille de route élaborée dans un document élaboré, qui n'est pas défini ad hoc. заметки.

## Indices des documents| Actifs | Localisation | Première |
|-------|----------|-------|
| Rapport d'audit de trace acheminée (T1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Канонический источник доказательств Phase B1 ; зеркалируется в портал через `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Suivi delta de configuration | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Consultez les résumés de différences TRACE-CONFIG-DELTA, les réviseurs originaux et le journal GOV-2026-03-19. |
| Plan de remédiation de télémétrie | `docs/source/nexus_telemetry_remediation_plan.md` | Documentez le pack d'alertes, la taille du lot OTLP et les garde-fous du budget d'exportation, en relation avec B2. |
| Tracker de répétition multi-voies | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Liste des articles fabriqués le 9 avril, manifeste/résumé du validateur, notes/agenda du deuxième trimestre et preuves d'annulation. |
| Manifeste/résumé du pack de télémétrie (dernier) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Plage d'emplacements de correctif 912-936, graine `NEXUS-REH-2026Q2` et ensembles de gouvernance d'artéfacts de hachage. |
| Manifeste de profil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash a intégré le profil TLS, effectué lors de la réexécution du deuxième trimestre ; ссылайтесь в routé-trace annexes. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Plans pour la répétition du deuxième trimestre (fenêtre, plage de créneaux, graine de charge de travail, propriétaires d'action). |
| Runbook de répétition de lancement | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Операционный чеклист mise en scène -> exécution -> restauration ; обновлять при изменении топологии voies ou orientation exportateurs. || Validateur de pack télémétrie | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI, utilisé dans le rétro B4 ; ARCHивируйте digests вместе с tracker при любом изменении pack. |
| Régression multivoie | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | En vérifiant `nexus.enabled = true` pour les configurations multi-voies, en prenant en compte les hachages du catalogue Sora et en fournissant les chemins Kura/merge-log locaux de voie (`blocks/lane_{id:03}_{slug}`) depuis `ConfigLaneRouter` avant публикацией résumés d'artefacts. |