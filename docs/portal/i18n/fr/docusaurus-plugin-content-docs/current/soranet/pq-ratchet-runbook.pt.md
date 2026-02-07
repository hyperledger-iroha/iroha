---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-ratchet-runbook
titre : Simulacro PQ Ratchet de SoraNet
sidebar_label : Runbook de PQ Ratchet
description: Passos de ensaio on call para promover ou rebaixar a politica de anonimato PQ em estagis com validacao deterministica de telemetria.
---

:::note Fonte canonica
Cette page espelha `docs/source/soranet/pq_ratchet_runbook.md`. Mantenha ambas comme copies synchronisées.
:::

## Proposition

Ce runbook guide la séquence de simulation pour la politique anonymisée post-quantique (PQ) dans les étapes de SoraNet. Les opérateurs effectuent la promotion (Étape A -> Étape B -> Étape C) et la promotion est contrôlée en fonction de l'étape B/A lorsqu'une offre de PQ est disponible. Les crochets de télémétrie simulés valides (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) et les artéfacts pour le journal de répétition des incidents.

## Prérequis

- Ultimo binario `sorafs_orchestrator` avec pondération des capacités (commit igual ou postérieur à la référence de forage affiché dans `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Accès à la pile Prometheus/Grafana qui sert `dashboards/grafana/soranet_pq_ratchet.json`.
- Instantané nominal du répertoire de garde. Vérifiez et validez une copie avant le simulation :

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Voir le répertoire source publier comme JSON, ré-encoder pour le binaire Norito avec `soranet-directory build` avant de démarrer les assistants de rotation.

- Capturer les métadonnées et les artéfacts de rotation préalables de l'émetteur avec la CLI :

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Janela de mudanca aprovada pelos time on call de networking et observabilité.

## Passos de promotion

1. **Audit d'étape**

   Registre ou étape initiale :

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Attendez `anon-guard-pq` avant la promotion.

2. **Promova para Stage B (PQ majoritaire)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Aguarde >=5 minutos para manifestes serem atualizados.
   - No Grafana (tableau de bord `SoraNet PQ Ratchet Drill`) confirme que le tableau "Événements de politique" affiche `outcome=met` pour `stage=anon-majority-pq`.
   - Capturez une capture d'écran ou un fichier JSON et une annexe au journal des incidents.

3. **Promova para Stage C (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Vérifiez que les histogrammes `sorafs_orchestrator_pq_ratio_*` tendent vers 1.0.
   - Confirmez que le contacteur de baisse de tension est permanent ; cas contraire, siga os passos de despromocao.

## Despromocao / forage de baisse de tension

1. **Induza uma escassez sintetica de PQ**

   Relais désatifs PQ dans le terrain de jeu ambiant réduisant le répertoire de garde aux entrées classiques, après avoir rechargé le cache de l'orchestrateur :

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observez une télémétrie de baisse de tension**

   - Tableau de bord : le tableau "Brownout Rate" est affiché à 0.
   - PromQL : `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` a développé le rapport `anonymity_outcome="brownout"` avec `anonymity_reason="missing_majority_pq"`.

3. **Despromova pour Stade B / Stade A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Voir une offre de PQ également pour insuffisante, despromova pour `anon-guard-pq`. La simulation se termine lorsque les contrôleurs de baisse de tension s'établissent et que les promotions peuvent être réappliquées.

4. **Restauration du répertoire de garde**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Télémétrie et artéfacts- **Tableau de bord :** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertes Prometheus :** garantit que l'alerte de baisse de tension de `sorafs_orchestrator_policy_events_total` est abaissée par le SLO configuré (<5 % pendant toute une semaine de 10 minutes).
- **Journal des incidents :** annexes de télémétrie et notes de l'opérateur sur `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Capture perdue :** utilisez `cargo xtask soranet-rollout-capture` pour copier le journal d'exercices et le tableau de bord pour `artifacts/soranet_pq_rollout/<timestamp>/`, les résumés calculés BLAKE3 et produire un `rollout_capture.json` supprimé.

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

Anexe os metadata gerados et assinatura o pacte de gouvernance.

## Restauration

Pour que le simulacre révèle une évasion réelle de PQ, en permanence à l'étape A, notifiez le réseau TL et l'annexe en tant que mesures collectées conjointement avec les différences dans le répertoire de garde du suivi des incidents. Utilisez o export do guard directory capturé antérieurement pour restaurer le service normal.

:::tip Couverture de régression
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` fournit une validation synthétique qui supporte ce simulacre.
:::