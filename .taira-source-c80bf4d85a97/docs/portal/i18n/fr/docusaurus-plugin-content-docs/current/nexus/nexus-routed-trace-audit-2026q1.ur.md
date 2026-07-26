---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-routed-trace-audit-2026q1
titre : 2026 Q1 routé-trace آڈٹ رپورٹ (B1)
description: `docs/source/nexus_routed_trace_audit_report_2026q1.md` کا آئینہ، جو سہ ماہی ٹیلیمیٹری ریہرسل کے نتائج کا احاطہ کرتا ہے۔
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_routed_trace_audit_report_2026q1.md` کی عکاسی کرتا ہے۔ باقی تراجم آنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# 2026 Q1 Routed-Trace آڈٹ رپورٹ (B1)

روڈ میپ آئٹم **B1 - Routed-Trace Audits & Telemetry Baseline** Nexus routed-trace est disponible en ligne. یہ رپورٹ Q1 2026 (جنوری-مارچ) کی آڈٹ ونڈو دستاویز کرتی ہے تاکہ گورننس کونسل Q2 لانچ ریہرسل سے پہلے ٹیلیمیٹری پوزیشن کی منظوری دے سکے۔

## دائرہ کار اور ٹائم لائن

| Identifiant de trace | ونڈو (UTC) | مقصد |
|--------------|--------------|---------------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | multi-voies, des histogrammes d'admission de voie, des potins de file d'attente et un flux d'alerte. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Jalons AND4/AND7 pour la relecture OTLP, la parité différentielle des bots et l'ingestion de télémétrie SDK. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12h00-12h30 | RC1 coupe les deltas `iroha_config` approuvés par la gouvernance et la préparation au retour en arrière |

Il s'agit d'une topologie de type production et d'instruments de trace acheminée ainsi que de règles d'Alertmanager et de preuves. `docs/examples/` میں ایکسپورٹ ہوا۔

## طریقہ کار1. ** Les métriques ** ٹیلیمیٹری** Les métriques structurées `nexus.audit.outcome` et les métriques متعلقہ (`nexus_audit_outcome_total*`) émettent des émissions helper `scripts/telemetry/check_nexus_audit_outcome.py` pour la queue de journal JSON et la charge utile `docs/examples/nexus_audit_outcomes/` pour la charge utile کیا۔ [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **`dashboards/alerts/nexus_audit_rules.yml` pour le harnais de test et les seuils de bruit d'alerte et modèles de charge utile. CI ہر تبدیلی پر `dashboards/alerts/tests/nexus_audit_rules.test.yml` چلاتا ہے؛ یہی رولز ہر ونڈو میں دستی طور پر بھی چلائے گئے۔
3. **L'état de santé de la poignée de main est `dashboards/grafana/soranet_sn16_handshake.json`, les panneaux de trace acheminée et les tableaux de bord de présentation de la télémétrie et l'état de la file d'attente. Les résultats de l'audit sont en corrélation avec les autres
4. ** ریویو نوٹس۔** گورننس سیکرٹری نے reviewer initials, فیصلہ اور tickets d'atténuation et [Nexus transition notes](./nexus-transition-notes) et config delta tracker (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) میں لاگ کیا۔

## نتائج| Identifiant de trace | نتیجہ | ثبوت | نوٹس |
|----------|---------|--------------|-------|
| `TRACE-LANE-ROUTING` | Passer | Alerte incendie/récupération اسکرین شاٹس (اندرونی لنک) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` replay؛ différences de télémétrie [notes de transition Nexus] (./nexus-transition-notes#quarterly-routed-trace-audit-schedule) میں ریکارڈ۔ | Admission de file d'attente P95 612 ms پر رہا (ہدف <=750 ms)۔ فالو اپ درکار نہیں۔ |
| `TRACE-TELEMETRY-BRIDGE` | Passer | Charge utile du résultat archivé `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` et hachage de relecture OTLP `status.md` | Sels de rédaction du SDK Rust baseline سے match تھے؛ diff bot نے zéro deltas رپورٹ کیے۔ |
| `TRACE-CONFIG-DELTA` | Pass (atténuation fermée) | Entrée du tracker de gouvernance (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifeste de profil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifeste du pack de télémétrie (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Réexécution du deuxième trimestre avec hachage de profil TLS et zéro retardateur manifeste de télémétrie dans les emplacements 912-936 et graine de charge de travail `NEXUS-REH-2026Q2` pour le client |

Les traces sont disponibles en ligne avec le système `nexus.audit.outcome` pour les garde-corps Alertmanager. ہوئے (`NexusAuditOutcomeFailure` پورے کوارٹر میں گرین رہا)۔

## Suivis- Annexe de trace acheminée pour le hachage TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`. atténuation des notes de transition `NEXUS-421`
- Android AND4/AND7 examine les preuves de parité et les replays OTLP bruts ainsi que les artefacts de diff Torii. کرتے رہیں۔
- Répétitions `TRACE-MULTILANE-CANARY` et assistant de télémétrie pour le workflow validé d'approbation du deuxième trimestre فائدہ اٹھائے۔

## Artefact انڈیکس

| Actif | مقام |
|-------|--------------|
| Validateur de télémétrie | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Règles d'alerte & tests | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Exemple de charge utile de résultat | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Suivi delta de configuration | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Calendrier et notes du tracé acheminé | [Notes de transition Nexus](./nexus-transition-notes) |

Il s'agit d'artefacts et d'exportations d'alertes/télémétrie et de journaux de décisions de gouvernance. کے لئے B1 بند ہو جائے۔