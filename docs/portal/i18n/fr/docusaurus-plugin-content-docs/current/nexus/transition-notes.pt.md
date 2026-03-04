---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/transition-notes.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-transition-notes
titre : Notes de transition du Nexus
description : Espelho de `docs/source/nexus_transition_notes.md`, cobrindo évidence de transition da Phase B, le calendrier des auditoires et comme atténuations.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Notes de transition du Nexus

Cet enregistrement accompagne le travail pendant la **Phase B - Nexus Transition Foundations** et est complété par une liste de contrôle de lancement à plusieurs voies. Il complète les entrées de jalons dans `roadmap.md` et met en évidence les preuves référencées par B1-B4 dans un endroit unique pour la gouvernance, SRE et les dirigeants du SDK partagés à même la source de vérité.

## Escopo et cadence

- Cobre que les auditoires acheminés et les garde-corps de télémétrie (B1/B2), ou le ensemble de deltas de configuration approuvés par le gouvernement (B3) et les accompagnements de l'enceinte de lancement multi-voies (B4).
- Substitui a nota temporaria de cadencia que antes vivia aqui; À partir de l'auditoire du premier trimestre 2026, le rapport détaillé réside dans `docs/source/nexus_routed_trace_audit_report_2026q1.md`, alors que cette page contient le calendrier actuel et le registre des mesures à prendre.
- Actualisez comme tabelas apos cada janela routed-trace, voto de gouvernance ou ensaio de lancamento. Lorsque les objets sont déplacés, renvoiez-les à une nouvelle localisation dans cette page pour que les documents postérieurs (statut, tableaux de bord, port SDK) puissent créer un lien vers un emplacement sécurisé.

## Aperçu des preuves (T1-T2 2026)| Flux de travail | Preuve | Propriétaire(s) | Statut | Notes |
|------------|----------|--------------|--------|-------|
| **B1 - Audits de trace acheminée** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @télémétrie-opérations, @gouvernance | Complet (T1 2026) | Tres Janelas de auditoria registradas; Le TLS de `TRACE-CONFIG-DELTA` a été récupéré au cours de la rediffusion du deuxième trimestre. |
| **B2 - Réparation de télémétrie et garde-corps** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Complet | Pack d'alertes, politique du bot de différence et tamanho de lote OTLP (journal `nexus.scheduler.headroom` + tableau Grafana de marge) envoyés ; sem renonciations em aberto. |
| **B3 - Approbations des deltas de configuration** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Complet | Vote GOV-2026-03-19 enregistré; le paquet d'aliments consommés ou le paquet de télémétrie cité abaixo. |
| **B4 - Ensaio de lancamento multi-voies** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Complet (T2 2026) | Ou réexécutez Canary de Q2 pour atténuer l'impact de TLS ; Le manifeste du validateur + `.sha256` capture l'intervalle des emplacements 912-936, la graine de charge de travail `NEXUS-REH-2026Q2` et le hachage du profil TLS enregistré sans réexécution. |

## Calendrier trimestriel des auditoires routé-trace| Identifiant de trace | Janela (UTC) | Résultat | Notes |
|--------------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Approuvé | L'admission à la file d'attente P95 est ficou bem abaixo do alvo <=750 ms. Nenhuma cacao nécessaire. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Approuvé | La relecture OTLP hache les informations sur `status.md` ; La parité du robot de comparaison SDK confirme la dérive zéro. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12h00-12h30 | Résolu | L'atraso TLS a été diffusé lors de la rediffusion du T2 ; Le pack de télémétrie pour `NEXUS-REH-2026Q2` enregistre le hachage du profil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (vers `artifacts/nexus/tls_profile_rollout_2026q2/`) et zéro perte. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Approuvé | Graine de charge de travail `NEXUS-REH-2026Q2` ; pack de télémétrie + manifeste/résumé sur `artifacts/nexus/rehearsals/2026q1/` (plage d'emplacements 912-936) avec agenda sur `artifacts/nexus/rehearsals/2026q2/`. |

Le trimestre futur doit ajouter de nouvelles lignes et déménager comme entrées conclues pour une annexe lorsque le tableau continue de croître pendant le trimestre actuel. La référence est cette section à partir des relations routed-trace ou des données de gouvernance en utilisant l'ancre `#quarterly-routed-trace-audit-schedule`.

