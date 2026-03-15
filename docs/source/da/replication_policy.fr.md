---
lang: fr
direction: ltr
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T15:38:30.661849+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Politique de réplication de disponibilité des données (DA-4)

_Statut : En cours – Propriétaires : Core Protocol WG / Storage Team / SRE_

Le pipeline d'ingestion DA applique désormais des objectifs de rétention déterministes pour
chaque classe blob décrite dans `roadmap.md` (flux de travail DA-4). Torii refuse de
conserver les enveloppes de rétention fournies par l'appelant qui ne correspondent pas à la configuration
politique, garantissant que chaque validateur/nœud de stockage conserve les
nombre d'époques et de répliques sans compter sur l'intention de l'émetteur.

## Politique par défaut

| Classe Blob | Rétention à chaud | Rétention au froid | Répliques requises | Classe de stockage | Étiquette de gouvernance |
|------------|--------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 heures | 14 jours | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 heures | 7 jours | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 heures | 180 jours | 3 | `cold` | `da.governance` |
| _Par défaut (toutes les autres classes)_ | 6 heures | 30 jours | 3 | `warm` | `da.default` |

Ces valeurs sont intégrées dans `torii.da_ingest.replication_policy` et appliquées à
toutes les soumissions `/v2/da/ingest`. Torii réécrit les manifestes avec l'application
profil de rétention et émet un avertissement lorsque les appelants fournissent des valeurs incompatibles afin
les opérateurs peuvent détecter les SDK obsolètes.

### Cours de disponibilité de Taikai

Les manifestes de routage Taikai (métadonnées `taikai.trm`) incluent désormais un
Indice `availability_class` (`Hot`, `Warm` ou `Cold`). Lorsqu'il est présent, Torii
sélectionne le profil de rétention correspondant dans `torii.da_ingest.replication_policy`
avant de diviser la charge utile, permettant aux opérateurs d'événements de rétrograder les éléments inactifs
rendus sans modifier la table de stratégie globale. Les valeurs par défaut sont :

| Classe de disponibilité | Rétention à chaud | Rétention au froid | Répliques requises | Classe de stockage | Étiquette de gouvernance |
|------------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 heures | 14 jours | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 heures | 30 jours | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 heure | 180 jours | 3 | `cold` | `da.taikai.archive` |

Si le manifeste omet `availability_class`, le chemin d'ingestion revient au
Profil `hot` afin que les flux en direct conservent leur jeu de répliques complet. Les opérateurs peuvent
remplacez ces valeurs en modifiant le nouveau
Bloc `torii.da_ingest.replication_policy.taikai_availability` en configuration.

##Configuration

La politique réside sous `torii.da_ingest.replication_policy` et expose un
Modèle *par défaut* plus un tableau de remplacements par classe. Les identifiants de classe sont
ne respecte pas la casse et accepte `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact` ou `custom:<u16>` pour les extensions approuvées par la gouvernance.
Les classes de stockage acceptent `hot`, `warm` ou `cold`.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

Laissez le bloc intact pour exécuter avec les valeurs par défaut répertoriées ci-dessus. Pour serrer un
classe, mettez à jour le remplacement correspondant ; pour changer la ligne de base pour les nouvelles classes,
modifier `default_retention`.Pour ajuster des classes de disponibilité Taikai spécifiques, ajoutez des entrées sous
`torii.da_ingest.replication_policy.taikai_availability` :

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## Sémantique d'application

- Torii remplace le `RetentionPolicy` fourni par l'utilisateur par le profil appliqué
  avant le découpage ou l’émission manifeste.
- Les manifestes prédéfinis qui déclarent un profil de rétention incompatible sont rejetés
  avec `400 schema mismatch` afin que les clients périmés ne puissent pas affaiblir le contrat.
- Chaque événement de remplacement est enregistré (`blob_class`, politique soumise par rapport à celle attendue)
  pour détecter les appelants non conformes lors du déploiement.

Voir `docs/source/da/ingest_plan.md` (liste de contrôle de validation) pour la porte mise à jour
couvrant l’application de la rétention.

## Workflow de réplication (suivi DA-4)

