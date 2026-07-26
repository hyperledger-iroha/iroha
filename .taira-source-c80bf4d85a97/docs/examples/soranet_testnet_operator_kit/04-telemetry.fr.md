---
lang: fr
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-11-04T17:24:14.014911+00:00"
translation_last_reviewed: 2026-01-01
---

# Exigences de telemetrie

## Cibles Prometheus

Scrapez le relay et l'orchestrator avec les labels suivants:

```yaml
- job_name: "soranet-relay"
  static_configs:
    - targets: ["relay-host:9898"]
      labels:
        region: "testnet-t0"
        role: "relay"
- job_name: "sorafs-orchestrator"
  static_configs:
    - targets: ["orchestrator-host:9797"]
      labels:
        region: "testnet-t0"
        role: "orchestrator"
```

## Dashboards requis

1. `dashboards/grafana/soranet_testnet_overview.json` *(a publier)* - chargez le JSON et importez les variables `region` et `relay_id`.
2. `dashboards/grafana/soranet_privacy_metrics.json` *(asset SNNet-8 existant)* - assurez-vous que les panneaux de privacy bucket s'affichent sans trous.

## Regles d'alerte

Les seuils doivent correspondre aux attentes du playbook:

- `soranet_privacy_circuit_events_total{kind="downgrade"}` increase > 0 sur 10 minutes declenche `critical`.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 sur 30 minutes declenche `warning`.
- `up{job="soranet-relay"}` == 0 pendant 2 minutes declenche `critical`.

Chargez vos regles dans Alertmanager avec le receiver `testnet-t0`; validez avec `amtool check-config`.

## Evaluation des metriques

Agregez un snapshot de 14 jours et donnez-le au validateur SNNet-10:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- Remplacez le fichier sample par votre snapshot exporte lors d'un run sur des donnees live.
- Un resultat `status = fail` bloque la promotion; corrigez les checks signales avant de reessayer.

## Reporting

Chaque semaine, televersez:

- Captures de requetes (`.png` ou `.pdf`) montrant le ratio PQ, le taux de succes des circuits et l'histogramme de resolution PoW.
- Sortie des regles d'enregistrement Prometheus pour `soranet_privacy_throttles_per_minute`.
- Un court recit decrivant les alertes qui ont declenche et les etapes de mitigation (inclure les timestamps).
