---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: "Déploiement du plan et fournisseurs d'annonces pertinents SoraFS"
---

> Adapté à [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# Plan de déploiement et d'annonces publicitaires SoraFS

Ce plan prévoit la mise en place de publicités permissives en vue de la réussite
Prise en charge `ProviderAdvertV1`, idéale pour les appareils multi-sources
morceaux. Nous nous concentrons sur les trois livrables :

- **Руководство оператора.** Пошаговые действия, которые провайдеры хранения должны
  выполнить до включения каждого portail.
- **Покрытие телеметрией.** Дашbordы и alerts, которые Observability и Ops используют,
  que vous puissiez publier, que vous définissez uniquement les publicités les plus pertinentes.
  Le SDK et les outils peuvent planifier les versions.

Déploiement du système avec les véhicules SF-2b/2c
[Feuille de route pour les migrations SoraFS](./migration-roadmap) et avant l'arrivée de la stratégie
[Politique d'admission du fournisseur](./provider-admission-policy) уже действует.

## Таймлайн фаз

| Faza | Окно (цель) | Поведение | Opérateurs de réception | Focus sur les démos |
|-------|-----------------|-----------|------------------|-------------------|

## Opérateur de contrôle

1. **Inventaire des annonces.** Ouvrir l'annonce publique et insérer :
   - Путь к enveloppe de gouvernance (`defaults/nexus/sorafs_admission/...` ou production-эквивалент).
   - `profile_id` et `profile_aliases` publicité.
   - Capacités de pointe (elles correspondent au minimum `torii_gateway` et `chunk_range_fetch`).
   - Indicateur `allow_unknown_capabilities` (identifié par le TLV réservé par le fournisseur).
2. **Gestion des outils du fournisseur.**
   - Пересоберите payload через éditeur annonce, убедившись в:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` avec `max_span`
     - `allow_unknown_capabilities=<true|false>` pour la graisse TLV
   - Vérifiez que `/v2/sorafs/providers` et `sorafs_fetch` ; avant-première
     Les capacités les plus récentes doivent être triées.
3. **Préparation multi-sources.**
   - Выполните `sorafs_fetch` avec `--provider-advert=<path>` ; CLI теперь падает,
     Je vais résoudre le problème `chunk_range_fetch` et je préviens
     проигнорированных неизвестных capacités. Supprimer le fichier JSON et
     ARCHивируйте его с opérations journaux.
4. **Projet de commande.**
   - Enveloppes `ProviderAdmissionRenewalV1` minimum de 30 jours
     application de la passerelle (R2). Produire une poignée canonique et
     capacités набор; Il s'agit uniquement de enjeux, de points de terminaison ou de métadonnées.
5. **Коммуникация с зависимыми командами.**
   - Les versions SDK doivent être téléchargées pour afficher les avertissements de l'opérateur.
     при отклонении annonces.
   - DevRel annonce chaque épisode ; включайте ссылки на tableaux de bord et logique
     порогов ниже.
6. **Tableaux de bord et alertes supplémentaires.**
   - Importez Grafana et exportez-le vers **SoraFS / Provider
     Déploiement** avec UID `sorafs-provider-admission`.
   - Убедитесь, что règles d'alerte направлены в общий canal
     `sorafs-advert-rollout` dans la mise en scène et la production.

## Télémétrie et frontières

Les mesures suivantes sont disponibles à partir de `iroha_telemetry` :- `torii_sorafs_admission_total{result,reason}` — Prises de courant, ouverture
  и avertissements. Les prix incluent `missing_envelope`, `unknown_capability`, `stale` et
  `policy_violation`.

Exportation Grafana : [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importez le fichier dans votre dépôt d'entreprise (`observability/dashboards`) et
Vérifiez uniquement la source de données UID avant la publication.

Le tableau de bord est publié dans le paquet Grafana **SoraFS / Déploiement du fournisseur** avec
UID stable `sorafs-provider-admission`. Règles d'alerte
`sorafs-admission-warn` (avertissement) et `sorafs-admission-reject` (critique)
преднастроены на politique уведомлений `sorafs-advert-rollout` ; меняйте контактный
Il s'agit d'une étape importante pour les utilisateurs utilisant les paramètres JSON du bord.

Panneaux recommandés Grafana :

| Panneau | Requête | Remarques |
|-------|-------|-------|
| **Taux de résultats d'admission** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Graphique de pile pour la visualisation accepter vs avertir vs rejeter. Alerte par avertissement > 0,05 * total (avertissement) ou rejet > 0 (critique). |
| **Taux d'avertissement** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Série temporelle complète, un téléavertisseur (taux d'avertissement de 5 % par semaine de 15 minutes). |
| **Motifs de refus** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Pour le tri dans le runbook ; прикрепляйте ссылки на шаги atténuation. |
| **Rafraîchir la dette** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Указывает на fournisseurs, пропустивших date limite d'actualisation ; сверяйте с логами cache de découverte. |

Artefacts CLI pour les vols à bord :

- `sorafs_fetch --provider-metrics-out` pour les fiches `failures`, `successes` et
  `disabled` par le fournisseur de téléphone. Importer des tableaux de bord ad hoc, par exemple
  surveiller l'orchestrateur à sec avant de sélectionner les fournisseurs de production.
- Pour `chunk_retry_rate` et `provider_failure_rate` dans l'option JSON
  Il est possible de limiter la limitation ou de signaler des charges utiles obsolètes, ce qui signifie que
  отклонениям admission.

### Porte-clés Grafana

Tableau d'observabilité publié — ** Admission du fournisseur SoraFS
Déploiement** (`sorafs-provider-admission`) — **SoraFS / Déploiement du fournisseur**
avec les identifiants de panneau canonique :

- Panel 1 — *Taux de résultat d'admission* (zone empilée, единица "ops/min").
- Panneau 2 — *Taux d'avertissement* (série unique), выражение
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   somme(taux(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Motifs de rejet* (séries chronologiques, сгруппированные по `reason`), сортировка по
  `rate(...[5m])`.
- Panel 4 — *Actualiser la dette* (stat), отражает запрос из таблицы выше и
  Annulation du délai d'actualisation du grand livre de migration.

Copier (ou télécharger) le squelette JSON dans les référentiels d'infrastructures des pays-bords
`observability/dashboards/sorafs_provider_admission.json`, veuillez consulter ce dernier
Source de données UID ; ID de panneau et règles d'alerte utilisés dans les runbooks ici, mais non
Перенумеровывайте их без обновления этой документации.

Pour créer un référentiel, vous pouvez définir la définition du tableau de bord de référence dans
`docs/source/grafana_sorafs_admission.json` ; скопируйте его в вашу Grafana папку,
s'il n'y a pas de variante de démarrage pour le test local.

### Alertes envoyées PrometheusAjoutez le groupe en question à
`observability/prometheus/sorafs_admission.rules.yml` (vous devez le faire, si c'est le cas)
(Veuillez consulter le groupe SoraFS) et ajoutez-le à la configuration Prometheus.
Indiquez `<pagerduty>` sur l'étiquette de routage réelle pour votre rotation d'astreinte.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

Postez `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
Avant l'ouverture, ce qui se passe, quelle syntaxe se produit
`promtool check rules`.

## La matrice contemporaine

| Caractéristiques annonce | R0 | R1 | R2 | R3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` присутствует, alias canoniques, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Capacité nette `chunk_range_fetch` | ⚠️ Avertir (ingérer + télémétrie) | ⚠️ Avertir | ❌ Rejeter (`reason="missing_capability"`) | ❌ Rejeter |
| Capacité TLV inégalée sans `allow_unknown_capabilities=true` | ✅ | ⚠️ Avertir (`reason="unknown_capability"`) | ❌ Rejeter | ❌ Rejeter |
| Produit `refresh_deadline` | ❌ Rejeter | ❌ Rejeter | ❌ Rejeter | ❌ Rejeter |
| `signature_strict=false` (appareils de diagnostic) | ✅ (développement только) | ⚠️ Avertir | ⚠️ Avertir | ❌ Rejeter |

Tous les temps sont disponibles en UTC. Les données d'application figurent dans le registre des migrations et non
будут изменены без голосования conseil; les activités de loisirs et de loisirs
файла и ledger в одном PR.

> **Modification de la réalisation :** R1 correspond à la série `result="warn"` dans
> `torii_sorafs_admission_total`. Ingestion du patch Torii, nouvelle étiquette,
> отслеживается вместе с задачами телеметрии SF-2; до его попадания используйте

## Les incidents commerciaux et de surveillance

- **Statut de statut actuel.** DevRel a défini la mesure du résumé
  admission, текущих avertissements и предстоящих délais.
- **Réponse aux incidents.** Si vous signalez les alertes `reject`, les agents de garde :
  1. Téléchargez l'annonce problématique pour Discovery Torii (`/v2/sorafs/providers`).
  2. Publier une annonce de validation dans le pipeline du fournisseur et la publier
     `/v2/sorafs/providers`, que vous avez choisi.
  3. Coordonner la rotation de l'annonce en fonction de la date limite d'actualisation.
- **Amélioration des capacités de schéma.** Capacités de schéma améliorées dans R1/R2, etc.
  Le déploiement du comité n'est pas terminé ; L'épandage de GRAISSE doit être effectué à un moment donné
  Vous devez également surveiller et fixer le registre des migrations.

## Ссылки

- [Protocole nœud/client SoraFS](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Politique d'admission des fournisseurs](./provider-admission-policy)
- [Feuille de route de migration](./migration-roadmap)
- [Extensions multi-sources d'annonce de fournisseur] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)