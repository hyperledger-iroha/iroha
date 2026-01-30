---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: "Plan de déploiement des adverts de providers SoraFS"
---

> Adapté de [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# Plan de déploiement des adverts de providers SoraFS

Ce plan coordonne la bascule des adverts permissifs des providers vers la surface
entièrement gouvernée `ProviderAdvertV1` requise pour la récupération de chunks
multi-source. Il se concentre sur trois livrables :

- **Guide opérateur.** Actions pas à pas que les providers de stockage doivent
  terminer avant chaque gate.
- **Couverture de télémétrie.** Dashboards et alertes qu'Observabilité et Ops
  utilisent pour confirmer que le réseau n'accepte que des adverts conformes.
- **Calendrier de déploiement.** Dates explicites pour rejeter les envelopes

Le déploiement s'aligne sur les jalons SF-2b/2c de la
[roadmap de migration SoraFS](./migration-roadmap) et suppose que la politique
admission du [provider admission policy](./provider-admission-policy) est déjà en
vigueur.

## Chronologie des phases

| Phase | Fenêtre (cible) | Comportement | Actions opérateur | Focus observabilité |
|-------|-----------------|-----------|------------------|-------------------|

## Checklist opérateur

1. **Inventorier les adverts.** Lister chaque advert publié et enregistrer :
   - Chemin de l'envelope gouvernant (`defaults/nexus/sorafs_admission/...` ou équivalent en production).
   - `profile_id` et `profile_aliases` de l'advert.
   - Liste de capabilities (au moins `torii_gateway` et `chunk_range_fetch`).
   - Flag `allow_unknown_capabilities` (requis quand des TLVs réservés vendor sont présents).
2. **Régénérer avec le tooling provider.**
   - Reconstruire le payload avec votre publisher de provider advert, en garantissant :
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` avec un `max_span` défini
     - `allow_unknown_capabilities=<true|false>` quand des TLVs GREASE sont présents
   - Valider via `/v1/sorafs/providers` et `sorafs_fetch` ; les warnings sur des
     capabilities inconnues doivent être triés.
3. **Valider la readiness multi-source.**
   - Exécuter `sorafs_fetch` avec `--provider-advert=<path>` ; le CLI échoue
     désormais quand `chunk_range_fetch` manque et affiche des warnings pour des
     capabilities inconnues ignorées. Capturer le rapport JSON et l'archiver avec
     les logs d'opérations.
4. **Préparer les renouvellements.**
   - Soumettre des envelopes `ProviderAdmissionRenewalV1` au moins 30 jours avant
     l'enforcement gateway (R2). Les renouvellements doivent conserver le handle
     canonique et l'ensemble des capabilities ; seuls le stake, les endpoints ou
     la metadata doivent changer.
5. **Communiquer avec les équipes dépendantes.**
   - Les owners SDK doivent publier des versions qui exposent des warnings aux
     opérateurs lorsque les adverts sont rejetés.
   - DevRel annonce chaque transition de phase ; inclure les liens de dashboards
     et la logique de seuil ci-dessous.
6. **Installer dashboards et alertes.**
   - Importer l'export Grafana et le placer sous **SoraFS / Provider
     Rollout** avec l'UID `sorafs-provider-admission`.
   - S'assurer que les règles d'alertes pointent vers le canal partagé
     `sorafs-advert-rollout` en staging et production.

## Télémétrie & dashboards

Les métriques suivantes sont déjà exposées via `iroha_telemetry` :

- `torii_sorafs_admission_total{result,reason}` — compte les acceptés, rejetés
  et avertissements. Les raisons incluent `missing_envelope`, `unknown_capability`,
  `stale`, et `policy_violation`.

Export Grafana : [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importez le fichier dans le repo partagé de dashboards (`observability/dashboards`)
et mettez à jour uniquement l'UID de datasource avant publication.

Le board est publié sous le dossier Grafana **SoraFS / Provider Rollout** avec
l'UID stable `sorafs-provider-admission`. Les règles d'alerte
`sorafs-admission-warn` (warning) et `sorafs-admission-reject` (critical) sont
pré-configurées pour utiliser la policy de notification `sorafs-advert-rollout` ;
ajustez ce contact point si la liste de destinations change plutôt que d'éditer
le JSON du dashboard.

Panneaux Grafana recommandés :

| Panel | Query | Notes |
|-------|-------|-------|
| **Admission outcome rate** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Stack chart pour visualiser accept vs warn vs reject. Alerte quand warn > 0.05 * total (warning) ou reject > 0 (critical). |
| **Warning ratio** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Timeseries à ligne unique qui alimente le seuil pager (taux warning 5 % roulant sur 15 minutes). |
| **Rejection reasons** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Guide le triage du runbook ; attacher des liens vers les étapes de mitigation. |
| **Refresh debt** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Indique des providers qui ratent la deadline de refresh ; croiser avec les logs du discovery cache. |

Artefacts CLI pour dashboards manuels :

- `sorafs_fetch --provider-metrics-out` écrit des compteurs `failures`, `successes` et
  `disabled` par provider. Importez dans des dashboards ad-hoc pour surveiller
  les dry-runs d'orchestrator avant de basculer les providers en production.
- Les champs `chunk_retry_rate` et `provider_failure_rate` du rapport JSON
  mettent en avant les throttlings ou symptômes de payloads stale qui précèdent
  souvent les rejets d'admission.

### Mise en page du dashboard Grafana

Observabilité publie un board dédié — **SoraFS Provider Admission
Rollout** (`sorafs-provider-admission`) — sous **SoraFS / Provider Rollout**
avec les IDs de panel canoniques suivants :

- Panel 1 — *Admission outcome rate* (stacked area, unité "ops/min").
- Panel 2 — *Warning ratio* (single series), émettant l'expression
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Rejection reasons* (time series regroupées par `reason`), triées par
  `rate(...[5m])`.
- Panel 4 — *Refresh debt* (stat), reprenant la query du tableau ci-dessus et
  annoté avec les deadlines de refresh des adverts extraites du migration ledger.

Copiez (ou créez) le squelette JSON dans le repo de dashboards d'infra à
`observability/dashboards/sorafs_provider_admission.json`, puis mettez à jour
uniquement l'UID de datasource ; les IDs de panel et les règles d'alertes sont
référencés par les runbooks ci-dessous, évitez donc de les renuméroter sans
mettre à jour cette documentation.

Pour convenance, le repo fournit une définition de dashboard de référence à
`docs/source/grafana_sorafs_admission.json` ; copiez-la dans votre dossier Grafana
si vous avez besoin d'un point de départ pour des tests locaux.

### Règles d'alerte Prometheus

Ajoutez le groupe de règles suivant à
`observability/prometheus/sorafs_admission.rules.yml` (créez le fichier si c'est
le premier groupe de règles SoraFS) et incluez-le dans votre configuration
Prometheus. Remplacez `<pagerduty>` par le label de routage réel pour votre
rotation on-call.

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

Exécutez `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
avant de pousser les changements pour vérifier que la syntaxe passe
`promtool check rules`.

## Matrice de déploiement

| Caractéristiques de l'advert | R0 | R1 | R2 | R3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` présent, aliases canoniques, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Absence de capability `chunk_range_fetch` | ⚠️ Warn (ingest + telemetry) | ⚠️ Warn | ❌ Reject (`reason="missing_capability"`) | ❌ Reject |
| TLVs de capability inconnue sans `allow_unknown_capabilities=true` | ✅ | ⚠️ Warn (`reason="unknown_capability"`) | ❌ Reject | ❌ Reject |
| `refresh_deadline` expiré | ❌ Reject | ❌ Reject | ❌ Reject | ❌ Reject |
| `signature_strict=false` (fixtures diagnostic) | ✅ (développement uniquement) | ⚠️ Warn | ⚠️ Warn | ❌ Reject |

Toutes les heures utilisent UTC. Les dates d'enforcement sont reflétées dans le
migration ledger et ne bougeront pas sans vote du council ; tout changement
requiert la mise à jour de ce fichier et du ledger dans la même PR.

> **Note d'implémentation :** R1 introduit la série `result="warn"` dans
> `torii_sorafs_admission_total`. Le patch d'ingestion Torii qui ajoute le nouveau
> label est suivi avec les tâches de télémétrie SF-2 ; jusque-là, utilisez le

## Communication & gestion d'incident

- **Mailer hebdomadaire de statut.** DevRel diffuse un bref résumé des métriques
  d'admission, des warnings en cours et des deadlines à venir.
- **Réponse incident.** Si les alertes `reject` se déclenchent, l'on-call :
  1. Récupère l'advert fautif via discovery Torii (`/v1/sorafs/providers`).
  2. Relance la validation de l'advert dans le pipeline provider et compare avec
     `/v1/sorafs/providers` pour reproduire l'erreur.
  3. Coordonne avec le provider pour faire tourner l'advert avant la prochaine
     deadline de refresh.
- **Gel des changements.** Aucune modification du schéma de capabilities pendant
  R1/R2 à moins que le comité de rollout ne valide ; les essais GREASE doivent
  être planifiés durant la fenêtre de maintenance hebdomadaire et journalisés
  dans le migration ledger.

## Références

- [SoraFS Node/Client Protocol](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Provider Admission Policy](./provider-admission-policy)
- [Migration Roadmap](./migration-roadmap)
- [Provider Advert Multi-Source Extensions](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)