## Atténuations et éléments de backlog| Article | Description | Propriétaire | Alvo | Statut / Notes |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Finalisez la propagation du profil TLS qui a été trasado pendant `TRACE-CONFIG-DELTA`, capturez les preuves de la réexécution et récupérez le registre d'atténuation. | @release-eng, @sre-core | Janela acheminé-trace du T2 2026 | Fechado - hash do profile TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` capturé dans `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` ; ou réexécutez la confirmation que nao ha atrasados. |
| `TRACE-MULTILANE-CANARY` préparation | Programmer l'essai Q2, ajouter des accessoires au pack de télémétrie et garantir que les exploits du SDK sont réutilisés ou validés par l'assistant. | @telemetry-ops, programme SDK | Chamada de avion 2026-04-30 | Complet - l'agenda est armazenada em `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` avec les métadonnées de slot/workload ; réutilizacao faire exploiter anotada pas de tracker. |
| Rotation du résumé du pack de télémétrie | Exécuter `scripts/telemetry/validate_nexus_telemetry_pack.py` avant chaque enregistrement/version et les résumés du registraire à côté du tracker de config delta. | @opérations de télémétrie | Por release candidate | Complet - `telemetry_manifest.json` + `.sha256` émis dans `artifacts/nexus/rehearsals/2026q1/` (plage d'emplacements `912-936`, graine `NEXUS-REH-2026Q2`) ; digère les copies sans tracker ni indice de preuve. |

## Intégration du bundle de configuration delta- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` suit le résumé canonique des différences. Quand vous achetez un nouveau `defaults/nexus/*.toml` ou changez de genèse, actualisez ce tracker premier et après avoir refait vos tâches ici.
- Les bundles de configuration signés alimentent le pack de télémétrie d'ensaio. Le pack, validé par `scripts/telemetry/validate_nexus_telemetry_pack.py`, doit être publié conjointement avec une preuve de différence de configuration pour que les opérateurs puissent reproduire les artéfacts utilisés pendant la période B4.
- Les bundles de Iroha 2 permanents avec des voies : les configurations avec `nexus.enabled = false` avant de supprimer les remplacements de voie/espace de données/routage à moins que le profil Nexus soit autorisé (`--sora`), puis supprimé au fur et à mesure `nexus.*` das modèles à voie unique.
- Mantenha o log de voto de gouvernance (GOV-2026-03-19) lié tanto no tracker quanto nesta nota para que futurs votos possam copier o formato sem redescobrir o rituel de aprovacao.

## Accompanhamentos do ensaio de lancamento- `docs/source/runbooks/nexus_multilane_rehearsal.md` capture le plan Canary, la liste des participants et les étapes de restauration ; actualisez le runbook lorsque la topologie des voies ou les exportateurs de télémétrie changent.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` lista cada artefato checado durante o ensaio du 9 avril e agora inclui notas/agenda de preparacao Q2. Ajoutez des prévisions futures à mon tracker pour ouvrir des trackers isolés afin de maintenir une preuve monotone.
- Les extraits publics du gestionnaire OTLP et des exportations font Grafana (version `docs/source/telemetry.md`) lorsque l'orientation du traitement par lots de l'exportateur change ; L'actualisation du premier trimestre a élevé la taille du lot à 256 heures pour éviter les alertes de marge.
- Une preuve de CI/tests multi-voies agora vive em `integration_tests/tests/nexus/multilane_pipeline.rs` et roda sob o workflow `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), en remplacement de la référence posée `pytests/nexus/test_multilane_pipeline.py` ; Le hachage de `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) doit être synchronisé avec le tracker pour actualiser les bundles d'enregistrement.

## Cycle de vie des voies dans l'exécution- Les plans de cycle de vie des voies dans l'exécution avant de valider les liaisons de l'espace de données et de les abandonner lors de la réconciliation du Kura/armement dans les chambres fausses, en gardant le catalogue modifié. Les assistants peuvent relayer les voies dans le cache pour les voies posées, afin que le grand livre de fusion sintés puisse réutiliser les preuves obsolètes.
- Appliquer les plans pour les aides de configuration/cycle de vie de Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) pour ajouter/retirer les voies sans problème ; le routage, les instantanés TEU et les registres de manifestes sont automatiquement récupérés sur un plan bien réussi.
- Orientation pour les opérateurs : lorsqu'un plan est incorrect, vérifiez que les espaces de données ausentes ou les racines de stockage ne peuvent pas être créées (racine froide à plusieurs niveaux/directions Kura por lane). Corrija os caminhos base e tente Novamente; les plans bem-sucedidos réémettent la différence de télémétrie de voie/espace de données pour que les tableaux de bord reflètent une nouvelle topologie.

## Telemetria NPoS et preuves de contre-pression

