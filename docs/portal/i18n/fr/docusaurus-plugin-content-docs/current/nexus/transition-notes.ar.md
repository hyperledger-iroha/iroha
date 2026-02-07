---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-transition-notes
titre : ملاحظات انتقال Nexus
description : Le test est pour `docs/source/nexus_transition_notes.md`, il s'agit de la phase B et de la phase B.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ملاحظات انتقال Nexus

يتتبع هذا السجل العمل المتبقي من **Phase B - Nexus Fondations de transition** حتى تكتمل قائمة فحص اطلاق الـ multi-voies. Il s'agit d'une application pour `roadmap.md` et d'une application pour B1-B4 pour votre projet. Le SDK et le SRE sont également compatibles avec la fonctionnalité.

## النطاق والوتيرة

- Il existe des garde-corps à trace acheminée et des garde-corps pour (B1/B2) et des deltas pour les systèmes de sécurité et de sécurité (B3). تدريب الاطلاق متعدد voies (B4).
- يستبدل ملاحظة الوتيرة المؤقتة التي كانت هنا؛ منذ تدقيق Q1 2026 يوجد الصفحة بالجدول التشغيلي وسجل التخفيفات.
- حدث الجداول بعد كل نافذة routed-trace او تصويت حوكمة او تدريب اطلاق. Les artefacts sont désormais disponibles en ligne (statut, tableaux de bord, applications SDK) الارتباط بمرساة ثابتة.

## لقطة ادلة (2026 T1-T2)| مسار العمل | الادلة | الملاك | الحالة | ملاحظات |
|------------|----------|--------------|--------|-------|
| **B1 - Audits de trace acheminée** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @télémétrie-opérations, @gouvernance | مكتمل (T1 2026) | تم تسجيل ثلاث نوافذ تدقيق؛ Vous avez utilisé TLS pour `TRACE-CONFIG-DELTA` pour réexécuter Q2. |
| **B2 - Remédiation télémétrique et garde-corps** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | مكتمل | Vous avez besoin du pack d'alertes et du bot diff pour OTLP (journal `nexus.scheduler.headroom` + marge de sécurité pour Grafana) لا توجد renonciations مفتوحة. |
| **B3 - Approbations delta de configuration** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | مكتمل | تم تسجيل تصويت GOV-2026-03-19؛ الحزمة الموقعة تغذي pack de télémétrie المذكور ادناه. |
| **B4 - Répétition de lancement multi-voies** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | مكتمل (T2 2026) | Je vais rediffuser la vidéo au deuxième trimestre avec TLS Il s'agit du manifeste du validateur + `.sha256` pour les emplacements 912-936 et de la graine de charge de travail `NEXUS-REH-2026Q2` et du hachage pour la réexécution de TLS. |

