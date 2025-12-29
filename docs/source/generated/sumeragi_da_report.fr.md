---
lang: fr
direction: ltr
source: docs/source/generated/sumeragi_da_report.md
status: needs-update
translator: manual
generator: scripts/sync_docs_i18n.py
source_hash: fd381ca301bd673a337c07379e755e0cd9c390072fa60aece98b60cddcea351a
source_last_modified: "2025-11-02T04:40:40.073146+00:00"
translation_last_reviewed: 2025-11-14
---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to `docs/source/generated/sumeragi_da_report.md` for current semantics.

# Rapport de disponibilité des données Sumeragi

3 fichiers de synthèse ont été traités à partir de `artifacts/sumeragi-da/20251005T190335Z`.

## Résumé

| Scénario | Exécutions | Pairs | Charge utile (MiB) | Médiane RBC deliver (ms) | Max RBC deliver (ms) | Médiane Commit (ms) | Max Commit (ms) | Médiane du débit (MiB/s) | Débit minimum (MiB/s) | RBC<=Commit | Profondeur max file BG | Max pertes P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| sumeragi_rbc_da_large_payload_four_peers | 1 | 4 | 10.50 | 3120 | 3120 | 3380 | 3380 | 3.28 | 3.28 | yes | 18 | 0 |
| sumeragi_rbc_da_large_payload_six_peers | 1 | 6 | 10.50 | 3280 | 3280 | 3560 | 3560 | 3.21 | 3.21 | yes | 24 | 0 |
| sumeragi_rbc_recovers_after_peer_restart | 1 | 4 | 10.50 | 3340 | 3340 | 3620 | 3620 | 3.16 | 3.16 | yes | 19 | 0 |

### sumeragi_rbc_da_large_payload_four_peers

- exécutions : 1
- pairs : 4
- charge utile : 11010048 octets (10.50 MiB)
- chunks RBC observés : 168
- nombre de votes READY : 4
- livraison RBC protégée par Commit : yes
- moyenne de RBC deliver (ms) : 3120.00
- moyenne de Commit (ms) : 3380.00
- débit moyen (MiB/s) : 3.28
- profondeur de file BG post max/médiane : 18 / 18
- pertes dans la file P2P max/médiane : 0 / 0
- octets par pair : 11010048 - 11010048
- diffusions deliver par pair : 1 - 1
- diffusions READY par pair : 1 - 1

| Exécution | Source | Bloc | Hauteur | View | RBC deliver (ms) | Commit (ms) | Débit (MiB/s) | RBC<=Commit | READY | Total chunks | Reçus | Profondeur BG max | Pertes P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_four_peers.summary.json | 0x8f2fd8804f... | 58 | 2 | 3120 | 3380 | 3.28 | yes | 4 | 168 | 168 | 18 | 0 |

### sumeragi_rbc_da_large_payload_six_peers

- exécutions : 1
- pairs : 6
- charge utile : 11010048 octets (10.50 MiB)
- chunks RBC observés : 168
- nombre de votes READY : 5
- livraison RBC protégée par Commit : yes
- moyenne de RBC deliver (ms) : 3280.00
- moyenne de Commit (ms) : 3560.00
- débit moyen (MiB/s) : 3.21
- profondeur de file BG post max/médiane : 24 / 24
- pertes dans la file P2P max/médiane : 0 / 0
- octets par pair : 11010048 - 11010048
- diffusions deliver par pair : 1 - 1
- diffusions READY par pair : 1 - 1

| Exécution | Source | Bloc | Hauteur | View | RBC deliver (ms) | Commit (ms) | Débit (MiB/s) | RBC<=Commit | READY | Total chunks | Reçus | Profondeur BG max | Pertes P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_six_peers.summary.json | 0x71e3cdebcf... | 59 | 3 | 3280 | 3560 | 3.21 | yes | 5 | 168 | 168 | 24 | 0 |

### sumeragi_rbc_recovers_after_peer_restart

- exécutions : 1
- pairs : 4
- charge utile : 11010048 octets (10.50 MiB)
- chunks RBC observés : 168
- nombre de votes READY : 4
- livraison RBC protégée par Commit : yes
- moyenne de RBC deliver (ms) : 3340.00
- moyenne de Commit (ms) : 3620.00
- débit moyen (MiB/s) : 3.16
- profondeur de file BG post max/médiane : 19 / 19
- pertes dans la file P2P max/médiane : 0 / 0
- octets par pair : 11010048 - 11010048
- diffusions deliver par pair : 1 - 1
- diffusions READY par pair : 1 - 1

| Exécution | Source | Bloc | Hauteur | View | RBC deliver (ms) | Commit (ms) | Débit (MiB/s) | RBC<=Commit | READY | Total chunks | Reçus | Profondeur BG max | Pertes P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_recovers_after_peer_restart.summary.json | 0xeaf2198957... | 60 | 3 | 3340 | 3620 | 3.16 | yes | 4 | 168 | 168 | 19 | 0 |
