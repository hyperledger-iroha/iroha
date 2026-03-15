---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: "SoraFS پرووائیڈر advert رول آؤٹ اور مطابقتی پلان"
---

> [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) en ligne

# SoraFS پرووائیڈر advert رول آؤٹ اور مطابقتی پلان

Il existe des annonces de fournisseurs permissives et des annonces régies par `ProviderAdvertV1`
surface et cut-over pour la récupération de fragments multi-sources et la récupération de fragments multi-sources
ضروری ہے۔ Voici les livrables de votre projet :

- **Guide de l'opérateur.** et les fournisseurs de stockage ainsi que la porte d'entrée et les fournisseurs de stockage.
  سے پہلے مکمل کرنا ہیں۔
- **Couverture de télémétrie.** tableaux de bord et alertes en plus Observabilité et Ops en cours
  Il y a des annonces conformes aux normes en vigueur
  Les SDK et les outils d'outillage sont publiés plus tard

یہ déploiement [Feuille de route de migration SoraFS] (./migration-roadmap) et jalons SF-2b/2c
کے ساتھ align ہے اور فرض کرتا ہے کہ [politique d'admission du fournisseur] (./provider-admission-policy)
پہلے سے نافذ ہے۔

## Chronologie des phases

| Phases | Fenêtre (cible) | Comportement | Actions de l'opérateur | Focus sur l'observabilité |
|-------|-----------------|-----------|------------------|-------------------|

## Liste de contrôle de l'opérateur

1. **Annonces d'inventaire.** Une annonce publiée est une annonce publicitaire :
   - Chemin de l'enveloppe directrice (équivalent de production `defaults/nexus/sorafs_admission/...` یا).
   - annonce `profile_id` et `profile_aliases`.
   - liste des capacités (کم از کم `torii_gateway` et `chunk_range_fetch`).
   - Indicateur `allow_unknown_capabilities` (TLV réservés au fournisseur par exemple)
2. **Régénérez avec les outils du fournisseur.**
   - L'éditeur d'annonces du fournisseur et la charge utile sont également disponibles:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` et `max_span`
     - TLV GRAISSE pour `allow_unknown_capabilities=<true|false>`
   - `/v1/sorafs/providers` et `sorafs_fetch` pour valider la commande inconnu
     capacités, avertissements, triage,
3. **Valider la préparation multi-sources.**
   - `sorafs_fetch` et `--provider-advert=<path>` pour le produit Par `chunk_range_fetch`
     Il y a un échec de la CLI ou des capacités inconnues ignorées ou des avertissements.
     Rapport JSON pour les journaux d'opérations et les archives d'archives
4. **Renouvellements d'étapes.**
   - application de la passerelle (R2) pendant 30 jours `ProviderAdmissionRenewalV1`
     enveloppes جمع کریں۔ renouvellements میں handle canonique et ensemble de capacités برقرار
     رہنا چاہئے؛ صرف enjeux, points de terminaison et métadonnées
5. **Communiquer avec les équipes dépendantes.**
   - Les propriétaires de SDK publient des versions et des publicités rejettent les opérateurs et les avertissements.
   - DevRel annonce la transition de phase liens vers le tableau de bord et logique de seuil
6. **Installer des tableaux de bord et des alertes.**
   - Grafana export امپورٹ کریں اور **SoraFS / Provider Rollout** تحت رکھیں، UID
     `sorafs-provider-admission`
   - Règles d'alerte de mise en scène et de production partagées
     Canal de notification `sorafs-advert-rollout` en anglais

## Télémétrie et tableaux de bord

Les métriques du `iroha_telemetry` sont les suivantes :- `torii_sorafs_admission_total{result,reason}` — accepté, rejeté et avertissement
  résultats گنتا ہے۔ raisons `missing_envelope`, `unknown_capability`, `stale`
  Pour `policy_violation` ہیں۔

Exportation Grafana : [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Comment importer un référentiel de tableaux de bord partagés (`observability/dashboards`)
L'UID de la source de données est également disponible.

Voir le dossier Grafana **SoraFS / Provider Rollout** avec UID stable
`sorafs-provider-admission` کے ساتھ publier ہوتا ہے۔ règles d'alerte
`sorafs-admission-warn` (avertissement) et `sorafs-admission-reject` (critique)
La politique de notification `sorafs-advert-rollout` est configurée pour être configurée
Ajouter la liste de destinations au tableau de bord JSON et modifier le point de contact
mettre à jour کریں۔

Panneaux Grafana recommandés :

| Panneau | Requête | Remarques |
|-------|-------|-------|
| **Taux de résultats d'admission** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Graphique de pile et accepter vs avertir vs rejeter warn > 0,05 * total (avertissement) یا rejet > 0 (critique) پر alerte۔ |
| **Taux d'avertissement** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Série temporelle sur une seule ligne et seuil de téléavertisseur et flux d'alimentation (taux d'avertissement de 15 à 5 %) |
| **Motifs de refus** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | triage des runbooks mesures d'atténuation |
| **Rafraîchir la dette** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | actualiser la date limite manquer les fournisseurs et les fournisseurs journaux de cache de découverte et références croisées |

Tableaux de bord manuels et artefacts CLI :

- `sorafs_fetch --provider-metrics-out` et fournisseur کے لیے `failures`, `successes`,
  اور `disabled` compteurs لکھتا ہے۔ essais à sec d'orchestrateur et moniteur
  tableaux de bord ad hoc pour importer des fichiers
- Rapport JSON concernant la limitation des champs `chunk_retry_rate` et `provider_failure_rate`.
  symptômes de charge utile obsolètes et les rejets d'admission

### Disposition du tableau de bord Grafana

Carte dédiée à l'observabilité - ** Admission du fournisseur SoraFS
Déploiement** (`sorafs-provider-admission`) — **SoraFS / Déploiement du fournisseur**
publier les identifiants canoniques des panneaux et les identifiants canoniques :

- Panel 1 — *Taux de résultat d'admission* (zone empilée, یونٹ "ops/min").
- Panneau 2 — *Taux d'avertissement* (série unique)، اظہار
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   somme(taux(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Motifs de rejet* (`reason` ou séries chronologiques)، `rate(...[5m])`
  کے مطابق trier کی گئی۔
- Panel 4 — *Actualiser la dette* (stat) et requête de table et miroir et requête de table
  registre de migration et délais d'actualisation des annonces et annoté

Squelette JSON et dépôt de tableaux de bord d'infrastructure par `observability/dashboards/sorafs_provider_admission.json`
Pour copier ou créer un UID de source de données supplémentaire. ID de panneau et règles d'alerte
Les runbooks sont référencés et les documents sont renumérotés.

Tableau de bord de référence `docs/source/grafana_sorafs_admission.json`
définition دیتا ہے؛ Comment tester le dossier Grafana et copier le dossier

### Règles d'alerte PrometheusGroupe de règles associé à `observability/prometheus/sorafs_admission.rules.yml`
Groupe de règles (groupe de règles SoraFS et groupe de règles Prometheus)
configuration سے inclure کریں۔ `<pagerduty>` pour étiquette de routage d'astreinte

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

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
Voici la syntaxe `promtool check rules` de la syntaxe `promtool check rules` de la syntaxe `promtool check rules`.

## Communication et gestion des incidents

- ** Mailer d'état hebdomadaire. ** Mesures d'admission DevRel, avertissements en suspens et délais et délais de livraison
- **Réponse aux incidents.** Alertes `reject` en cas d'astreinte :
  1. Découverte Torii (`/v1/sorafs/providers`) et récupération d'annonces incriminées
  2. Pipeline du fournisseur pour la validation de l'annonce et pour `/v1/sorafs/providers` pour comparer et reproduire l'erreur.
  3. Le fournisseur coordonne les coordonnées du fournisseur pour le délai d'actualisation et la rotation de l'annonce.
- **Le changement se fige.** Le schéma de capacités R1/R2 est en cours de déploiement en cours de déploiement. Essais GREASE et fenêtre de maintenance et calendrier et grand livre de migration et journal de bord

## Références

- [Protocole nœud/client SoraFS](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Politique d'admission des fournisseurs](./provider-admission-policy)
- [Feuille de route de migration](./migration-roadmap)
- [Extensions multi-sources d'annonce de fournisseur] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)