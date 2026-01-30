---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: pq-ratchet-runbook
title: Simulation PQ Ratchet SoraNet
sidebar_label: Runbook PQ Ratchet
description: Etapes de rehearsal on-call pour promouvoir ou retrograder la politique d'anonymat PQ et valider la telemetrie deterministe.
---

:::note Source canonique
Cette page reflete `docs/source/soranet/pq_ratchet_runbook.md`. Gardez les deux copies alignees jusqu'a la retraite de l'ancien set de documentation.
:::

## Objectif

Ce runbook guide la sequence de fire-drill pour la politique d'anonymat post-quantum (PQ) etagee de SoraNet. Les operateurs repetent la promotion (Stage A -> Stage B -> Stage C) ainsi que la retrogradation controlee vers Stage B/A quand l'offre PQ baisse. Le drill valide les hooks de telemetrie (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) et collecte les artefacts pour le log de rehearsal d'incident.

## Prerequis

- Dernier binaire `sorafs_orchestrator` avec capability-weighting (commit egal ou posterieur a la reference du drill dans `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Acces au stack Prometheus/Grafana qui sert `dashboards/grafana/soranet_pq_ratchet.json`.
- Snapshot nominal du guard directory. Recuperez et verifiez une copie avant le drill:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Si le source directory ne publie que du JSON, re-encodez-le en binaire Norito avec `soranet-directory build` avant d'executer les helpers de rotation.

- Capturez les metadata et pre-stagez les artefacts de rotation de l'issuer avec le CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Fenetre de changement approuvee par les equipes on-call networking et observability.

## Etapes de promotion

1. **Stage audit**

   Enregistrez le stage de depart:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Attendez `anon-guard-pq` avant promotion.

2. **Promotion vers Stage B (Majority PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Attendez >=5 minutes pour que les manifests se rafraichissent.
   - Dans Grafana (dashboard `SoraNet PQ Ratchet Drill`) confirmez que le panneau "Policy Events" affiche `outcome=met` pour `stage=anon-majority-pq`.
   - Capturez un screenshot ou le JSON du panneau et attachez-le au log d'incident.

3. **Promotion vers Stage C (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Verifiez que les histogrammes `sorafs_orchestrator_pq_ratio_*` tendent vers 1.0.
   - Confirmez que le compteur brownout reste plat; sinon suivez les etapes de retrogradation.

## Drill de retrogradation / brownout

1. **Induire une penurie PQ synthetique**

   Desactivez les relays PQ dans l'environnement playground en taillant le guard directory aux seules entrees classiques, puis rechargez le cache orchestrator:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observer la telemetrie brownout**

   - Dashboard: le panneau "Brownout Rate" monte au-dessus de 0.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` doit reporter `anonymity_outcome="brownout"` avec `anonymity_reason="missing_majority_pq"`.

3. **Retrograder vers Stage B / Stage A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Si l'offre PQ reste insuffisante, retrogradez vers `anon-guard-pq`. Le drill se termine quand les compteurs brownout se stabilisent et que les promotions peuvent etre reappliquees.

4. **Restaurer le guard directory**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetrie et artefacts

- **Dashboard:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertes Prometheus:** assurez-vous que l'alerte brownout pour `sorafs_orchestrator_policy_events_total` reste sous le SLO configure (&lt;5% sur toute fenetre de 10 minutes).
- **Incident log:** ajoutez les snippets de telemetrie et les notes operateur a `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Capture signee:** utilisez `cargo xtask soranet-rollout-capture` pour copier le drill log et le scoreboard dans `artifacts/soranet_pq_rollout/<timestamp>/`, calculer les digests BLAKE3 et produire un `rollout_capture.json` signe.

Exemple:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Joignez les metadata generees et la signature au dossier governance.

## Rollback

Si le drill revele une veritable penurie PQ, restez sur Stage A, notifiez le Networking TL et attachez les metriques collectees ainsi que les diffs du guard directory au tracker d'incident. Utilisez l'export du guard directory capture plus tot pour restaurer le service normal.

:::tip Couverture de regression
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` fournit la validation synthetique qui soutient ce drill.
:::