Le rétro-essai de lancement de la phase B a permis des captures de télémétrie déterministes qui prouvent que le stimulateur cardiaque NPoS et des caméras de potins permanentes dans ses limites de contre-pression. L'exploitation de l'intégration dans `integration_tests/tests/sumeragi_npos_performance.rs` exerce ces scénarios et émet des résumés JSON (`sumeragi_baseline_summary::<scenario>::...`) lorsque de nouvelles mesures s'effectuent. Exécutez localement com :

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```Définir `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` ou `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` pour explorer les topologies de principale activité ; Les valeurs doivent refléter le profil des couleurs 1 s/`k=3` utilisé en B4.| Scénario / test | Couverture | Télémétrie chave |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloqueia 12 rodadas com o block time do ensaio para registrar enveloppes de latence EMA, profondeurs de fil et jauges d'envoi redondant avant de sérialiser ou paquet de preuves. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Il s'agit d'un fil de transport pour garantir que les reports d'admission soient activés de manière déterministe et que le fil exporte des contadores de capacité/saturation. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | La gigue du stimulateur cardiaque et les délais d'attente de vue ont été prouvés par une bande de +/-125 pour mille et appliquée. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Empurra payloads RBC a atteint les limites soft/hard du store pour montrer que sesssoes et contadores de bytes sobem, recuam e estabilizam sem ultrapassar o store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Forcez les retransmissions pour que les jauges de rapport d'envoi redondant et les contadores de collecteurs sur cible avancent, à condition que la télémétrie effectuée par la rétro soit connectée de bout en bout. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. || `npos_rbc_chunk_loss_fault_reports_backlog` | Téléchargez des morceaux à intervalles déterminés pour vérifier que les moniteurs de backlog falhas en temps d'exécution silencieux des charges utiles. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Anexe as linhas JSON que le harnais imprime conjointement avec le scrape do Prometheus capturé lors de l'exécution semper que le gouvernement demande des preuves que les alarmes de contre-pression correspondent à la topologie de l'essai.

## Checklist de mise à jour

1. Adicione novas janelas routed-trace et retrait as antigas quando os trimestres girarem.
2. Créez un tableau d'atténuation après chaque accompagnement d'Alertmanager, même si cela demande un ticket.
3. Lorsque vous configurez les différences, actualisez le tracker, cette note et la liste des résumés du pack de télémétrie ne nécessitent aucune demande d'extraction.
4. Lien ici avec tout nouvel artéfact d'analyse/télémétrie pour que les futures actualisations de la feuille de route puissent référencer un document unique sous la forme de notes ad hoc dispersées.

## Indice de preuve| Ativo | Localisation | Notes |
|-------|----------|-------|
| Rapport d'audience routé-trace (T1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Fonte canonique des preuves de la Phase B1 ; Espelhado para o portail em `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker de configuration delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Considérez les résumés des différences TRACE-CONFIG-DELTA, les premiers réviseurs et le journal de vote GOV-2026-03-19. |
| Plan de réparation de télémétrie | `docs/source/nexus_telemetry_remediation_plan.md` | Documentation du pack d'alerte, du tamanho de lote OTLP et des garde-corps d'orcamento de exportacao vinculados a B2. |
| Tracker d'ensaio multi-voies | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Liste des articles de l'enregistrement du 9 avril, manifeste/résumé du validateur, notes/agenda Q2 et preuves de restauration. |
| Manifeste/résumé du pack de télémétrie (dernier) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Enregistrez la plage d'emplacements 912-936, la graine `NEXUS-REH-2026Q2` et les hachages d'artefatos pour les bundles de gouvernance. |
| Manifeste de profil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Le hachage du profil TLS approuvé a été capturé lors de la réexécution du deuxième trimestre ; citer les annexes routed-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notes de planification pour l'enregistrement Q2 (janela, plage de slots, graine de charge de travail, propriétaires d'acoes). |
| Runbook d'essai de lancement | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Liste de contrôle opérationnelle pour la mise en scène -> exécution -> restauration ; actualiser quand la topologie des voies ou l'orientation des exportateurs changent. || Validateur de pack télémétrie | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI référencé par le rétro B4 ; archiver les résumés ao lado do tracker sempre que o pack mudar. |
| Régression multivoie | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Prova `nexus.enabled = true` pour les configurations multi-voies, préserve les hachages du catalogue Sora et fournit des chemins Kura/merge-log par voie (`blocks/lane_{id:03}_{slug}`) via `ConfigLaneRouter` avant de publier les résumés des œuvres. |