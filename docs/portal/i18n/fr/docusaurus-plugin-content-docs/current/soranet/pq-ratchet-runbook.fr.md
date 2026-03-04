---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-ratchet-runbook
titre : Simulation PQ Ratchet SoraNet
sidebar_label : Runbook PQ Ratchet
description : Etapes de répétition d'astreinte pour promouvoir ou rétrograder la politique d'anonymat PQ et valider la télémétrie déterministe.
---

:::note Source canonique
Cette page reflète `docs/source/soranet/pq_ratchet_runbook.md`. Gardez les deux copies alignées jusqu'à la retraite de l'ancien ensemble de documentation.
:::

## Objectif

Ce runbook guide la séquence de fire-drill pour la politique d'anonymat post-quantique (PQ) étape par SoraNet. Les opérateurs répètent la promotion (Stage A -> Stage B -> Stage C) ainsi que la rétrogradation contrôlée vers Stage B/A lorsque l'offre PQ baisse. Le foret valide les crochets de télémétrie (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) et collecte les artefacts pour le journal de répétition d'incident.

## Prérequis

- Dernier binaire `sorafs_orchestrator` avec capability-weighting (commit égal ou postérieur à la référence du forage dans `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Accès au stack Prometheus/Grafana qui sert `dashboards/grafana/soranet_pq_ratchet.json`.
- Snapshot nominal du répertoire de garde. Récupérez et vérifiez une copie avant le forage :

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Si le répertoire source ne publie que du JSON, ré-encodez-le en binaire Norito avec `soranet-directory build` avant d'exécuter les helpers de rotation.

- Capturez les métadonnées et pré-stagez les artefacts de rotation de l'émetteur avec le CLI :

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Fenêtre de changement approuvée par les équipes de mise en réseau et d'observabilité d'astreinte.

## Étapes de promotion

1. **Audit d'étape**

   enregistrer l'étape de départ:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Attendez `anon-guard-pq` avant promotion.

2. **Promotion vers l'étape B (PQ majoritaire)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Attendez >=5 minutes pour que les manifestes se rafraichissent.
   - Dans Grafana (dashboard `SoraNet PQ Ratchet Drill`) confirmez que le panneau "Policy Events" affiche `outcome=met` pour `stage=anon-majority-pq`.
   - Capturez une capture d'écran ou le JSON du panneau et attachez-le au journal d'incident.

3. **Promotion vers Stage C (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Vérifiez que les histogrammes `sorafs_orchestrator_pq_ratio_*` tendent vers 1.0.
   - Confirmez que le compteur baisse de tension reste plat; sinon suivez les étapes de rétrogradation.

## Forage de rétrogradation / baisse de tension

1. **Induire une penurie PQ synthétique**

   Désactivez les relais PQ dans l'environnement terrain de jeu en taillant le répertoire de garde aux seules entrées classiques, puis rechargez le cache orchestrator :

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observer la baisse de tension de la télémétrie**

   - Tableau de bord : le panneau "Brownout Rate" monte au-dessus de 0.
   - PromQL : `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` doit reporter `anonymity_outcome="brownout"` avec `anonymity_reason="missing_majority_pq"`.

3. **Rétrograder vers Stage B / Stage A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Si l'offre PQ reste insuffisante, rétrogradez vers `anon-guard-pq`. Le forage se termine quand les compteurs baisse de tension se stabilisent et que les promotions peuvent être réappliquées.

4. **Annuaire Restaurateur le guard**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Télémétrie et artefacts- **Tableau de bord :** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertes Prometheus :** Assurez-vous que l'alerte de baisse de tension pour `sorafs_orchestrator_policy_events_total` reste sous le SLO configuré (<5% sur toute la fenêtre de 10 minutes).
- **Journal des incidents :** ajoute les extraits de télémétrie et les notes opérateur à `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Capture signee :**utilisez `cargo xtask soranet-rollout-capture` pour copier le drill log et le scoreboard dans `artifacts/soranet_pq_rollout/<timestamp>/`, calculer les digests BLAKE3 et produire un signe `rollout_capture.json`.

Exemple :

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Joignez les métadonnées générées et la signature au dossier gouvernance.

## Restauration

Si l'exercice révèle une véritable pénurie PQ, restez sur Stage A, notifiez le Networking TL et attachez les métriques collectées ainsi que les diffs du guard directory au tracker d'incident. Utilisez l'export du guard directory capture plus pour restaurer le service normal.

:::tip Couverture de régression
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` fournit la validation synthétique qui soutient ce foret.
:::