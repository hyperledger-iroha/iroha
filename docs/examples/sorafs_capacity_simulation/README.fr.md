---
lang: fr
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-11-05T17:59:15.481814+00:00"
translation_last_reviewed: 2026-01-01
---

# Toolkit de simulation de capacite SoraFS

Ce repertoire livre les artefacts reproductibles pour la simulation du marche de capacite SF-2c. Le toolkit exerce la negotiation de quotas, la gestion du failover et la remediation de slashing avec les helpers CLI de production et un script d'analyse leger.

## Prerequis

- Toolchain Rust capable d'executer `cargo run` pour les membres du workspace.
- Python 3.10+ (bibliotheque standard uniquement).

## Demarrage rapide

```bash
# 1. Generer les artefacts CLI canoniques
./run_cli.sh ./artifacts

# 2. Agreger les resultats et emettre les metriques Prometheus
./analyze.py --artifacts ./artifacts
```

Le script `run_cli.sh` invoque `sorafs_manifest_stub capacity` pour construire:

- Des declarations de providers deterministes pour le set de fixtures de negotiation de quotas.
- Un ordre de replication qui correspond au scenario de negotiation.
- Des snapshots de telemetrie pour la fenetre de failover.
- Un payload de litige qui capture la demande de slashing.

Le script ecrit des bytes Norito (`*.to`), des payloads base64 (`*.b64`), des
bodies de requete Torii et des resumes lisibles (`*_summary.json`) sous le
repertoire d'artefacts choisi.

`analyze.py` consomme les resumes generes, produit un rapport agregat
(`capacity_simulation_report.json`) et emet un textfile Prometheus
(`capacity_simulation.prom`) contenant:

- Des gauges `sorafs_simulation_quota_*` decrivant la capacite negociee et la
  part d'allocation par provider.
- Des gauges `sorafs_simulation_failover_*` mettant en avant les deltas de downtime et
  le provider de remplacement selectionne.
- `sorafs_simulation_slash_requested` enregistrant le pourcentage de remediation
  extrait du payload de litige.

Importez le bundle Grafana dans `dashboards/grafana/sorafs_capacity_simulation.json`
et pointez vers une datasource Prometheus qui scrape le textfile genere (par exemple
via le textfile collector de node-exporter). Le runbook dans
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` decrit le workflow complet,
y compris des conseils de configuration Prometheus.

## Fixtures

- `scenarios/quota_negotiation/` - Specs de declaration provider et ordre de replication.
- `scenarios/failover/` - Fenetres de telemetrie pour la panne primaire et le lift de failover.
- `scenarios/slashing/` - Spec de litige qui reference le meme ordre de replication.

Ces fixtures sont validees dans `crates/sorafs_car/tests/capacity_simulation_toolkit.rs`
pour garantir qu'elles restent synchronisees avec le schema CLI.
