---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-routed-trace-audit-2026q1
titre : Relatorio de auditoria routed-trace 2026 Q1 (B1)
description : Espelho de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, cobrindo os resultados trimestrais das revisoes de telemetria.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note Fonte canonica
Cette page reflète `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Mantenha as duas copias alinhadas ate que as traducoes restantes cheguem.
:::

# Rapport d'audience Routed-Trace 2026 Q1 (B1)

L'élément de la feuille de route **B1 - Routed-Trace Audits & Telemetry Baseline** exige une révision trimestrielle du programme routed-trace Nexus. Ce rapport est documenté pour le premier trimestre 2026 (janvier-marc) de l'auditoire afin que le conseil d'administration puisse approuver la position de télémétrie avant les essais de lancement du deuxième trimestre.

## Escopo et chronogramme

| Identifiant de trace | Janela (UTC) | Objet |
|--------------|--------------|---------------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Vérifiez les histogrammes d'admission de voie, les potins de fil et le flux d'alertes avant l'habilitation multi-voies. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Valider la relecture OTLP, la parité du bot de comparaison et l'acquisition de la télémétrie du SDK avant les marques AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12h00-12h30 | Confirmer les deltas de `iroha_config` approuvés par la gouvernance et la prévision de restauration avant la carte RC1. |Cela permet d'utiliser une topologie similaire à la production d'un instrument de trace routée habilité (télémétrie `nexus.audit.outcome` + contacteurs Prometheus), enregistré par Alertmanager chargé et preuves exportées pour `docs/examples/`.

## Méthodologie

1. **Coleta de telemetria.** Tous nos émetteurs ou événements structurés `nexus.audit.outcome` et en tant que mesures associées (`nexus_audit_outcome_total*`). L'assistant `scripts/telemetry/check_nexus_audit_outcome.py` fez la queue du journal JSON, validez le statut de l'événement et archivez la charge utile dans `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Validation des alertes.** `dashboards/alerts/nexus_audit_rules.yml` et votre harnais de test garantissent que les limites de gestion et les modèles rendent la charge utile cohérente en permanence. O CI executa `dashboards/alerts/tests/nexus_audit_rules.test.yml` à cada mudanca ; comme mesmas regras foram exercitadas manualmente durante cada janela.
3. **Capture des tableaux de bord.** Les opérateurs exportent les traces routées de `dashboards/grafana/soranet_sn16_handshake.json` (sans poignée de main) et les tableaux de bord de visa général de télémétrie pour corréler l'audace des fils avec les résultats de l'auditoire.
4. **Notes de révision.** Le secrétaire d'État enregistre les débuts des réviseurs, les décisions et les tickets d'atténuation dans les [notes de transition Nexus] (./nexus-transition-notes) et aucun suivi du delta de configuration (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Achados| Identifiant de trace | Résultat | Preuve | Notes |
|----------|---------|--------------|-------|
| `TRACE-LANE-ROUTING` | Passer | Captures d'alerte incendie/récupération (lien interne) + replay de `dashboards/alerts/tests/soranet_lane_rules.test.yml` ; différences de télémétrie enregistrées dans [Nexus transition notes](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 de l'admission du fil en permanence à 612 ms (alvo <=750 ms). Suivi Sem. |
| `TRACE-TELEMETRY-BRIDGE` | Passer | Charge utile archivée `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` mais hachage de relecture OTLP enregistré dans `status.md`. | Les sels de rédaction du SDK sont basés sur Rust ; Le robot de comparaison rapporte zéro delta. |
| `TRACE-CONFIG-DELTA` | Pass (atténuation fermée) | Entrée sans suivi de gouvernance (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifeste de profil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifeste du paquet de télémétrie (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Une réexécution du deuxième trimestre a permis d'approuver le profil TLS et de confirmer qu'il n'y a aucun retard ; Le manifeste de télémétrie enregistré ou l'intervalle des emplacements 912-936 et la graine de charge de travail `NEXUS-REH-2026Q2`. |

Toutes les traces seront produites au moins un événement `nexus.audit.outcome` à l'intérieur de vos fenêtres, en satisfaisant les garde-corps d'Alertmanager (`NexusAuditOutcomeFailure` vert permanent pendant un trimestre).

## Suivis- L'annexe routed-trace a été actualisée avec le hachage TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ; a mitigacao `NEXUS-421` foi encerrada nas notes de transition.
- Continuer l'analyse des replays OTLP bruts et artefatos de diff do Torii à l'archive pour renforcer les preuves de parité pour la révision d'Android AND4/AND7.
- Confirmer qu'au fur et à mesure des répétitions, `TRACE-MULTILANE-CANARY` réutilisera l'assistant de télémétrie pour que la signature du deuxième trimestre bénéficie du flux de travail validé.

## Indice des artéfacts

| Ativo | Locale |
|-------|--------------|
| Validateur de télémétrie | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Registres et tests d'alerte | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Charge utile du résultat de l'exemple | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker delta de configuration | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Cronogrammes et notes routé-trace | [Notes de transition Nexus](./nexus-transition-notes) |

Dans ce rapport, les artéfacts acima et les exportations d'alertes/télémétrie doivent être ajoutés au journal des décisions de gouvernance pour la fechar ou le B1 du trimestre.