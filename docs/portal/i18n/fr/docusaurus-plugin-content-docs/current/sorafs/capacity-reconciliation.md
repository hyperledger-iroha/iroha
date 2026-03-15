---
id: capacity-reconciliation
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/capacity-reconciliation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

L’item de roadmap **SF-2c** impose que la trésorerie prouve que le registre des frais de capacité correspond aux transferts XOR exécutés chaque nuit. Utilisez le helper `scripts/telemetry/capacity_reconcile.py` pour comparer le snapshot `/v1/sorafs/capacity/state` au lot de transferts exécuté et émettre des métriques textuelles Prometheus pour Alertmanager.

## Prérequis
- Snapshot d’état de capacité (entrées `fee_ledger`) exporté depuis Torii.
- Export du ledger pour la même fenêtre (JSON ou NDJSON avec `provider_id_hex`,
  `kind` = settlement/penalty, et `amount_nano`).
- Chemin vers le textfile collector de node_exporter si vous souhaitez des alertes.

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- Codes de sortie : `0` si la concordance est parfaite, `1` lorsque des settlements/penalties manquent ou sont surpayés, `2` si les entrées sont invalides.
- Joignez le résumé JSON + les hashes au packet de trésorerie dans
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- Lorsque le fichier `.prom` arrive dans le textfile collector, l’alerte
  `SoraFSCapacityReconciliationMismatch` (voir
  `dashboards/alerts/sorafs_capacity_rules.yml`) se déclenche lorsque des transferts manquants, surpayés ou inattendus sont détectés.

## Sorties
- Statuts par fournisseur avec diffs pour settlements et penalties.
- Totaux exportés en tant que jauges :
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## Plages attendues et tolérances
- La réconciliation est exacte : les nanos attendus vs réels de settlement/penalty doivent correspondre avec une tolérance zéro. Tout écart non nul doit pager les opérateurs.
- CI fixe un digest de soak sur 30 jours pour le ledger des frais de capacité (test `capacity_fee_ledger_30_day_soak_deterministic`) à `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`. Actualisez ce digest uniquement lorsque les règles de pricing ou de cooldown changent.
- Dans le profil de soak (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`), les penalties restent à zéro ; en production, les penalties ne doivent être émis que lorsque les seuils d’utilisation/uptime/PoR sont franchis et doivent respecter le cooldown configuré avant des slashes successifs.
