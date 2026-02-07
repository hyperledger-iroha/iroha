---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-ratchet-runbook
titre : Exercice d'incendie à cliquet SoraNet PQ
sidebar_label : Runbook PQ Ratchet
la description: مرحلہ وار PQ politique d'anonymat کو promouvoir یا rétrograder کرنے کے لئے les étapes de répétition de garde et اور validation télémétrique déterministe.
---

:::note Source canonique
یہ صفحہ `docs/source/soranet/pq_ratchet_runbook.md` کی عکاسی کرتا ہے۔ جب تک پرانا documentation set retiré نہ ہو، دونوں کاپیاں رکھیں۔
:::

## مقصد

Le runbook SoraNet et la politique d'anonymat post-quantique (PQ) par étapes ainsi que la séquence d'exercices d'incendie sont en cours. Promotion des opérateurs (Étape A -> Étape B -> Étape C) pour l'approvisionnement du PQ en cas de rétrogradation contrôlée et pour l'étape B/A pour les répétitions Les crochets de télémétrie de forage (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) valident le journal de répétition de l'incident et les artefacts.

## Prérequis

- Pondération de capacité en utilisant le binaire `sorafs_orchestrator` (référence de forage de validation en anglais) ہے)۔
- La pile Prometheus/Grafana est utilisée pour fonctionner et `dashboards/grafana/soranet_pq_ratchet.json` sert de pile
- Instantané du répertoire de garde nominal۔ percer et copier, récupérer et vérifier:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Ajouter le répertoire source à la publication JSON et aux aides à la rotation pour le réencodage binaire `soranet-directory build` et Norito

- Capture de métadonnées CLI et artefacts de rotation des émetteurs avant l'étape :

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Mise en réseau et observabilité des équipes d'astreinte et fenêtre de changement

## Étapes de la promotion

1. **Audit d'étape**

   ابتدا کا stage ریکارڈ کریں:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Promotion سے پہلے `anon-guard-pq` attendre کریں۔

2. **Étape B (PQ majoritaire) pour promouvoir le کریں**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Actualisation des manifestes ہونے کے لئے >=5 منٹ انتظار کریں۔
   - Grafana (tableau de bord `SoraNet PQ Ratchet Drill`) Panneau « Événements de stratégie » pour `stage=anon-majority-pq` pour `outcome=met` pour `outcome=met`.
   - Capture d'écran du panneau JSON capture et journal des incidents et pièce jointe

3. **Étape C (PQ strict) pour promouvoir la santé**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Histogrammes `sorafs_orchestrator_pq_ratio_*` version 1.0 pour vérifier la version
   - Compteur de baisse de tension à plat, confirmation ورنہ étapes de rétrogradation فالو کریں۔

## Exercice de rétrogradation / baisse de tension

1. **Pénurie de PQ synthétique ici**

   Environnement de terrain de jeu, répertoire de garde, entrées classiques, garniture, relais PQ désactivés, rechargement du cache de l'orchestrateur, :

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **La télémétrie des baisses de tension observe کریں**

   - Tableau de bord : panneau "Brownout Rate" 0 et pic de tension
   - PromQL : `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` et `anonymity_outcome="brownout"` et `anonymity_reason="missing_majority_pq"` pour votre téléphone

3. **Étape B / Étape A ou rétrogradation**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   L'approvisionnement du PQ est en cours de réalisation et le `anon-guard-pq` est rétrogradé. Percez les compteurs de baisse de tension et réglez les promotions et les promotions

4. **Répertoire de garde pour les utilisateurs**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Télémétrie et artefacts- **Tableau de bord :** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertes Prometheus :** Alerte de baisse de tension `sorafs_orchestrator_policy_events_total` configurée SLO en temps réel (<5 % par fenêtre de 10 minutes)
- **Journal des incidents :** extraits de télémétrie et notes de l'opérateur comme `docs/examples/soranet_pq_ratchet_fire_drill.log` et ajouter ici
- **Capture signée :** `cargo xtask soranet-rollout-capture` copie du journal de forage et du tableau de bord `artifacts/soranet_pq_rollout/<timestamp>/` copie du journal de forage et du tableau de bord BLAKE3 résume le calcul et est signé `rollout_capture.json` Français

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

Métadonnées générées et signature et paquet de gouvernance et pièces jointes

## Restauration

Il s'agit d'un exercice de forage pour la pénurie de PQ et d'un stage A pour Networking TL et d'un système de suivi des incidents, ainsi que des métriques collectées, des différences dans le répertoire de garde et du suivi des incidents. کریں۔ Capturer le répertoire de garde, exporter, restaurer le service normal

:::tip Couverture de régression
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` forage et support pour la validation synthétique
:::