---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : "Plan de despliegue de adverts de provenedores SoraFS"
---

> Adapté de [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# Plan d'affichage des annonces des fournisseurs SoraFS

Ce plan coordonne le basculement à partir des publicités autorisées par les fournisseurs vers la
Surface Gobernada `ProviderAdvertV1` requise pour la récupération multi-origine
des morceaux. Se centre en trois livrables :

- **Guide des opérateurs.** Acciones paso a paso que les fournisseurs de stockage doivent
  compléter la porte antes de cada.
- **Cobertura de telemetria.** Tableaux de bord et alertes que l'observabilité et les opérations utilisent
  pour confirmer que la red solo accepte les annonces conformes.
  pour que les équipes de SDK et d'outillage planifient leurs versions.

Le déploiement est aligné avec les hits SF-2b/2c du
[feuille de route de migration de SoraFS](./migration-roadmap) et suppose que la politique de
admission del [provider admission Policy](./provider-admission-policy) esta fr
vigueur.

## Chronogramme des phases

| Phase | Ventana (objetivo) | Comportement | Actions de l'opérateur | Enfoque d'observabilité |
|-------|-----------------|-----------|------------------|-------------------|

## Checklist des opérateurs

1. **Inventaire des annonces.** Liste de chaque annonce publiée et enregistrée :
   - Route de l'enveloppe gouvernementale (`defaults/nexus/sorafs_admission/...` ou équivalent en production).
   - `profile_id` et `profile_aliases` de l'annonce.
   - Liste des capacités (voir Espera al Menos `torii_gateway` et `chunk_range_fetch`).
   - Indicateur `allow_unknown_capabilities` (requerido cuando hay TLVs reservados por supplier).
2. **Régénérer les outils des fournisseurs.**
   - Reconstruire la charge utile avec l'annonce de l'éditeur du fournisseur, en garantissant :
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` avec un `max_span` défini
     - `allow_unknown_capabilities=<true|false>` quando haya TLV GRAISSE
   - Validé via `/v1/sorafs/providers` et `sorafs_fetch` ; les publicités sur
     les capacités desconocidas doivent être triées.
3. **Préparation valide multi-origine.**
   - Exécution `sorafs_fetch` avec `--provider-advert=<path>` ; la CLI aujourd'hui est tombée
     lorsque vous tombez sur `chunk_range_fetch` et que vous avez des publicités pour les capacités
     desconocidas ignoradas. Capturer le rapport JSON et l'archiver avec les journaux
     des opérations.
4. **Préparer les rénovations.**
   - Envia enveloppes `ProviderAdmissionRenewalV1` au moins 30 jours avant
     application et passerelle (R2). Les rénovations doivent conserver la poignée
     canonique et l'ensemble des capacités ; enjeu solo, points finaux ou métadonnées deben
     changer.
5. **Comunicar a los equipos dependientes.**
   - Les propriétaires du SDK doivent libérer des versions qui exposent des publicités à eux
     opérateurs lorsque les publicités sean rechazados.
   - DevRel annonce chaque transition de phase ; incluir enlace un tableaux de bord et la
     logique de l'ombre de bas.
6. **Installer des tableaux de bord et des alertes.**
   - Importation de l'exportation de Grafana et colocalo bajo **SoraFS / Provider
     Déploiement** avec l'UID `sorafs-provider-admission`.
   - Assurez-vous que les règles d'alerte correspondent au canal partagé
     `sorafs-advert-rollout` en mise en scène et production.

## Télémétrie et tableaux de bordLes mesures suivantes sont affichées via `iroha_telemetry` :

- `torii_sorafs_admission_total{result,reason}` — comptes acceptés, rechazados
  et résultats avec publicités. Les raisons incluent `missing_envelope`, `unknown_capability`,
  `stale` et `policy_violation`.

Exporter de Grafana : [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importer les archives dans le référentiel compartimenté de tableaux de bord (`observability/dashboards`)
et actualisez uniquement l'UID de la source de données avant de la publier.

Le tableau est publié sous le tapis de Grafana **SoraFS / Provider Rollout** avec
l'UID estable `sorafs-provider-admission`. Les règles d'alerte
`sorafs-admission-warn` (avertissement) et `sorafs-admission-reject` (critique) sont
préconfigurés pour utiliser la politique de notification `sorafs-advert-rollout` ;
Ajustez ce point de contact si la liste de destination change au lieu de modifier le
JSON du tableau de bord.

Panneaux Grafana recommandés :

| Panneau | Requête | Remarques |
|-------|-------|-------|
| **Taux de résultats d'admission** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Graphique de pile pour visualiser accepter, avertir ou rejeter. Alerte quand avertissement > 0,05 * total (avertissement) ou rejet > 0 (critique). |
| **Taux d'avertissement** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Série temporelle d'une seule ligne qui alimente l'ombre du téléavertisseur (taux d'avertissement de 5 % pendant 15 minutes). |
| **Motifs de refus** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Guider le triage du runbook ; l'adjunta enlace des pasos de mitigacion. |
| **Rafraîchir la dette** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Les fournisseurs Indica ne respectent pas la date limite de rafraîchissement ; cruza avec les journaux de cache de découverte. |

Artefacts de CLI pour les manuels des tableaux de bord :

- `sorafs_fetch --provider-metrics-out` écrivent les contadores `failures`, `successes` et
  `disabled` par le fournisseur. Importation de tableaux de bord ad hoc pour moniteur
  essais à sec de l'orchestrateur avant de changer de fournisseur en production.
- Les champs `chunk_retry_rate` et `provider_failure_rate` du rapport JSON
  resaltan throttling o sintomas de payloads stale que suelen précèdent les rechazos
  d'admission.

### Disposition du tableau de bord de Grafana

Observability publica un board dédié — **SoraFS Admission du fournisseur
Déploiement** (`sorafs-provider-admission`) — bas **SoraFS / Déploiement du fournisseur**
avec les identifiants canoniques suivants du panneau :

- Panel 1 — *Taux de résultat d'admission* (zone empilée, unidad "ops/min").
- Panneau 2 — *Taux d'avertissement* (série unique), émettant la expression
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   somme(taux(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Motifs de rejet* (série de tiempo agrupada por `reason`), ordonné par
  `rate(...[5m])`.
- Panel 4 — *Refresh dette* (stat), réfléchissant la requête de la table antérieure y
  annoté avec les délais d'actualisation des annonces extraites du grand livre de migration.

Copier (ou créer) le squelette JSON dans le dépôt de tableaux de bord d'infrastructure
`observability/dashboards/sorafs_provider_admission.json`, puis actualisez-le seul
UID de la source de données ; les identifiants du panneau et les règles d'alerte sont référencées en
les runbooks de bas, donc vous éviterez de les numériser sans réviser cette documentation.Pour plus de commodité, le référentiel comprend une définition du tableau de bord de
référence en `docs/source/grafana_sorafs_admission.json` ; copier sur ton tapis
Grafana si vous avez besoin d'un point de départ pour les essais locaux.

### Verres d'alerte de Prometheus

Agrega el suivant grupo de reglas a
`observability/prometheus/sorafs_admission.rules.yml` (créer l'archive si ceci est
el primer grupo de reglas SoraFS) et inclut votre configuration de
Prometheus. Remplacez `<pagerduty>` avec l'étiquette d'engagement réel pour vous
rotation sur appel.

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

Éjecté `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
avant de subir des changements pour garantir que la syntaxe passe `promtool check rules`.

## Matrice de déploiement

| Caractéristiques de l'annonce | R0 | R1 | R2 | R3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` présente, alias canonicos, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Carece de la capacité `chunk_range_fetch` | ⚠️ Avertir (ingérer + télémétrie) | ⚠️ Avertir | ❌ Rejeter (`reason="missing_capability"`) | ❌ Rejeter |
| TLV de capacité découverte sans `allow_unknown_capabilities=true` | ✅ | ⚠️ Avertir (`reason="unknown_capability"`) | ❌ Rejeter | ❌ Rejeter |
| `refresh_deadline` expiré | ❌ Rejeter | ❌ Rejeter | ❌ Rejeter | ❌ Rejeter |
| `signature_strict=false` (appareils de diagnostic) | ✅ (solo desarrollo) | ⚠️ Avertir | ⚠️ Avertir | ❌ Rejeter |

Tous les horaires utilisent UTC. Les tâches d'application sont réfléchies lors de la migration
le grand livre ne se déplace pas sans un vote du conseil ; n'importe quel changement nécessite une mise à jour
este archivo y el ledger dans el mismo PR.

> **Note de mise en œuvre :** R1 introduit la série `result="warn"` fr
> `torii_sorafs_admission_total`. Le patch d'ingesta Torii qui ajoute la nouvelle
> l'étiquette est associée aux paramètres de télémétrie SF-2 ; hasta que llegue,

## Communication et gestion des incidents

- **Mailer semanal de estado.** DevRel fait circuler un résumé bref des mesures de
  admission, advertencias pendientes et délais proches.
- **Réponse à un incident.** Si les alertes `reject` sont activées, de garde :
  1. Récupérez l'annonce officielle via la découverte de Torii (`/v1/sorafs/providers`).
  2. Relancez la validation de l'annonce sur le pipeline du fournisseur et comparez-la avec
     `/v1/sorafs/providers` pour reproduire l'erreur.
  3. Coordonner avec le fournisseur la rotation de l'annonce avant l'actualisation suivante
     date limite.
- **Congelamientos de change.** Aucun changement de schéma de capacités n'est appliqué
  pendant R1/R2 au moins que le comité de déploiement soit en avril ; les essais GRAISSE deben
  programmer pendant la ventana semanal de mantenimiento et s'inscrire en el
  registre des migrations.

## Références

- [Protocole nœud/client SoraFS](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Politique d'admission des fournisseurs](./provider-admission-policy)
- [Feuille de route de migration](./migration-roadmap)
- [Extensions multi-sources d'annonce de fournisseur] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)