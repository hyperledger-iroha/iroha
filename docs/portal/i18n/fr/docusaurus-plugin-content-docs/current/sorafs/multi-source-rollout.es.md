---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : déploiement multi-source
titre : Runbook de despliegue multi-origen et denegación de provenedores
sidebar_label : Runbook de despliegue multi-origine
description : Liste de vérification opérationnelle pour les opérations multi-origines par étapes et dénonciation des fournisseurs en cas d'urgence.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/runbooks/multi_source_rollout.md`. Assurez-vous d'avoir des copies alignées jusqu'à ce que le ensemble de documents héréditaires soit retiré.
:::

## Proposition

Ce runbook guide le SRE et les ingénieurs de garde pour le suivi des flux critiques :

1. Desplegar el orquestador multi-origen en oleadas controladas.
2. Refuser ou déprioriser les fournisseurs qui se comportent mal sans déstabiliser les sessions existantes.

Supposons que la pile d'orchestration entre sous SF-6 est déchargée (`sorafs_orchestrator`, API de gamme de morceaux de la passerelle, exportateurs de télémétrie).

> **Voir aussi :** Le [Runbook des opérations de l'orchestre](./orchestrator-ops.md) approfondit les procédures d'exécution (capture du tableau de bord, bascules de déclenchement par étapes, restauration). Utilisez des références en conjonction lors de changements en vivo.

## 1. Validation préalable1. **Confirmer les entrées de gouvernement.**
   - Tous les fournisseurs candidats doivent être publiés sur `ProviderAdvertV1` avec des charges utiles de capacité de portée et de présupposés de flux. Valable au milieu du `/v1/sorafs/providers` et comparé aux champs de capacité des espérés.
   - Les instants de télémétrie qui apportent des tâches de latence/chute doivent avoir moins de 15 minutes avant chaque exécution canarie.
2. **Préparer la configuration.**
   - Conserver la configuration JSON de l'explorateur dans l'arbre `iroha_config` par exemple :

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Actualisez le JSON avec les limites spécifiques au déploiement (`max_providers`, présupposés de réintention). Utilisez le même fichier de mise en scène/production pour que les différences soient minimes.
3. **Éjercitar los canónicos.**
   - Indiquez les variables d'entrée du manifeste/jeton et exécutez la récupération déterministe :

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

     Les variables d'entrée doivent contenir le résumé de la charge utile du manifeste (hex) et les jetons de flux codifiés en base64 pour chaque fournisseur participant au Canary.
   - Compara `artifacts/canary.scoreboard.json` avec le déclencheur antérieur. Tout fournisseur nouveau non éligible ou un changement de peso > 10 % nécessite une révision.
4. **Vérifiez que la télémétrie est connectée.**
   - Ouvrir l'exportation de Grafana et `docs/examples/sorafs_fetch_dashboard.json`. Assurez-vous que les mesures `sorafs_orchestrator_*` soient mises en scène avant de continuer.## 2. Délégation des fournisseurs en urgence

Si cette procédure est effectuée lorsqu'un fournisseur entreprend des morceaux corrompus, il y a des délais d'attente de forme persistante ou des erreurs d'achat.1. **Capturer les preuves.**
   - Exporter le résumé de récupération le plus récent (salida de `--json-out`). Registre des indices des morceaux tombés, alias des fournisseurs et désajustés du digest.
   - Garder des extraits de journaux pertinents pour les cibles `telemetry::sorafs.fetch.*`.
2. **Appliquer un remplacement immédiat.**
   - Marquez le fournisseur comme pénalisé dans l'instantané de télémétrie distribuée à l'orquestador (établissement `penalty=true` ou limite `token_health` à `0`). La construction suivante du tableau de bord sera automatiquement exclue du fournisseur.
   - Pour des essais d'humeur ad hoc, passez les étapes `--deny-provider gw-alpha` et `sorafs_cli fetch` pour exécuter l'itinéraire de chute sans espérer la propagation de la télémétrie.
   - Regardez le paquet mis à jour de télémétrie/configuration dans l'environnement affecté (staging → canary → production). Documentez le changement dans le journal de l'incident.
3. **Valider le remplacement.**
   - Répétez la récupération du luminaire canonique. Confirmez que le tableau de bord marque le fournisseur comme non éligible avec le motif `policy_denied`.
   - Inspecciona `sorafs_orchestrator_provider_failures_total` pour s'assurer que le contacteur est déjà en train d'augmenter pour le fournisseur défectueux.
4. **Escalar bloqueos prolongados.**- Si le fournisseur est bloqué pendant >24 h, ouvrez un ticket de commande pour faire tourner ou suspendre votre annonce. Assurez-vous de passer le vote, de maintenir la liste de refus et de rafraîchir les instants de télémétrie pour que le fournisseur ne réintègre pas le tableau de bord.
5. **Protocole de restauration.**
   - Pour rétablir le fournisseur, éliminer la liste de dénonciation, voir le désabonnement et capturer une fresque instantanée du tableau de bord. Ajouter le changement à l'après-mort de l'incident.

## 3. Plan de déploiement par étapes| Phase | Alcance | Senales requises | Critères de Go/No-Go |
|------|---------|----------|----------------------|
| **Labo** | Groupe d'intégration dédié | Récupérer le manuel de la CLI pour les charges utiles des appareils | Tous les morceaux sont complets, les compteurs de défauts du fournisseur sont permanents à 0, taille de réintention < 5 %. |
| **Mise en scène** | Mise en scène du plan de contrôle complet | Tableau de bord de Grafana connecté ; règles d'alerte en mode solo avertissement | `sorafs_orchestrator_active_fetches` vuelve a zéro après chaque exécution de test ; il n’y a pas d’alertes disparates `warn/critical`. |
| **Canari** | ≤10% du trafic de production | Pager silencieux mais télémétrique surveillé en temps réel | Rapport de réintention < 10 %, chutes des fournisseurs aislados aux pairs ruidosos connus, histogramme de latence coïncide avec la ligne de base de mise en scène ±20 %. |
| **Disponibilité générale** | Déploiement à 100 % | Verres du téléavertisseur activé | Aucune erreur `NoHealthyProviders` pendant 24 h, rapport de réintention établi, panneaux SLA du tableau de bord en vert. |

Pour chaque phase :1. Actualisez le JSON de l'explorateur avec le `max_providers` et les présupposés de réintentions prévus.
2. Exécutez `sorafs_cli fetch` ou la suite d'essais d'intégration du SDK contre l'appareil canonique et un manifeste représentant l'entreprise.
3. Capturez les artefacts du tableau de bord et le résumé, ainsi que les ajouts au registre de sortie.
4. Révisez les tableaux de bord de télémétrie avec l'ingénieur de garde avant de promouvoir la phase suivante.

## 4. Observabilité et contrôle des incidents

- **Métriques :** Assurez-vous que Alertmanager est surveillé `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` et `sorafs_orchestrator_retries_total`. Un pico repentino suele significar que un fournisseur se dégrada bajo carga.
- **Journaux :** Enregistrez les cibles `telemetry::sorafs.fetch.*` dans l'agrégateur de journaux partagé. Créez des gardes-fous pour `event=complete status=failed` pour accélérer le triage.
- **Tableaux de bord :** Conservez chaque artefact de tableau de bord sur une grande place. Le JSON sert également de base de preuves pour les révisions de compilation et les restaurations par étapes.
- **Tableaux de bord :** Clona el tablero Grafana canónico (`docs/examples/sorafs_fetch_dashboard.json`) sur le tapis de production avec le règlement d'alerte de `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Communication et documentation- Enregistrez chaque changement de désactivation/boost dans le journal des modifications des opérations avec la marque du temps, de l'opérateur, du motif et de l'incident associé.
- Notifier les équipes du SDK lorsque les pesos des fournisseurs ou les présupposés de réintention changent pour aligner les attentes du travail du client.
- Une fois GA terminé, actualisez `status.md` avec le résumé du déploiement et archivez cette référence du runbook dans les notes de la version.