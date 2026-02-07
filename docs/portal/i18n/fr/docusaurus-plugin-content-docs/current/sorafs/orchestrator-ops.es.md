---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : orchestrator-ops
titre : Runbook des opérations de l'orchestre de SoraFS
sidebar_label : Runbook de l'orchestre
description : Guide opérationnel pas à pas pour désemplir, superviser et rétablir l'orquestador multi-origen.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Il s'agit de copies synchronisées jusqu'à ce que le ensemble de documents Sphinx hérité soit migré complètement.
:::

Ce runbook guide le SRE pour la préparation, le déroulement et l'opération de l'explorateur de récupération multi-origine. Complète le guide de développement avec des procédures ajustées pour les tâches de production, y compris l'habilitation pour les étapes et le blocage des pairs.

> **Voir aussi :** Le [Runbook de despliegue multi-origen](./multi-source-rollout.md) est centré sur les oleadas de despliegue au niveau de flottaison et sur la dénégation des fournisseurs en urgence. Conseiller pour la coordination de la gouvernance / de la mise en scène pendant que nous utilisons ce document pour les opérations du journal de l'orchestre.

## 1. Liste de vérification préalable

1. **Recopilar entradas de provenedores**
   - Les dernières annonces des fournisseurs (`ProviderAdvertV1`) et la télémétrie instantanée de la flottaison objet.
   - Le plan de charge utile (`plan.json`) dérivé du manifeste en bas de l'essai.
2. **Générer un tableau de bord déterministe**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - Valida que `artifacts/scoreboard.json` liste chaque fournisseur de production comme `eligible`.
   - Archivage du JSON de reprise avec le tableau de bord ; les auditeurs sont apoyan par les contadores de reintentos de chunks al certificar la sollicitud de change.
3. **Dry-run con luminaires** — Effectuez la même commande contre les luminaires publics en `docs/examples/sorafs_ci_sample/` pour garantir que le binaire de l'explorateur coïncide avec la version attendue avant de charger les charges utiles de production.

## 2. Procédure d'exécution par étape

1. **Etapa canaria (≤2 fournisseurs)**
   - Reconstruire le tableau de bord et exécuter avec `--max-peers=2` pour limiter l'orchestre à un sous-ensemble petit.
   - Supervise :
     -`sorafs_orchestrator_active_fetches`
     -`sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     -`sorafs_orchestrator_retries_total`
   - Continuez lorsque les poches de réintention se maintiennent en bajo du 1% pour une récupération complète du manifeste et n'importe quel fournisseur cumule les chutes.
2. **Etapa de rampa (50% des fournisseurs)**
   - Incrémentez `--max-peers` et vuelve à exécuter avec une télémétrie instantanée récente.
   - Continuer chaque exécution avec `--provider-metrics-out` et `--chunk-receipts-out`. Conserver les objets pendant ≥7 jours.
3. **Despligue completo**
   - Elimina `--max-peers` (la configuration du dossier complet des éléments éligibles).
   - Habiliter le mode explorateur chez les clients clients : distribuer le tableau de bord persistant et le JSON de configuration à travers votre système de gestion de configuration.
   - Actualiser les tableaux de bord pour afficher `sorafs_orchestrator_fetch_duration_ms` p95/p99 et les histogrammes de réintention par région.

## 3. Blocage et renvoi des pairsUtilisez les annonces politiques de pointage de la CLI pour classifier les fournisseurs non saludables sans attendre les mises à jour du gouvernement.

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```

- `--deny-provider` supprimer l'alias indiqué pour la considération dans la session actuelle.
- `--boost-provider=<alias>=<weight>` augmente le poids du planificateur du fournisseur. Les valeurs sont exprimées en poids normalisé du tableau de bord et s'appliquent uniquement à l'exécution locale.
- Enregistrez les annonces sur le ticket d'incident et ajoutez les sorties JSON pour que l'équipe responsable puisse concilier l'état une fois qu'il corrija le problème subyacente.

Pour des changements permanents, modifiez la télémétrie d'origine (marque du contrevenant comme pénalisé) ou actualisez l'annonce avec des présupposés de flux actualisés avant de nettoyer les annonces de la CLI.

## 4. Diagnostic des erreurs

Lorsque vous récupérez l'erreur :

1. Capturez les objets suivants avant de retourner à l'exécution :
   -`scoreboard.json`
   -`session.summary.json`
   -`chunk_receipts.json`
   -`provider_metrics.json`
2. Inspecciona `session.summary.json` pour voir la chaîne d'erreur lisible :
   - `no providers were supplied` → vérifier les itinéraires des fournisseurs et les annonces.
   - `retry budget exhausted ...` → incrémenta `--retry-budget` ou élimination des pairs inestables.
   - `no compatible providers available ...` → audita la metainformación de capacidad de rango del provenor infractor.
3. Corrélez le numéro du fournisseur avec `sorafs_orchestrator_provider_failures_total` et créez un ticket de suivi si la métrique disparaît.
4. Reproduire l'obtención sans connexion avec `--scoreboard-json` et la télémétrie capturée pour reproduire la chute de forme déterministe.

## 5. Réversion

Pour revenir à un jeu de l'orchestre :

1. Distribuez une configuration établie par `--max-peers=1` (déshabilitation de la planification multi-origine) ou vuelve aux clients sur la route de récupération héritée d'une seule origine.
2. Éliminez toute annulation `--boost-provider` pour que le tableau de bord donne un poids neutre.
3. Continuez à recueillir les données de l'orchestre au cours de la journée pour confirmer que rien n'est récupéré en vue.

Maintenir une capture disciplinée d'artefacts et d'expéditions pendant des périodes garantit que l'orquesteur multi-origine pourra opérer de manière sûre sur des flottes hétérogènes de fournisseurs pendant qu'il maintient les exigences d'observation et d'auditoire.