## جدول تدقيق routed-trace ربع السنوي| Identifiant de trace | النافذة (UTC) | النتيجة | ملاحظات |
|--------------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | ناجح | ظل Queue-admission P95 اقل بكثير من الهدف <=750 ms. لا يلزم اي اجراء. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | ناجح | J'utilise les hachages de relecture OTLP par `status.md` ; La parité est utilisée avec le bot diff SDK et la dérive. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12h00-12h30 | تم الحل | Vous avez utilisé TLS pour réexécuter le deuxième trimestre Il s'agit du pack de télémétrie pour le hachage `NEXUS-REH-2026Q2` et du TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (`artifacts/nexus/tls_profile_rollout_2026q2/`) et des fonctionnalités supplémentaires. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | ناجح | Graine de charge de travail `NEXUS-REH-2026Q2` ; pack de télémétrie + manifeste/résumé sous `artifacts/nexus/rehearsals/2026q1/` (plage d'emplacements 912-936) et agenda sous `artifacts/nexus/rehearsals/2026q2/`. |

يجب على الفصول القادمة اضافة صفوف جديدة ونقل الادخالات المكتملة الى ملحق عندما يتجاوز الجدول الربع الحالي. Il s'agit d'une méthode de routed-trace et d'une méthode de traçage `#quarterly-routed-trace-audit-schedule`.

## التخفيفات وعناصر backlog| العنصر | الوصف | المالك | الهدف | الحالة / الملاحظات |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Vous pouvez utiliser TLS pour créer une nouvelle exécution `TRACE-CONFIG-DELTA`, ainsi que pour une réexécution. | @release-eng, @sre-core | Lire routed-trace Q2 2026 | مغلق - hash ملف TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ملتقط في `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; اكد rediffusion عدم وجود متخلفات. |
| `TRACE-MULTILANE-CANARY` préparation | Pour le Q2, les luminaires sont utilisés pour les exploits du SDK et pour les exploits du SDK. | @telemetry-ops, programme SDK | اجتماع التخطيط 2026-04-30 | مكتمل - agenda محفوظة في `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` مع emplacement de métadonnées/charge de travail؛ Il s'agit d'un harnais pour tracker. |
| Rotation du résumé du pack de télémétrie | Téléchargez `scripts/telemetry/validate_nexus_telemetry_pack.py` pour la version/Release et les résumés du tracker pour le delta de configuration. | @opérations de télémétrie | لكل version candidate | مكتمل - `telemetry_manifest.json` + `.sha256` sont associés à `artifacts/nexus/rehearsals/2026q1/` (plage d'emplacements `912-936`, graine `NEXUS-REH-2026Q2`) ; تم نسخ digests الى tracker وفهرس الادلة. |

## دمج حزمة delta de configuration- يظل `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` الملخص القانوني للفروقات. Il s'agit d'un `defaults/nexus/*.toml` qui est en train de créer Genesis, un tracker et un outil de suivi.
- Les bundles de configuration signés sont également disponibles. يجب نشر الحزمة، التي تم التحقق منها عبر `scripts/telemetry/validate_nexus_telemetry_pack.py`, بجانب ادلة config delta لكي يتمكن المشغلون من اعادة تشغيل Voir les artefacts en B4.
- Pour Iroha 2 voies : configurations pour `nexus.enabled = false`, les remplacements de voie/espace de données/routage sont pour Nexus. (`--sora`) ، لذا ازل اقسام `nexus.*` من قوالب voie unique.
- ابق سجل تصويت الحوكمة (GOV-2026-03-19) مرتبطا من tracker ومن هذه الملاحظة لكي تتمكن التصويتات المستقبلية من نسخ التنسيق دون اعادة اكتشاف طقوس الموافقة.

## متابعات تدريب الاطلاق- يسجل `docs/source/runbooks/nexus_multilane_rehearsal.md` خطة الكاناري وقائمة المشاركين وخطوات rollback؛ Le runbook contient également des voies et des exportateurs.
- يسرد `docs/source/project_tracker/nexus_rehearsal_2026q1.md` كل artefact تم التحقق منه في تدريب 9 ابريل ويشمل الان ملاحظات/agenda تحضير Q2. Il s'agit d'un tracker ou d'un tracker plus proche.
- Extraits de code pour les exportations OTLP avec Grafana (`docs/source/telemetry.md`) pour le traitement par lots pour l'exportateur La taille du lot du premier trimestre est de 256 avec une marge de sécurité maximale.
- Application CI/tests pour les méthodes multivoies avec `integration_tests/tests/nexus/multilane_pipeline.rs` et flux de travail `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`) pour plus de détails. Numéro `pytests/nexus/test_multilane_pipeline.py` ; Vous pouvez utiliser le hachage pour `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) comme tracker pour les bundles disponibles.

## دورة حياة voies في وقت التشغيل- خطط دورة حياة lanes في وقت التشغيل تتحقق الان من liaisons خاصة بـ dataspace وتتوقف عند فشل réconciliation لـ Kura/التخزين الطبقي، مع ترك الكتالوج دون تغيير. Il s'agit d'assistants comme de relais pour les voies de circulation et de preuves pour le grand livre de fusion de synthèse.
- Aides à la configuration/cycle de vie pour Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) pour les voies/voies تشغيل؛ Il y a le routage, les instantanés TEU et les registres de manifestations.
- Fonctionnement : les espaces de données sont associés aux racines de stockage et aux racines de stockage (racine froide à plusieurs niveaux/voie Kura). اصلح المسارات الاساسية وحاول مجددا؛ Les différences entre les voies/espaces de données et les tableaux de bord des tableaux de bord sont également prises en compte.

## تيليمتري NPoS et contre-pression

Test de phase B pour le stimulateur cardiaque NPoS et potins حدود contre-pression. Harnais pour `integration_tests/tests/sumeragi_npos_performance.rs` pour les applications JSON (`sumeragi_baseline_summary::<scenario>::...`) جديدة. شغله محليا عبر:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

اضبط `SUMERAGI_NPOS_STRESS_PEERS` و`SUMERAGI_NPOS_STRESS_COLLECTORS_K` او `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` لاستكشاف طوبولوجيات اشد ضغطا؛ L'installation se fait en 1 s/`k=3` pour B4.| السيناريو / test | التغطية | تيليمتري اساسية |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Il y a 12 heures de temps de bloc pour les enveloppes et la latence EMA ainsi que les jauges et les envois redondants pour le bundle. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Les reports et les admissions sont également possibles. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | La gigue du stimulateur cardiaque et les délais d'attente pour la vue sont de +/-125 pour mille. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Les charges utiles RBC sont disponibles en soft/hard pour le magasin et les octets sont disponibles en ligne. Magasin تجاوز. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Les jauges à envoi redondant et les collecteurs sur cible sont des outils de bout en bout. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Les morceaux contiennent des éléments liés au retard et aux erreurs ainsi qu'aux charges utiles. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Comment utiliser JSON pour exploiter le harnais et gratter le système pour Prometheus La contre-pression est également un problème.## قائمة التحديث

1. Utilisez routed-trace pour utiliser la fonction routed-trace.
2. Connectez-vous à Alertmanager pour vous connecter à Alertmanager.
3. Vous pouvez utiliser les deltas de configuration, le tracker et les résumés du pack de télémétrie pour une demande d'extraction.
4. اربط هنا اي artefact جديد للتدريب/التيليمتري كي تشير تحديثات roadmap المستقبلية الى مستند واحد بدلا من ملاحظات ad hoc متناثرة.

## فهرس الادلة| الاصل | الموقع | ملاحظات |
|-------|----------|-------|
| تقرير تدقيق routed-trace (T1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | المصدر القانوني لادلة Phase B1؛ Il s'agit de `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker pour le delta de configuration | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Il s'agit du TRACE-CONFIG-DELTA et du GOV-2026-03-19. |
| خطة assainissement للتيليمتري | `docs/source/nexus_telemetry_remediation_plan.md` | Il s'agit d'un pack d'alertes avec OTLP et garde-corps pour B2. |
| Tracker تدريب multivoies | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | يسرد artefacts تدريب 9 ابريل وmanifest/digest الخاص بالـ validateur وملاحظات/agenda Q2 وادلة rollback. |
| Manifeste/résumé du pack de télémétrie (dernier) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | La plage de slots 912-936 et la graine `NEXUS-REH-2026Q2` et les artefacts de hachage sont disponibles. |
| Manifeste de profil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | hash avec TLS pour la réexécution du deuxième trimestre Il s'agit d'un routé-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Les travaux ont été effectués au deuxième trimestre (plage d'emplacements, graine de charge de travail, etc.). |
| Runbook تدريب الاطلاق | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Liste de contrôle pour la mise en scène -> exécution -> restauration Il y a des voies et des exportateurs. |
| Validateur de pack télémétrie | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI مشار اليه في rétro B4؛ ارشف digère بجانب tracker عند تغير الحزمة. || Régression multivoie | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Pour `nexus.enabled = true` configurations multi-voies, pour les hachages de catalogue Sora et pour Kura/merge-log pour voie (`blocks/lane_{id:03}_{slug}`) pour `ConfigLaneRouter` قبل نشر digère les artefacts. |