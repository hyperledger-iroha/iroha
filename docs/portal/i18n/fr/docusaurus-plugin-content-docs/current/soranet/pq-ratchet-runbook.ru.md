---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-ratchet-runbook
titre : Учебная тревога PQ Ratchet SoraNet
sidebar_label : Runbook PQ Ratchet
description: Шаги répétition de garde для повышения или понижения стадийной Politique d'anonymat du PQ avec validation de la télémétrie.
---

:::note Канонический источник
Cette page indique `docs/source/soranet/pq_ratchet_runbook.md`. Vous pouvez obtenir des copies synchronisées si vous souhaitez télécharger des documents, mais vous ne pouvez pas les utiliser.
:::

## Назначение

Ce runbook propose un exercice d'exercice d'incendie pour la politique d'anonymat post-quantique (PQ) de SoraNet. Les opérateurs effectuent la promotion (étape A -> étape B -> étape C), ainsi que la rétrogradation contrôlée de l'étape B/A lors de la fourniture de PQ. Percez les crochets de télémétrie (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) et enregistrez les artefacts pour le journal de répétition des incidents.

## Prérequis

- Il s'agit également de la pondération binaire des capacités `sorafs_orchestrator` (validation ou exercice de référence par `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Installez la pile Prometheus/Grafana pour obtenir `dashboards/grafana/soranet_pq_ratchet.json`.
- Instantané du répertoire de garde nominal. Veuillez et vérifiez la copie de l'exercice :

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Si le répertoire source est publié en JSON, placez-le dans le binaire Norito en utilisant `soranet-directory build` avant d'installer les assistants de rotation.

- Ajouter les métadonnées et les artefacts de pré-étape à l'émetteur via la CLI :

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Changer la fenêtre de mise en réseau et d'observabilité des commandes d'astreinte.

## Étapes de la promotion

1. **Audit d'étape**

   Зафиксируйте стартовый étape:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Avant la promotion, veuillez contacter `anon-guard-pq`.

2. **Promotion à l'étape B (PQ majoritaire)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Подождите >=5 minutes для обновления manifestes.
   - Dans Grafana (tableau de bord `SoraNet PQ Ratchet Drill`), indiquez que le panneau "Événements de stratégie" indique `outcome=met` pour `stage=anon-majority-pq`.
   - Prenez une capture d'écran ou des panneaux JSON et ajoutez le journal des incidents.

3. **Promotion à l'étape C (PQ strict)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Vérifiez que l'histogramme `sorafs_orchestrator_pq_ratio_*` est disponible sur 1.0.
   - Убедитесь, что brownout counter остается плоским ; иначе выполните шаги rétrogradation.

## Exercice de rétrogradation / baisse de tension

1. **Индуцируйте синтетический дефицит PQ**

   Ouvrez les relais PQ dans l'aire de jeux, accédez au répertoire de garde des entrées classiques et ouvrez le cache de l'orchestrateur :

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Activer la baisse de tension de télémétrie**

   - Tableau de bord : le panneau "Brownout Rate" correspond à 0.
   - PromQL : `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` doit correspondre à `anonymity_outcome="brownout"` avec `anonymity_reason="missing_majority_pq"`.

3. **Rétrogradation vers l'étape B / l'étape A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Si vous fournissez PQ, vous devez contacter `anon-guard-pq`. Les forets sont utilisés, les compteurs de baisses de tension se stabilisent et les promotions peuvent être prises en compte.

4. **Répertoire de garde de Восстановление**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Télémétrie et artefacts- **Tableau de bord :** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertes Prometheus :** activez l'alerte de baisse de tension `sorafs_orchestrator_policy_events_total` qui indique le SLO le plus élevé (<5 % pendant 10 minutes).
- **Journal des incidents :** permet d'utiliser des extraits de télémétrie et des informations sur l'opérateur dans `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Capture signée :** utilisez `cargo xtask soranet-rollout-capture` pour copier le journal de forage et le tableau de bord dans `artifacts/soranet_pq_rollout/<timestamp>/`, consultez les résumés BLAKE3 et enregistrez les résultats. `rollout_capture.json`.

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

Utilisez des métadonnées et des signatures générées pour la gouvernance des paquets.

## Restauration

Si l'exercice révèle un véritable PQ, installez-vous à l'étape A, étudiez Networking TL et utilisez les métriques associées aux différences du répertoire de garde et au suivi des incidents. Utilisez le programme d'exportation du répertoire Guard pour bénéficier d'un service normal.

:::tip Couverture de régression
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` est une validation synthétique qui permet de réaliser cette perceuse.
:::