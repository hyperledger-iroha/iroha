---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : déploiement multi-source
titre : Runbook de déploiement multi-origem et refus des fournisseurs
sidebar_label : Runbook de déploiement multi-origem
description : Liste de contrôle opérationnelle pour les déploiements multi-originaux en phases et en cas d'urgence des fournisseurs.
---

:::note Fonte canônica
Cette page s'affiche `docs/source/sorafs/runbooks/multi_source_rollout.md`. Mantenha ambas comme copies synchronisées.
:::

## Objet

Ce runbook oriente les SRE et les ingénieurs de plantation avec deux flux critiques :

1. Fazer o rollout do orquestrador multi-origem em ondas controladas.
2. Negar ou depriorizar provenores com mau comportamento sem desestabilizar sessões existentes.

Appuyez sur le fait que le pilha d'orquestration entre SF-6 est déjà implanté (`sorafs_orchestrator`, API d'intervalle de fragments de la passerelle, exportateurs de télémétrie).

> **Voir aussi :** Le [Runbook des opérations de l'explorateur](./orchestrator-ops.md) approfondit les procédures d'exécution (capture du tableau de bord, bascules de déploiement dans les phases, restauration). Utilisez des ambas comme références dans le cadre des situations vécues.

## 1. Validation préalable1. **Confirmer les assurances de gouvernance.**
   - Tous les candidats candidats ont développé des enveloppes publiques `ProviderAdvertV1` avec des charges utiles de capacité d'intervalle et d'ornements de flux. Validez via `/v1/sorafs/providers` et comparez avec les champs de capacité des espérés.
   - Des instantanés de télémétrie qui fournissent des taxes de latence/falha durent moins de 15 minutes avant chaque exécution canária.
2. **Préparer une configuration.**
   - Conserver la configuration JSON de l'explorateur pour l'enregistrer dans les champs `iroha_config` :

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Actualisez le JSON avec les limites spécifiques au déploiement (`max_providers`, budgets de nouvelle tentative). Utilisez votre fichier d'archives dans la mise en scène/production pour le mode de vie selon des différences minimales.
3. **Exercitar luminaires canônicas.**
   - Utiliser les variables ambiantes du manifeste/jeton et exécuter ou récupérer de manière déterminante :

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     En tant que variables ambiantes, nous développons le résumé de la charge utile du manifeste (hex) et des jetons de flux codifiés en base64 pour chaque fournisseur de Canary.
   - Comparez `artifacts/canary.scoreboard.json` avec la version antérieure. Tout nouveau fournisseur inelegível ou mudança de peso >10% exige une révision.
4. **Vérifiez que la télémétrie est connectée.**
   - Ouvrez l'exportation du Grafana vers `docs/examples/sorafs_fetch_dashboard.json`. Garanta que les métriques `sorafs_orchestrator_*` apparaîtront dans la mise en scène avant d'avancer.

## 2. Négation émergente des fournisseursSuivez cette procédure lorsqu'un fournisseur utilise des morceaux corrompus, qu'il y a des délais d'attente de forme persistante ou qu'ils échouent dans les vérifications de conformité.1. **Capturer les preuves.**
   - Exporter le résumé de récupération le plus récent (saída de `--json-out`). Enregistrez les indices des morceaux avec les faux, les alias des fournisseurs et les divergences du résumé.
   - Salve trechos de log relevantes dos cibles `telemetry::sorafs.fetch.*`.
2. **Appliquer le remplacement immédiatement.**
   - Marquer le fournisseur comme pénalisé par aucun instantané de télémétrie distribué à l'explorateur (définir `penalty=true` ou limiter `token_health` à `0`). La construction prochaine du tableau de bord exclura ou prouvera automatiquement.
   - Pour des tests de fumée ad hoc, passez `--deny-provider gw-alpha` à `sorafs_cli fetch` pour exercer le chemin de fausse route en attendant la propagation de la télémétrie.
   - Réimplantation du bundle actualisé de télémétrie/configuration dans une ambiance adaptée (staging → canary → production). Documentez la situation dans aucun journal d'incident.
3. **Valider ou remplacer.**
   - Réexécutez la récupération du luminaire canônico. Confirmez que le tableau de bord marque le fournisseur comme inlégif avec le motif `policy_denied`.
   - Inspecione `sorafs_orchestrator_provider_failures_total` pour garantir que le contacteur pare-piste d'augmentation pour le fournisseur négatif.
4. **Escalar banimentos prolongados.**
   - Si le fournisseur est bloqué en permanence pendant >24 h, ouvrez un ticket de gouvernement pour tourner ou suspendre votre publicité. Après avoir voté, maintenez la liste de refus et actualisez les instantanés de télémétrie pour que le fournisseur ne retourne pas au tableau de bord.
5. **Protocole de restauration.**- Pour réintégrer le fournisseur, supprimer la liste des négatifs, réimplanter et capturer un nouvel instantané du tableau de bord. Anexe a mudança ao postmortem do incidente.

## 3. Plan de déploiement par phases

| Phase | Escopo | Sinais obrigatoires | Critères Go/No-Go |
|------|--------|-----------|--------------------|
| **Labo** | Cluster d'intégration dédié | Récupérer le manuel de la CLI pour les charges utiles des appareils | Tous les morceaux sont conclus, les vendeurs de faux fournisseurs ont 0, les taxons de tentatives < 5 %. |
| **Mise en scène** | Staging complet du plan de contrôle | Le tableau de bord est connecté à Grafana ; regras d'alerte en mode quelque avertissement | `sorafs_orchestrator_active_fetches` passe à zéro après chaque exécution du test ; nenhum alerta `warn/critical`. |
| **Canari** | ≤10% du trafic de production | Pager silencieux mais télémétrique surveillé à un rythme réel | Taux de tentatives < 10 %, erreurs de preuves isolées par rapport aux pairs ruidosos conhecidos, histogramme de latence coïncident avec une ligne de base de mise en scène ± 20 %. |
| **Disponibilité générale** | 100 % effectuent le déploiement | Regras do pager ativas | Zéro erreur `NoHealthyProviders` pendant 24 h, la durée des tentatives est établie, le SLA du tableau de bord est en vert. |

Pour chaque phase :1. Actualisez le JSON de l'explorateur avec `max_providers` et les budgets de nouvelle tentative anticipée.
2. Exécutez `sorafs_cli fetch` ou une suite de tests d'intégration du SDK contre le luminaire canonique et un manifeste représentatif de l'environnement.
3. Capturez les œuvres du tableau de bord + le résumé et les annexes à l'enregistrement de la version.
4. Réviser les tableaux de bord de télémétrie ainsi que l'ingénierie de l'installation avant de promouvoir la phase suivante.

## 4. Observabilité et ganchos d'incident

- **Métriques :** Garantit que le moniteur Alertmanager `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` et `sorafs_orchestrator_retries_total`. Un petit repentir signifie en général qu'un fournisseur est dégradé sur sa charge.
- **Journaux :** La rotation des cibles `telemetry::sorafs.fetch.*` pour l'agrégateur de journaux partagés. Crie buscas salvas para `event=complete status=failed` pour accélérer le triage.
- **Tableaux de bord :** Persista cada artefato de scoreboard em armazenamento de longo prazo. Le JSON sert également de preuve pour les révisions de conformité et les restaurations par phase.
- **Tableaux de bord :** Clonez le tableau de bord Grafana canonique (`docs/examples/sorafs_fetch_dashboard.json`) en pâte de production avec comme source d'alerte de `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Communication et documentation- Enregistrez chaque modification de refus/boost dans le journal des modifications des opérations avec l'horodatage, l'opérateur, le motif et l'incident associé.
- Notifier les équipes du SDK lorsque les pesos des fournisseurs ou les budgets doivent réessayer pour fonctionner selon les attentes du client.
- Après avoir terminé GA, actualisez `status.md` avec le résumé du déploiement et archivez cette référence du runbook dans les notes de version.