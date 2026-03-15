---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : "Plan de déploiement des annonces des fournisseurs SoraFS"
---

> Adapté de [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# Plan de déploiement des annonces des fournisseurs SoraFS

Ce plan coordonne le basculement des publicités permises par les fournisseurs pour un
surface totalement gouvernée `ProviderAdvertV1` requise pour la récupération
morceaux multi-sources. Il y a trois livrables :

- **Guia de operadores.** Passos que les fournisseurs de stockage précisent conclure avant chaque porte.
- **Cobertura de telemetria.** Tableaux de bord et alertes pour l'observabilité et les opérations
  pour confirmer que la rede aceita apenas adverts est conforme.
  pour équiper les versions du SDK et du plan d'outillage.

Le déploiement est alinha aos marcos SF-2b/2c non
[feuille de route de migration SoraFS](./migration-roadmap) et supposons qu'une politique d'admission
non [politique d'admission du fournisseur](./provider-admission-policy) ja esta ativa.

## Chronologie des phases

| Phase | Janela (alvo) | Comportement | Acoes do opérateur | Foco de observabilité |
|-------|-----------------|-----------|------------------|-------------------|

## Liste de contrôle de l'opérateur

1. **Inventaire des annonces.** Liste de chaque annonce publiée et enregistrée :
   - Caminho do gouvernance enveloppe (`defaults/nexus/sorafs_admission/...` ou équivalente en production).
   - `profile_id` et `profile_aliases` faire de la publicité.
   - Liste des capacités (espérez-le moins `torii_gateway` et `chunk_range_fetch`).
   - Drapeau `allow_unknown_capabilities` (nécessaire quand les TLV sont réservées au vendeur et sont présentes).
2. **Régénérer le fournisseur d'outils de com.**
   - Reconstruisez la charge utile avec votre annonce d'éditeur de fournisseur, garanti :
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` avec `max_span` défini
     - `allow_unknown_capabilities=<true|false>` quando houver TLV GRAISSE
   - Valide via `/v1/sorafs/providers` et `sorafs_fetch` ; avertissements sur les capacités
     desconhecidas devem ser triageadas.
3. **Préparation valide multi-source.**
   - Exécuter `sorafs_fetch` avec `--provider-advert=<path>` ; o CLI agora falha quando
     `chunk_range_fetch` est ausente et affiche des avertissements pour les capacités découvertes
     ignorants. Capturez ou rapportez JSON et archivez les journaux d'exploitation.
4. **Préparer les rénovations.**
   - Envie enveloppes `ProviderAdmissionRenewalV1` pelo menos 30 dias antes do
     application sans passerelle (R2). Rénovacoes devem manter o handle canonico e o
     définir les capacités ; Il y a un enjeu, des points de terminaison ou des métadonnées à développer.
5. **Comunicar équipe les personnes dépendantes.**
   - Donos de SDK devem lancar versoes que mostrem warns aos opérateurs quando
     annonces forem rejeitados.
   - DevRel annonce chaque phase de transition ; inclut des liens de tableaux de bord et une logique
     de seuils abaixo.
6. **Installer des tableaux de bord et des alertes.**
   - Importer ou exporter Grafana et coloque sur **SoraFS / Provider Rollout** avec UID
     `sorafs-provider-admission`.
   - Garanta que as regras de alert apontem para o canal partagelhado
     `sorafs-advert-rollout` dans la mise en scène et la production.

## Télémétrie et tableaux de bord

Comme suit les mesures et ces informations via `iroha_telemetry` :- `torii_sorafs_admission_total{result,reason}` - avec des aceitos, des rejeitados et
  avertissements. Les motifs incluent `missing_envelope`, `unknown_capability`, `stale`
  et `policy_violation`.

Exportation Grafana : [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importer l'archive dans le référentiel de comparaison des tableaux de bord (`observability/dashboards`)
Il est possible d'actualiser l'UID de la source de données avant de la publier.

Le tableau et publié sur les pâtes Grafana **SoraFS / Provider Rollout** avec l'UID
estavel `sorafs-provider-admission`. Comme regras d'alerte
`sorafs-admission-warn` (avertissement) et `sorafs-admission-reject` (critique) estao
préconfigurés pour utiliser la politique de notification `sorafs-advert-rollout` ; ajuster
C'est un point de contact pour changer de destination, lorsque vous éditez le JSON sur le tableau de bord.

Peintures Grafana recommandés:

| Panneau | Requête | Remarques |
|-------|-------|-------|
| **Taux de résultats d'admission** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Graphique de pile pour visualiser accepter, avertir ou rejeter. Alerte lorsque warn > 0.05 * total (avertissement) ou rejet > 0 (critique). |
| **Taux d'avertissement** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Série temporelle d'une ligne unique qui alimente le seuil du téléavertisseur (taux d'avertissement de 5 % pendant 15 minutes). |
| **Motifs de refus** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Guia triage du runbook ; liens anexe para mitigacoes. |
| **Rafraîchir la dette** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Les fournisseurs Indica qui perdent la date limite de rafraîchissement ; Les journaux Cruze Com font un cache de découverte. |

Artefacts de CLI pour les manuels de tableaux de bord :

- `sorafs_fetch --provider-metrics-out` écrire des contadores `failures`, `successes`
  e `disabled` par le fournisseur. Importer des tableaux de bord ad hoc pour surveiller les essais à sec
  faire l'orchestrateur avant les fournisseurs de trocarts en production.
- Les champs `chunk_retry_rate` et `provider_failure_rate` signalent la caméra JSON
  limitation ou symptômes de charges utiles obsolètes qui nécessitent des refus d'admission.

### Mise en page du tableau de bord Grafana

Observabilité publiée sur un forum dédié - **SoraFS Admission du fournisseur
Déploiement** (`sorafs-provider-admission`) - sanglot **SoraFS / Déploiement du fournisseur**
avec les ID suivants canoniques du panneau :

- Panel 1 - *Taux de résultat d'admission* (zone empilée, unidade "ops/min").
- Panneau 2 - *Taux d'avertissement* (série unique), avec expressao
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   somme(taux(torii_sorafs_admission_total[5m]))`.
- Panel 3 - *Raisons de rejet* (séries chronologiques agrégées par `reason`), ordonnées par
  `rate(...[5m])`.
- Panel 4 - *Actualiser la dette* (stat), espelha a query da tabela acima e e anotada
  com actualiser les délais dos adverts extraidos do migration ledger.

Copiez (ou criez) le squelette JSON dans le dépôt des tableaux de bord ci-dessous
`observability/dashboards/sorafs_provider_admission.json`, après avoir actualisé les apenas
o UID de la source de données ; os IDs de panel et regras d'alerte sao referenciados pelos
Runbooks abaixo, afin d'éviter de renumérer sans réviser cette documentation.

Pour plus de commodité, le dépôt comprend une définition de tableau de bord de référence dans
`docs/source/grafana_sorafs_admission.json` ; copie para sua pasta Grafana se
préciser un point de départ pour les testicules locaux.

### Regras d'alerte PrometheusAdicione o seguinte grupo de regras em
`observability/prometheus/sorafs_admission.rules.yml` (crie ou arquivo se este pour
Le premier groupe de regras SoraFS) et inclut la configuration du Prometheus.
Substitua `<pagerduty>` pelo label de roteamento real da sua rotacao on-call.

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

Exécuter `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
avant d'envoyer des modifications pour garantir que la syntaxe passe `promtool check rules`.

## Matrice de déploiement

| Caractéristiques de l'annonce | R0 | R1 | R2 | R3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` présente, alias canonicos, `signature_strict=true` | D'accord | D'accord | D'accord | D'accord |
| Ausencia de `chunk_range_fetch` capacité | WARN (ingest + télémétrie) | AVERTIR | REJETER (`reason="missing_capability"`) | REJETER |
| TLV de capacité détectée sans `allow_unknown_capabilities=true` | D'accord | AVERTIR (`reason="unknown_capability"`) | REJETER | REJETER |
| `refresh_deadline` expiré | REJETER | REJETER | REJETER | REJETER |
| `signature_strict=false` (appareils de diagnostic) | OK (points de développement) | AVERTIR | AVERTIR | REJETER |

Tous les horaires sont en UTC. Données de mise en application sao reflètent pas de migration
grand livre et nao mudam sem voto do conseil; Qu'importe quoi demander d'actualiser cet élément
archiver et le grand livre dans mon PR.

> **Note de mise en œuvre :** R1 introduit la série `result="warn"` dans
> `torii_sorafs_admission_total`. Le patch d'ingestao do Torii qui ajoute
> nouvelle étiquette et accompagné des tares de télémétrie SF-2 ; mangé à la, utiliser

## Communication et traitement des incidents

- ** Mailer de statut hebdomadaire. ** DevRel partage un résumé des mesures d'admission,
  avertissements pendants et délais proches.
- **Réponse aux incidents.** Se alerte `reject` dispararem, ingénieurs de garde :
  1. Buscam ou publicité ofensivo via la découverte Torii (`/v1/sorafs/providers`).
  2. Réexécuter la validation de l'annonce sans pipeline du fournisseur et comparaison avec
     `/v1/sorafs/providers` pour reproduire une erreur.
  3. Coordonnez le fournisseur de la rotation de l'annonce avant la date limite d'actualisation la plus proche.
- **Le changement se fige.** Nenhuma ne modifie aucun schéma de capacités pendant R1/R2 a
  moins que le comité de déploiement approuve ; essais GREASE devem ser agendados na
  Janela Semanal de Manutencao e registrados no migration ledger.

## Références

- [Protocole nœud/client SoraFS](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Politique d'admission des fournisseurs](./provider-admission-policy)
- [Feuille de route de migration](./migration-roadmap)
- [Extensions multi-sources d'annonce de fournisseur] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)