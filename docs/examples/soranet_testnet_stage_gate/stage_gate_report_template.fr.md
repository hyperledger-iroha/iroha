---
lang: fr
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-11-07T11:07:58.852037+00:00"
translation_last_reviewed: 2026-01-01
---

# Rapport stage-gate SNNet-10 (T?_ -> T?_)

> Remplacez chaque placeholder (elements entre < >) avant soumission. Conservez les en-tetes de section pour que l'automatisation governance puisse parser le fichier.

## 1. Metadonnees

| Champ | Valeur |
|-------|-------|
| Promotion | `<T0->T1 ou T1->T2>` |
| Fenetre de reporting | `<YYYY-MM-DD -> YYYY-MM-DD>` |
| Relays dans le scope | `<count + IDs ou "voir annexe A">` |
| Contact principal | `<name/email/Matrix handle>` |
| Archive de soumission | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| Archive SHA-256 | `<sha256:...>` |

## 2. Resume des metriques

| Metrique | Observe | Seuil | Passe? | Source |
|--------|----------|-------|-------|--------|
| Ratio de succes des circuits | `<0.000>` | >=0.95 | N / Y | `reports/metrics-report.json` |
| Ratio brownout fetch | `<0.000>` | <=0.01 | N / Y | `reports/metrics-report.json` |
| Variance du mix GAR | `<+0.0%>` | <=+/-10% | N / Y | `reports/metrics-report.json` |
| PoW p95 en secondes | `<0.0 s>` | <=3 s | N / Y | `telemetry/pow_window.json` |
| Latence p95 | `<0 ms>` | <200 ms | N / Y | `telemetry/latency_window.json` |
| Ratio PQ (moyenne) | `<0.00>` | >= target | N / Y | `telemetry/pq_summary.json` |

**Narratif:** `<summaries of anomalies, mitigations, overrides>`

## 3. Journal drill & incident

| Timestamp (UTC) | Region | Type | Alert ID | Resume mitigation |
|-----------------|--------|------|----------|-------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. Pieces jointes et hashes

| Artefact | Chemin | SHA-256 |
|----------|-------|---------|
| Snapshot metriques | `reports/metrics-window.json` | `<sha256>` |
| Rapport metriques | `reports/metrics-report.json` | `<sha256>` |
| Transcriptions guard rotation | `evidence/guard_rotation/*.log` | `<sha256>` |
| Manifests exit bonding | `evidence/exit_bonds/*.to` | `<sha256>` |
| Logs drill | `evidence/drills/*.md` | `<sha256>` |
| MASQUE readiness (T1->T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| Plan rollback (T1->T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. Approbations

| Role | Nom | Signe (O/N) | Notes |
|------|-----|-------------|-------|
| Networking TL | `<name>` | O / N | `<comments>` |
| Governance rep | `<name>` | O / N | `<comments>` |
| SRE delegate | `<name>` | O / N | `<comments>` |

## Annexe A - Liste des relays

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## Annexe B - Resumes d'incidents

```
<Detailed context for any incidents or overrides referenced above.>
```
