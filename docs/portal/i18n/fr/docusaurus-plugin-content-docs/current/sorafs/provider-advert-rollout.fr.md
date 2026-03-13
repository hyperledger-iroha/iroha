---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : "Plan de déploiement des annonces de fournisseurs SoraFS"
---

> Adapté de [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# Plan de déploiement des annonces de fournisseurs SoraFS

Ce plan coordonne la bascule des annonces permissifs des fournisseurs vers la surface
entièrement gouvernée `ProviderAdvertV1` requise pour la récupération de chunks
multi-sources. Il se concentre sur trois livrables :

- **Guide opérateur.** Actions pas à pas que les fournisseurs de stockage doivent
  terminer avant chaque porte.
- **Couverture de télémétrie.** Dashboards et alertes qu'Observabilité et Ops
  Utiliser pour confirmer que le réseau n'accepte que des annonces conformes.
- **Calendrier de déploiement.** Dates explicites pour rejeter les enveloppes

Le déploiement s'aligne sur les jalons SF-2b/2c de la
[roadmap de migration SoraFS](./migration-roadmap) et suppose que la politique
admission du [provider admission Policy](./provider-admission-policy) est déjà en
vivaces.

## Chronologie des phases

| Phases | Fenêtre (câble) | Comportement | Actions opérateur | Focus observabilité |
|-------|-----------------|-----------|------------------|-------------------|

## Checklist opérateur1. **Inventaire des annonces.** Lister chaque annonce publiée et enregistrer :
   - Chemin de l'enveloppe gouvernante (`defaults/nexus/sorafs_admission/...` ou équivalent en production).
   - `profile_id` et `profile_aliases` de l'annonce.
   - Liste de capacités (au moins `torii_gateway` et `chunk_range_fetch`).
   - Flag `allow_unknown_capabilities` (requis quand des TLVs réservés seller sont présents).
2. **Régénérer avec le fournisseur d'outillage.**
   - Reconstruire le payload avec votre éditeur de fournisseur d'annonce, en garantissant :
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` avec un `max_span` défini
     - `allow_unknown_capabilities=<true|false>` quand des TLVs GREASE sont présents
   - Valideur via `/v2/sorafs/providers` et `sorafs_fetch` ; les avertissements sur des
     les capacités inconnues doivent être triées.
3. **Valider la préparation multi-source.**
   - Exécuter `sorafs_fetch` avec `--provider-advert=<path>` ; la CLI échoue
     désormais quand `chunk_range_fetch` manque et affiche des avertissements pour des
     capacités inconnues ignorées. Capturer le rapport JSON et l'archiver avec
     les logs d'opérations.
4. **Préparer les renouvellements.**
   - Soumettre des enveloppes `ProviderAdmissionRenewalV1` au moins 30 jours avant
     la passerelle d'application (R2). Les renouvellements doivent conserver le manche
     canonique et l'ensemble des capacités ; seuls le enjeux, les endpoints ou
     les métadonnées doivent changer.
5. **Communiquer avec les équipes dépendantes.**
   - Les propriétaires SDK doivent publier des versions qui exposent des avertissements aux
     opérateurs lorsque les annonces sont rejetées.
   - DevRel annonce chaque transition de phase ; inclure les liens de tableaux de bord
     et la logique de seuil ci-dessous.
6. **Tableaux de bord et alertes de l'installateur.**
   - Importateur l'export Grafana et le placer sous **SoraFS / Provider
     Déploiement** avec l'UID `sorafs-provider-admission`.
   - S'assurer que les règles d'alertes pointent vers le canal partagé
     `sorafs-advert-rollout` en mise en scène et production.

## Télémétrie & tableaux de bord

Les métriques suivantes sont déjà exposées via `iroha_telemetry` :

- `torii_sorafs_admission_total{result,reason}` — compte les acceptés, rejetés
  et avertissements. Les raisons incluent `missing_envelope`, `unknown_capability`,
  `stale`, et `policy_violation`.

Exporter Grafana : [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importez le fichier dans le repo partagé de tableaux de bord (`observability/dashboards`)
et mettre à jour uniquement l'UID de la source de données avant publication.

Le board est publié sous le dossier Grafana **SoraFS / Provider Rollout** avec
l'UID stable `sorafs-provider-admission`. Les règles d'alerte
`sorafs-admission-warn` (avertissement) et `sorafs-admission-reject` (critique) sont
pré-configurées pour utiliser la politique de notification `sorafs-advert-rollout` ;
ajustez ce point de contact si la liste de destinations change plutôt que d'éditer
le JSON du tableau de bord.

Panneaux Grafana recommandés :| Panneau | Requête | Remarques |
|-------|-------|-------|
| **Taux de résultats d'admission** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Graphique de pile pour le visualiseur accepter vs avertir vs rejeter. Alerte quand warn > 0.05 * total (avertissement) ou rejet > 0 (critique). |
| **Taux d'avertissement** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Timeseries à ligne unique qui alimente le seuil pager (taux warning 5 % roulant sur 15 minutes). |
| **Motifs de refus** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Guider le triage du runbook ; attacher des liens vers les étapes de limitation. |
| **Rafraîchir la dette** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Indiquez les fournisseurs qui notent la date limite de rafraîchissement ; croiser avec les logs du cache découverte. |

Artefacts CLI pour les manuels des tableaux de bord :

- `sorafs_fetch --provider-metrics-out` écrit des compteurs `failures`, `successes` et
  `disabled` par fournisseur. Importez dans des tableaux de bord ad hoc pour surveiller
  les dry-runs d'orchestrator avant de basculer les fournisseurs en production.
- Les champs `chunk_retry_rate` et `provider_failure_rate` du rapport JSON
  mettre en avant les throttlings ou symptômes de payloads stale qui précèdent
  souvent les rejets d'admission.

### Mise en page du tableau de bord Grafana

Observabilité publie un board dédié — **SoraFS Provider Admission
Déploiement** (`sorafs-provider-admission`) — sous **SoraFS / Déploiement du fournisseur**
avec les ID de panel canoniques suivants :

- Panel 1 — *Taux de résultat d'admission* (zone empilée, unité "ops/min").
- Panel 2 — *Warning ratio* (single series), émettant l'expression
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   somme(taux(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Motifs de rejet* (séries temporelles regroupées par `reason`), triées par
  `rate(...[5m])`.
- Panel 4 — *Refresh dette* (stat), reprenant la requête du tableau ci-dessus et
  annoté avec les délais de rafraîchissement des annonces extraites du grand livre de migration.

Copiez (ou créez) le squelette JSON dans le repo de tableaux de bord d'infra à
`observability/dashboards/sorafs_provider_admission.json`, puis mis à jour
uniquement l'UID de la source de données ; les IDs de panel et les règles d'alertes sont
référencés par les runbooks ci-dessous, évitez donc de les renuméroter sans
mettre à jour cette documentation.

Pour plus de commodité, le repo fournit une définition de tableau de bord de référence à
`docs/source/grafana_sorafs_admission.json` ; copiez-la dans votre dossier Grafana
si vous avez besoin d'un point de départ pour des tests locaux.

### Règles d'alerte Prometheus

Ajouter le groupe de règles suivant à
`observability/prometheus/sorafs_admission.rules.yml` (créez le fichier si c'est
le premier groupe de règles SoraFS) et incluez-le dans votre configuration
Prometheus. Remplacez `<pagerduty>` par le label de routage réel pour votre
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

Exécutez `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
avant de pousser les changements pour vérifier que la syntaxe passe
`promtool check rules`.

## Matrice de déploiement| Caractéristiques de l'annonce | R0 | R1 | R2 | R3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` présent, alias canoniques, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Absence de capacité `chunk_range_fetch` | ⚠️ Avertir (ingérer + télémétrie) | ⚠️ Avertir | ❌ Rejeter (`reason="missing_capability"`) | ❌ Rejeter |
| TLV de capacité inconnue sans `allow_unknown_capabilities=true` | ✅ | ⚠️ Avertir (`reason="unknown_capability"`) | ❌ Rejeter | ❌ Rejeter |
| `refresh_deadline` expiré | ❌ Rejeter | ❌ Rejeter | ❌ Rejeter | ❌ Rejeter |
| `signature_strict=false` (diagnostic des luminaires) | ✅ (développement uniquement) | ⚠️ Avertir | ⚠️ Avertir | ❌ Rejeter |

Toutes les heures utilisent UTC. Les dates d'exécution sont reflétées dans le
migration ledger et ne bougeront pas sans vote du conseil ; tout changement
nécessite la mise à jour de ce fichier et du grand livre dans la même PR.

> **Note d'implémentation :** R1 introduit la série `result="warn"` dans
> `torii_sorafs_admission_total`. Le patch d'ingestion Torii qui ajoute le nouveau
> label est suivi avec les tâches de télémétrie SF-2 ; jusque-là, utilisez le

## Communication & gestion des incidents

- **Mailer hebdomadaire de statut.** DevRel diffuse un bref résumé des métriques
  d'admission, des avertissements en cours et des dates limites à venir.
- **Réponse incident.** Si les alertes `reject` se déclenchent, l'on-call :
  1. Récupère l'annonce fautif via Discovery Torii (`/v2/sorafs/providers`).
  2. Relancer la validation de l'annonce dans le pipeline supplier et comparer avec
     `/v2/sorafs/providers` pour reproduire l'erreur.
  3. Coordonnez avec le fournisseur pour faire tourner l'annonce avant la prochaine
     date limite de rafraîchissement.
- **Gel des changements.** Aucune modification du schéma de capacités pendant
  R1/R2 à moins que le comité de déploiement ne valide ; les essais GREASE doivent
  être planifié durant la fenêtre de maintenance hebdomadaire et journalisés
  dans le registre des migrations.

##Référence

- [Protocole nœud/client SoraFS](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Politique d'admission des fournisseurs](./provider-admission-policy)
- [Feuille de route de migration](./migration-roadmap)
- [Extensions multi-sources d'annonce de fournisseur] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)