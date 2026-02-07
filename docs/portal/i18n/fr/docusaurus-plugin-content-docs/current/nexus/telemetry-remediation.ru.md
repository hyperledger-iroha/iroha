---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : lien-télémétrie-remédiation
titre : Plan d'installation des capteurs télémétriques Nexus (B2)
description : Le boîtier `docs/source/nexus_telemetry_remediation_plan.md` est destiné à documenter les sondes télémétriques et le processus d'exploitation.
---

# Обзор

Feuille de route **B2 - владение пробелами телеметрии** требует опубликованного плана, который привязывает каждый оставшийся пробел телеметрии Nexus est un signal qui s'applique à la mise en service, la livraison et la fabrication d'œuvres d'art pour l'Audi Q1 2026. Il s'agit d'un signal La page `docs/source/nexus_telemetry_remediation_plan.md` concerne l'ingénierie des versions, les opérations de télémétrie et le SDK qui peuvent être mis à jour avant la répétition de route-trace et `TRACE-TELEMETRY-BRIDGE`.

# Les sondes de matrice| ID d'écart | Signalisation et mise en service | Владелец / эскалация | Temps (UTC) | Docataires et fournisseurs |
|--------|-------------------------|----------|---------------|-------------------------|
| `GAP-TELEM-001` | L'historique `torii_lane_admission_latency_seconds{lane_id,endpoint}` avec l'alerte **`SoranetLaneAdmissionLatencyDegraded`**, doit être activé pendant 5 minutes (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (signal) + `@telemetry-ops` (alerte); эскалация через on-call routé-trace Nexus. | 2026-02-23 | Les tests d'alerte `dashboards/alerts/tests/soranet_lane_rules.test.yml` et la répétition `TRACE-LANE-ROUTING` d'alerte, de surveillance et d'archivage Torii `/metrics` dans [Notes de transition Nexus](./nexus-transition-notes). |
| `GAP-TELEM-002` | Fixez le garde-corps `nexus_config_diff_total{knob,profile}` au garde-corps `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` pour bloquer le déploiement (`docs/source/telemetry.md`). | `@nexus-core` (инструментирование) -> `@telemetry-ops` (alerte); Le mandat de la gouvernance est lié au rôle néo-schétchique. | 2026-02-26 | Выходы gouvernance à sec сохраняются рядом с `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` ; Le bouton de commande indique l'écran Prometheus et les autres logos, qui correspondent à `StateTelemetry::record_nexus_config_diff` diff. || `GAP-TELEM-003` | L'application `TelemetryEvent::AuditOutcome` (métrique `nexus.audit.outcome`) avec l'alerte **`NexusAuditOutcomeFailure`** lors de la surveillance de l'écran ou des résultats affichés est supérieur à 30 minute (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) avec téléchargement dans `@sec-observability`. | 2026-02-27 | Le CI-get `scripts/telemetry/check_nexus_audit_outcome.py` archive les charges utiles NDJSON et permet, bien sûr, TRACE ne soit pas pris en charge par votre entreprise ; Les écrans d'alerte associés à la recherche de trace routée. |
| `GAP-TELEM-004` | Jauge `nexus_lane_configured_total` avec garde-corps `nexus_lane_configured_total != EXPECTED_LANE_COUNT`, pour le personnel de garde SRE. | `@telemetry-ops` (jauge/export) avec la référence `@nexus-core`, vous pouvez utiliser le catalogue de paramètres nécessaire. | 2026-02-28 | Le test de planification télémétrique `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` permet d'envoyer un message ; Les opérateurs utilisent le diff Prometheus + le journal `StateTelemetry::set_nexus_catalogs` dans le paquet de répétition TRACE. |

# Processus de fonctionnement1. **Еженедельный триаж.** Les équipes indiquent l'état d'avancement du programme de préparation Nexus ; Les bloqueurs et les objets d'art testent les alertes détectées dans `status.md`.
2. **Alertes de fonctionnement à sec.** L'alerte de fonctionnement à sec s'affiche lorsque vous utilisez le `dashboards/alerts/tests/*.test.yml`, alors que le CI est activé `promtool test rules` lors de la configuration. garde-corps.
3. **Installation pour l'audio.** Lors de la répétition `TRACE-LANE-ROUTING` et `TRACE-TELEMETRY-BRIDGE`, vous devez obtenir les résultats souhaités. Prometheus, alertes d'historique et scripts pertinents (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` pour les signaux correspondants) et leur suivi Il s'agit d'artéfacts à trace acheminée.
4. **Activation.** Si le garde-corps s'attaque à une répétition, la commande ouvre l'incident Nexus, indiquez-le. plan, et fournit des mesures et des paramètres d'instantanés en cas de risque avant l'audit audio.

La matrice disponible - et les composants de `roadmap.md` et `status.md` - feuille de route **B2** vous permettent de répondre aux critères de sélection "ответственность, срок, алерт, проверка".