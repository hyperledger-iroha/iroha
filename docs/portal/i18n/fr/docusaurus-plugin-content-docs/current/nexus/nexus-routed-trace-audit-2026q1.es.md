---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-routed-trace-audit-2026q1
titre : Informe de auditoria de routed-trace 2026 Q1 (B1)
description: En particulier de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, qui cubre les résultats de la révision trimestrielle de télémétrie.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note Fuente canonica
Cette page reflète `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Manten ambas copias alineadas hasta que lleguen las traducciones restantes.
:::

# Informe des auditoires de Routed-Trace 2026 Q1 (B1)

L'élément de la feuille de route **B1 - Routed-Trace Audits & Telemetry Baseline** nécessite une révision trimestrielle du programme routed-trace de Nexus. Este informe documenta la ventana de auditoria Q1 2026 (janero-marzo) para que le consejo de gobernanza pueda aprobar la postura de telemetria avant les essais de lancement Q2.

## Alcance et ligne de temps

| Identifiant de trace | Ventana (UTC) | Objet |
|--------------|--------------|---------------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Vérifiez les histogrammes d'admission de voie, les potins de colas et les flux d'alertes avant d'activer plusieurs voies. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Valider la relecture OTLP, la parité du bot de comparaison et l'ingestion de télémétrie du SDK avant les hits AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12h00-12h30 | Confirmer les deltas de `iroha_config` approuvés par le gouvernement et la préparation du rollback avant la carte RC1. |Chaque opération est exécutée dans une topologie de type production avec l'instrument routé-trace habilité (télémétrie `nexus.audit.outcome` + contacteurs Prometheus), conformément au gestionnaire d'alerte chargé et aux preuves exportées en `docs/examples/`.

## Méthodologie

1. **Recoleccion de telemetria.** Tous les nœuds émettent l'événement structuré `nexus.audit.outcome` et les mesures associées (`nexus_audit_outcome_total*`). L'assistant `scripts/telemetry/check_nexus_audit_outcome.py` a une queue de journal JSON, valide l'état de l'événement et archive la charge utile dans `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Validation des alertes.** `dashboards/alerts/nexus_audit_rules.yml` et votre harnais de vérification assure que les ombrelles de gestion des alertes et les modèles de charge utile sont cohérents. CI exécute `dashboards/alerts/tests/nexus_audit_rules.test.yml` à chaque changement ; las mismas reglas se ejercitaron manualmente durante cada ventana.
3. **Capture des tableaux de bord.** Les opérateurs exportent les panneaux routés-trace de `dashboards/grafana/soranet_sn16_handshake.json` (salud de handshake) et les tableaux de bord de télémétrie générale pour corréler la santé des colis avec les résultats de l'auditoire.
4. **Notes de révision.** La secrétaire d'État enregistre les premières révisions, les décisions et les tickets d'atténuation dans les [notes de transition Nexus] (./nexus-transition-notes) et le suivi des deltas de configuration (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Hallazgos| Identifiant de trace | Résultat | Preuve | Notes |
|----------|---------|--------------|-------|
| `TRACE-LANE-ROUTING` | Passer | Captures d'alerte incendie/récupération (enlace interno) + replay de `dashboards/alerts/tests/soranet_lane_rules.test.yml` ; différences de télémétrie enregistrées en [Nexus transition notes](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 de l'admission de cola se déroule en 612 ms (objet <=750 ms). Aucune suite n'est requise. |
| `TRACE-TELEMETRY-BRIDGE` | Passer | Payload archivé `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` mais le hachage de replay OTLP est enregistré en `status.md`. | Les sels de rédaction du SDK coïncident avec la base Rust ; le diff bot rapporte des deltas de zéro. |
| `TRACE-CONFIG-DELTA` | Pass (atténuation fermée) | Accès au tracker de gouvernement (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifeste de profil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifeste de paquet de télémétrie (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | La rejet Q2 a le profil TLS approuvé et confirmé sans engagement ; Le manifeste de télémétrie enregistre la gamme d'emplacements 912-936 et la graine de charge de travail `NEXUS-REH-2026Q2`. |

Toutes les traces produisent au moins un événement `nexus.audit.outcome` à l'intérieur de vos fenêtres, satisfaisant les garde-corps d'Alertmanager (`NexusAuditOutcomeFailure` se maintenant en vert pendant le trimestre).

## Suivis- Mettre à jour l'annexe routed-trace avec le hachage TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ; l'atténuation `NEXUS-421` se cerro dans les notes de transition.
- Continuer à ajouter des replays OTLP sans processus et artefacts de différence de Torii dans l'archive pour renforcer la preuve de parité pour les révisions d'Android AND4/AND7.
- Confirmer que les prochaines répétitions `TRACE-MULTILANE-CANARY` réutilisent le même assistant de télémétrie pour que la signature du Q2 bénéficie du flux validé.

## Indice des artefacts

| Actif | Emplacement |
|-------|--------------|
| Validateur de télémétrie | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Règlements et tests d'alerte | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Charge utile du résultat de l'exemple | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker delta de configuration | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Calendrier et notes de routed-trace | [Notes de transition Nexus](./nexus-transition-notes) |

Cette information, les artefacts antérieurs et les exportations d'alertes/télémétries doivent être ajoutés au registre de décision de gouvernement pour arrêter le trimestre B1.