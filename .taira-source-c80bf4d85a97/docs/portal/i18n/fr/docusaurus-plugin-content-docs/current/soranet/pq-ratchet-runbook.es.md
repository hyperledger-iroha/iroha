---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-ratchet-runbook
titre : Simulacro de PQ Ratchet de SoraNet
sidebar_label : Runbook de PQ Ratchet
description : Pasos de ensayo para guardia al promover o degradar la politica de anonimato PQ escalonada con validacion de telemetria determinista.
---

:::note Fuente canonica
Cette page reflète `docs/source/soranet/pq_ratchet_runbook.md`. Manten ambas copias sincronizadas.
:::

## Proposition

Ce runbook guide la sécurité du simulacre pour la politique anonymisée post-quantique (PQ) augmentée par SoraNet. Les opérateurs essaient de promouvoir la promotion (Étape A -> Étape B -> Étape C) comme la dégradation contrôlée du retour à l'étape B/A lorsque l'approvisionnement en PQ est effectué. La simulation valide les crochets de télémétrie (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) et recole des artefacts pour le journal de répétition des incidents.

## Prérequis

- Dernier binaire `sorafs_orchestrator` avec pondération de capacité (commit en ou après la référence du simulacre affiché en `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Accès à la pile de Prometheus/Grafana qui correspond à `dashboards/grafana/soranet_pq_ratchet.json`.
- Répertoire nominal del guard instantané. Effectuez et vérifiez une copie avant le simulation :

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Si le répertoire source n'est public que JSON, réencodez le binaire Norito avec `soranet-directory build` avant d'exécuter les assistants de rotation.

- Capturer les métadonnées et les artefacts préliminaires de rotation de l'émetteur avec la CLI :

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Ventana de changement approuvé pour les équipes de mise en réseau et d'observabilité.

## Pas de promotion

1. **Audit d'étape**

   Inscription initiale de la scène :

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Espera `anon-guard-pq` avant la promotion.

2. **Promotion à l'étape B (PQ majoritaire)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Espera >=5 minutes pour que les manifestes se renouvellent.
   - En Grafana (tableau de bord `SoraNet PQ Ratchet Drill`) confirme que le panneau "Événements de stratégie" doit être `outcome=met` pour `stage=anon-majority-pq`.
   - Capturez une capture d'écran du panneau JSON et complétez le journal des incidents.

3. **Promotion de l'étape C (PQ strict)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Vérifiez que les histogrammes `sorafs_orchestrator_pq_ratio_*` sont à 1.0.
   - Confirmez que le contacteur de baisse de tension est permanent ; si non, suivez les étapes de dégradation.

## Simulation de dégradation / baisse de tension

1. **Induire un échappement synthétique de PQ**

   Déshabilitation des relais PQ sur l'entorno de terrain de jeu enregistrant le répertoire de garde pour les entrées classiques seulement, puis rechargez le cache de l'orchestrateur :

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observez la télémétrie de baisse de tension**

   - Tableau de bord : le panneau "Brownout Rate" est mis à 0.
   - PromQL : `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` doit être rapporté `anonymity_outcome="brownout"` avec `anonymity_reason="missing_majority_pq"`.

3. **Dégradation au stade B / stade A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Si l'alimentation PQ est insuffisante, dégrada a `anon-guard-pq`. La simulation se termine lorsque les contrôleurs de baisse de tension sont établis et les promotions peuvent être réappliquées.

4. **Répertoire Restaura el guard**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Télémétrie et artefacts- **Tableau de bord :** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertes Prometheus :** assurez-vous que l'alerte de baisse de tension de `sorafs_orchestrator_policy_events_total` est maintenue sous le SLO configuré (<5 % pendant 10 minutes).
- **Journal des incidents :** ajoute les extraits de télémétrie et les notes de l'opérateur à `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Capture d'entreprise :** utilisez `cargo xtask soranet-rollout-capture` pour copier le journal de forage et le tableau de bord en `artifacts/soranet_pq_rollout/<timestamp>/`, les résumés calculés BLAKE3 et produire un `rollout_capture.json` firmado.

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

Adjoindre les métadonnées générées et la société au paquet de gouvernance.

## Restauration

Si le simulateur découvre le véritable PQ, en permanence à l'étape A, notifie le Networking TL et ajoute les mesures recueillies avec les différences du répertoire de garde au suivi des incidents. Utilisez le répertoire d'exportation du garde capturé antérieurement pour restaurer le service normal.

:::tip Couverture de régression
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` fournit la validation synthétique qui répond à ce simulacre.
:::