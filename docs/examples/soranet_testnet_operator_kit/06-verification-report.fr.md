---
lang: fr
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/06-verification-report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf489427d0eba2beebfdefc44092730c3963cbd77e83669853f4e9681ac9fd2d
source_last_modified: "2025-11-21T14:25:28.923348+00:00"
translation_last_reviewed: 2026-01-01
---

## Rapport de verification operateur (phase T0)

- Nom de l'operateur: ______________________
- ID du descripteur relay: ______________________
- Date de soumission (UTC): ___________________
- Email / matrix de contact: ___________________

### Resume de checklist

| Element | Termine (O/N) | Notes |
|------|-----------------|-------|
| Materiel et reseau valides | | |
| Bloc compliance applique | | |
| Envelope d'admission verifie | | |
| Smoke test de guard rotation | | |
| Telemetrie scrapee et dashboards actifs | | |
| Brownout drill execute | | |
| Succes des tickets PoW dans la cible | | |

### Snapshot de metriques

- Ratio PQ (`sorafs_orchestrator_pq_ratio`): ________
- Compte de downgrade dernieres 24h: ________
- RTT moyen des circuits (p95): ________ ms
- Temps median de resolution PoW: ________ ms

### Pieces jointes

Merci de joindre:

1. Hash du support bundle relay (`sha256`): __________________________
2. Captures de dashboards (ratio PQ, succes circuits, histogramme PoW).
3. Bundle de drill signe (`drills-signed.json` + cle publique du signataire en hex et pieces jointes).
4. Rapport de metriques SNNet-10 (`cargo xtask soranet-testnet-metrics --input <snapshot> --out metrics-report.json`).

### Signature operateur

Je certifie que les informations ci-dessus sont exactes et que toutes les etapes requises sont completes.

Signature: _________________________  Date: ___________________
