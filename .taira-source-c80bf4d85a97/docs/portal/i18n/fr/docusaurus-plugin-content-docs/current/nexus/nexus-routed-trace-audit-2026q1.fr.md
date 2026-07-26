---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-routed-trace-audit-2026q1
titre : Rapport d'audit routé-trace 2026 Q1 (B1)
description : Miroir de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, couvrant les résultats trimestriels des répétitions de télémétrie.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note Source canonique
Cette page reflète `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Gardez les deux copies alignées jusqu'à ce que les traductions restantes arrivent.
:::

# Rapport d'audit Routed-Trace 2026 Q1 (B1)

L'item de roadmap **B1 - Routed-Trace Audits & Telemetry Baseline** exige une revue trimestrielle du programme routed-trace Nexus. Ce rapport documente la fenêtre d'audit Q1 2026 (janvier-mars) afin que le conseil de gouvernance puisse valider la posture de télémétrie avant les répétitions de lancement Q2.

## Portee et calendrier

| Identifiant de trace | Fenêtre (UTC) | Objectif |
|--------------|--------------|---------------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Vérifier les histogrammes d'admission des voies, le gossip des fichiers et le flux d'alertes avant l'activation multi-voies. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Valider le replay OTLP, la parité du bot de diff et l'ingestion de télémétrie SDK avant les jalons AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12h00-12h30 | Confirmer les deltas `iroha_config` approuvés par la gouvernance et la préparation au rollback avant le cut RC1. |Chaque répétition a tourne sur une topologie proche production avec l'instrumentation routed-trace active (télémétrie `nexus.audit.outcome` + compteurs Prometheus), les règles Alertmanager chargées et des preuves exportées dans `docs/examples/`.

## Méthodologie

1. **Collecte de télémétrie.** Tous les noeuds ont émis l'événement structure `nexus.audit.outcome` et les métriques associées (`nexus_audit_outcome_total*`). Le helper `scripts/telemetry/check_nexus_audit_outcome.py` a suivi le log JSON, valide le statut de l'événement et archive le payload sous `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Validation des alertes.** `dashboards/alerts/nexus_audit_rules.yml` et son harnais de test ont assuré que les seuils de bruit et le templating des charges utiles restent cohérents. CI exécute `dashboards/alerts/tests/nexus_audit_rules.test.yml` à chaque modification ; les memes regles ont ete exerces manuellement pendant chaque fenetre.
3. **Capture des tableaux de bord.** Les opérateurs ont exporté les panneaux routé-trace depuis `dashboards/grafana/soranet_sn16_handshake.json` (sante handshake) et les tableaux de bord de télémétrie globale pour correler la santé des fichiers avec les résultats d'audit.
4. **Notes des relecteurs.** La secrétaire de gouvernance a consigné les initiales, la décision et les tickets de mitigation dans [Nexus transition notes](./nexus-transition-notes) et dans le tracker de deltas de config (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

##Constatations| Identifiant de trace | Résultat | Préuvés | Remarques |
|----------|---------|--------------|-------|
| `TRACE-LANE-ROUTING` | Passer | Capture alerte incendie/récupération (lien interne) + replay `dashboards/alerts/tests/soranet_lane_rules.test.yml` ; diffs de télémétrie enregistrés dans [Nexus transition notes](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | Le P95 d'admission de fichier reste à 612 ms (cible <=750 ms). Aucun suivi requis. |
| `TRACE-TELEMETRY-BRIDGE` | Passer | Archive de charge utile `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` plus hash de replay OTLP enregistré dans `status.md`. | Les sels de rédaction du SDK correspondaient à la base Rust ; le diff bot a le signal zéro delta. |
| `TRACE-CONFIG-DELTA` | Pass (atténuation fermée) | Entrée du tracker de gouvernance (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifeste profil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifeste du pack télémétrie (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | La rediffusion Q2 a hashe le profil TLS approuve et confirme zéro retardataires ; le manifeste télémétrie enregistre la plage de slots 912-936 et le workload seed `NEXUS-REH-2026Q2`. |

Toutes les traces ont produit au moins un événement `nexus.audit.outcome` dans leurs fenêtres, satisfaisant les garde-corps Alertmanager (`NexusAuditOutcomeFailure` est reste vert sur le trimestre).

## Suivis- L'annexe routed-trace a été mise à jour avec le hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; la atténuation `NEXUS-421` est fermée dans les notes de transition.
- Continuer à joindre les replays OTLP bruts et les artefacts de diff Torii à l'archive pour renforcer la preuve de parite pour les revues Android AND4/AND7.
- Confirmer que les prochaines répétitions `TRACE-MULTILANE-CANARY` réutilisent le même helper de télémétrie pour que la validation Q2 bénéficie du workflow valide.

## Index des artefacts

| Actif | Emplacement |
|-------|--------------|
| Validateur de télémétrie | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Règlements et tests d'alerte | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Exemple de résultat de charge utile | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker des deltas de configuration | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Planification et notes routé-trace | [Notes de transition Nexus](./nexus-transition-notes) |

Ce rapport, les artefacts ci-dessus et les exportations d'alertes/télémétrie doivent être attachés au journal de décision de gouvernance pour cloturer B1 du trimestre.