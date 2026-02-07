---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-routed-trace-audit-2026q1
כותרת: Rapport d'audit routed-trace 2026 Q1 (B1)
תיאור: Miroir de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, קוברנט לתוצאות טרימסטריאל של חזרות בטלמטריה.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::הערה מקור קנוניק
Cette page reflete `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Gardez les deux copies alignees jusqu'a ce que les traductions restantes arrivent.
:::

# Rapport d'audit Routed-Trace 2026 Q1 (B1)

L'item de roadmap **B1 - נתיב-מעקב אחר ביקורת וטלמטריה Baseline** exige une revue trimestrielle du program routed-trace Nexus. Ce rapport documente la fenetre d'audit Q1 2026 (janvier-mars) afin que le conseil de gouvernance puisse valider la posture de telemetrie avant les repetitions de lancement Q2.

## Portee et calendrier

| מזהה מעקב | פנטר (UTC) | Objectif |
|--------|-------------|--------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Verifier les histogrammes d'admission des lanes, le gossip des files et le flux d'alertes avant l'activation multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Valider le replay OTLP, la parite du diff bot et l'ingestion de telemetrie SDK avant les jalons AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Confirmer les deltas `iroha_config` מאשר על פי הממשל וההכנה או החזרה לאחור avant le cut RC1. |

חזרת צ'אקה בטורנה על טופולוגיה פרושה לייצור עם מכשירים מנותבים-עקבות פעילים (טלמטרי `nexus.audit.outcome` + compteurs Prometheus), les regles Alertmanager chargees et des preuves exportees dans `docs/examples/`.

## מתודולוגיה

1. **Collecte de telemetrie.** Tous les noeuds ont emis l'evenement structure `nexus.audit.outcome` et les metriques associees (`nexus_audit_outcome_total*`). העוזר `scripts/telemetry/check_nexus_audit_outcome.py` נמצא כעת ב-JSON, תקף לתקנות האירועים והארכיון של מטען המטען ב-`docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. ** Validation des alertes.** `dashboards/alerts/nexus_audit_rules.yml` ו-son harness de test on assure que les seuils de bruit et le templating des payloads restent coherents. CI לבצע `dashboards/alerts/tests/nexus_audit_rules.test.yml` שינוי צ'אק; les memes regles ont ete exercees manuellement תליון chaque fenetre.
3. **Capture des Dashboards.** המפעילים מוצאים את ה-Panneaux עם ניתוב עקבות של `dashboards/grafana/soranet_sn16_handshake.json` (לחיצת יד טובה) ולוחות המחוונים של טלמטרים גלובליים לעזרת קבצים טובים עם תוצאות ביקורת.
4. **Notes des relecteurs.** La secretaire de governance a consigne les initiales, la decision et les tickets de mitigation dans [Nexus הערות מעבר](./nexus-transition-notes) et dans le tracker de deltas de config (Nexus).

## קונסטטציות| מזהה מעקב | תוצאות | Preuves | הערות |
|--------|--------|--------|-------|
| `TRACE-LANE-ROUTING` | לעבור | לוכד אש/שחזור התראה (שעבוד פנימי) + שידור חוזר `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diffs de telemetrie enregistres dans [Nexus הערות מעבר](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | Le P95 d'admission de file est reste a 612 ms (cible <=750 ms). Aucun suivi requis. |
| `TRACE-TELEMETRY-BRIDGE` | לעבור | ארכיון מטען `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` בתוספת hash de replay OTLP נרשם ב-`status.md`. | Les sels de redaction SDK correspondaient a la base Rust; le diff bot a signale zero delta. |
| `TRACE-CONFIG-DELTA` | מעבר (הקלה סגורה) | Entree du tracker de governance (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + פרופיל מניפסט TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + Manifest du pack telemetrie (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | הרצה חוזרת של רבעון 2 עם פרופיל TLS אישור ואישור אפס מעכבים; המניפסט טלמטרי נרשמים ל-Plage de slots 912-936 et le Seed עומס העבודה `NEXUS-REH-2026Q2`. |

Tous les traces ont produit au moins un evenement `nexus.audit.outcome` dans leeurs fenetres, satisfaisfaisfaisfaisfaisfaising les guards Alertmanager (`NexusAuditOutcomeFailure` est reste vert sur le trimestre).

## Suivis

- L'appendice מנותב-trace a ete mis a jour avec le hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; la mitigation `NEXUS-421` est fermee dans les transition notes.
- המשך להצטרף לשידורים חוזרים של OTLP bruts et les artefacts de diff Torii a l'archive pour renforcer la preuve de parite pour les revues Android AND4/AND7.
- Confirmer que les prochaines חזרות `TRACE-MULTILANE-CANARY` reutilisent le meme helper de telemetrie pour que la validation Q2 beneficie du workflow valide.

## אינדקס חפצי אומנות

| נכס | מיקום |
|-------|--------|
| Validateur de telemetrie | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Regles et tests d'alerte | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| דוגמה לתוצאת מטען | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker des deltas de config | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Planning et notes routed-trace | [הערות מעבר Nexus](./nexus-transition-notes) |

Ce rapport, les artefacts ci-dessus et les exports d'alertes/telemetrie doivent etre attaches au journal de decision de governance pour cloturer B1 du trimestre.