L’application des règles de rétention n’est que la première étape. Les opérateurs doivent également prouver que
les manifestes en direct et les ordres de réplication restent alignés sur la politique configurée afin
que SoraFS peut automatiquement répliquer à nouveau les blobs non conformes.

1. **Surveillez la dérive.** Torii émet
   `overriding DA retention policy to match configured network baseline` à chaque fois
   un appelant soumet des valeurs de rétention obsolètes. Associez ce journal à
   Télémétrie `torii_sorafs_replication_*` pour détecter les déficits ou les retards des répliques
   redéploiements.
2. **Intention différente par rapport aux réplicas actifs.** Utilisez le nouvel assistant d'audit :

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   La commande charge `torii.da_ingest.replication_policy` à partir du fichier fourni
   config, décode chaque manifeste (JSON ou Norito) et correspond éventuellement à n'importe quel
   Charges utiles `ReplicationOrderV1` par résumé du manifeste. Le résumé signale deux
   conditions :

   - `policy_mismatch` – le profil de rétention manifeste diverge de celui appliqué
     politique (cela ne devrait jamais se produire à moins que Torii ne soit mal configuré).
   - `replica_shortfall` – l'ordre de réplication en direct demande moins de répliques que
     `RetentionPolicy.required_replicas` ou fournit moins d'affectations que son
     cible.

   Un statut de sortie différent de zéro indique un déficit actif donc une automatisation CI/sur appel
   peut pager immédiatement. Joignez le rapport JSON au
   Paquet `docs/examples/da_manifest_review_template.md` pour les votes du Parlement.
3. **Déclenchez la re-réplication.** Lorsque l'audit signale un déficit, émettez une nouvelle
   `ReplicationOrderV1` via les outils de gouvernance décrits dans
   `docs/source/sorafs/storage_capacity_marketplace.md` et réexécutez l'audit
   jusqu'à ce que le jeu de répliques converge. Pour les remplacements d'urgence, associez la sortie CLI
   avec `iroha app da prove-availability` afin que les SRE puissent référencer le même résumé
   et les preuves PDP.

La couverture de régression réside dans `integration_tests/tests/da/replication_policy.rs` ;
la suite soumet une politique de rétention qui ne correspond pas à `/v2/da/ingest` et vérifie
que le manifeste récupéré expose le profil appliqué au lieu de l'appelant
intention.

## Télémétrie et tableaux de bord de preuve de santé (pont DA-5)

L'élément **DA-5** de la feuille de route exige que les résultats de l'application des PDP/PoTR soient auditables dans
en temps réel. Les événements `SorafsProofHealthAlert` pilotent désormais un ensemble dédié de
Métriques Prometheus :

-`torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
-`torii_sorafs_proof_health_pdp_failures{provider_id}`
-`torii_sorafs_proof_health_potr_breaches{provider_id}`
-`torii_sorafs_proof_health_penalty_nano{provider_id}`
-`torii_sorafs_proof_health_cooldown{provider_id}`
-`torii_sorafs_proof_health_window_end_epoch{provider_id}`

La carte **SoraFS PDP & PoTR Health** Grafana
(`dashboards/grafana/sorafs_pdp_potr_health.json`) expose désormais ces signaux :- *Proof Health Alerts by Trigger* trace les taux d'alerte par déclencheur/indicateur de pénalité afin
  Les opérateurs Taikai/CDN peuvent prouver si les frappes PDP uniquement, PoTR uniquement ou doubles sont
  tir.
- *Providers in Cooldown* rapporte la somme en direct des fournisseurs actuellement sous un
  Temps de recharge de SorafsProofHealthAlert.
- *Proof Health Window Snapshot* fusionne les compteurs PDP/PoTR, le montant de la pénalité,
  indicateur de temps de recharge et époque de fin de fenêtre de grève par fournisseur afin que les réviseurs de gouvernance
  peut joindre le tableau aux paquets d’incidents.

Les runbooks doivent relier ces panneaux lors de la présentation des preuves d’application du DA ; ils
lier les échecs du flux de preuve CLI directement aux métadonnées de pénalité en chaîne et
fournir le crochet d'observabilité mentionné dans la feuille de route.