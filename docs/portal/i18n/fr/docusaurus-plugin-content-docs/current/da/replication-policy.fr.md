---
lang: fr
direction: ltr
source: docs/portal/docs/da/replication-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Source canonique
Reflète `docs/source/da/replication_policy.md`. Gardez les deux versions fr
:::

# Politique de réplication Disponibilité des données (DA-4)

_Statut : En cours -- Responsables : Core Protocol WG / Storage Team / SRE_

Le pipeline d'ingest DA applique maintenant des objectifs de rétention
déterministes pour chaque classe de blob décrite dans `roadmap.md` (workstream
DA-4). Torii refuser de persister des enveloppes de rétention fournies par le
l'appelant qui ne correspond pas à la politique configurée, garantissant que
chaque noeud validateur/stockage retient le nombre requis d'époques et de
répliques sans dépendance à l'intention de l'émetteur.

## Politique par défaut

| Classe de blob | Rétention chaude | Rétention à froid | Répliques requises | Classe de stockage | Étiquette de gouvernance |
|---------------|---------------|----------------|-------------------|--------------------|--------------------|
| `taikai_segment` | 24 heures | 14 jours | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 heures | 7 jours | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 heures | 180 jours | 3 | `cold` | `da.governance` |
| _Default (toutes les autres classes)_ | 6 heures | 30 jours | 3 | `warm` | `da.default` |Ces valeurs sont intégrées dans `torii.da_ingest.replication_policy` et appliquées
à toutes les soumissions `/v2/da/ingest`. Torii recrit les manifestes avec le
profil de rétention impose et emet un avertissement lorsque les appelants sont fournis
des valeurs incohérentes afin que les opérateurs détectent des SDK obsolètes.

### Classes de disponibilité Taikai

Les manifestes de routage Taikai (`taikai.trm`) déclarent une `availability_class`
(`hot`, `warm`, ou `cold`). Torii applique la politique correspondante avant le
chunking afin que les opérateurs puissent ajuster les comptes de répliques par
stream sans éditeur la table globale. Valeurs par défaut :

| Classe de disponibilité | Rétention chaude | Rétention à froid | Répliques requises | Classe de stockage | Étiquette de gouvernance |
|------------------------------|---------------|----------------|-------------------|--------------------|--------------------|
| `hot` | 24 heures | 14 jours | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 heures | 30 jours | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 heure | 180 jours | 3 | `cold` | `da.taikai.archive` |

Les indices manquants reviennent a `hot` afin que les diffusions en direct retiennent la
politique la plus forte. Remplacez les valeurs par défaut via
`torii.da_ingest.replication_policy.taikai_availability` si votre réseau utilise
des cibles différentes.

##ConfigurationLa politique vit sous `torii.da_ingest.replication_policy` et exposer un modèle
*default* plus un tableau d'overrides par classe. Les identifiants de classe sont
insensible à la casse et acceptant `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, ou `custom:<u16>` pour des extensions approuvées par la
gouvernance. Les classes de stockage acceptent `hot`, `warm`, ou `cold`.

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

Laissez le bloc intact pour utiliser les paramètres par défaut ci-dessus. Pour durcir une
classe, mettre à jour l'override correspondant; pour changer la base pour de
nouvelles classes, éditez `default_retention`.

Les cours de disponibilité Taikai peuvent être surtaxés indépendamment via
`torii.da_ingest.replication_policy.taikai_availability` :

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## Sémantique d'application

- Torii remplace le `RetentionPolicy` fourni par l'utilisateur par le profil
  imposer avant le chunking ou l'émission de manifeste.
- Les manifestes préconstruits qui déclarent un profil de rétention différent
  sont rejetées avec `400 schema mismatch` afin que les clients obsolètes ne
  pourrait pas affaiblir le contrat.
- Chaque événement d'override est logge (`blob_class`, politique soumise vs attendue)
  pour mettre en évidence les appelants non conformes pendant le déploiement.

Voir [Data Availability Ingest Plan](ingest-plan.md) (Liste de contrôle de validation) pour
le gate mis a jour couvrant l'application de la rétention.

## Workflow de re-réplication (suivi DA-4)L'enforcement de rétention n'est que la première étape. Les opérateurs doivent
prouver également que les manifestes vivent et les ordres de réplication restent
s'aligne avec la politique configurée afin que SoraFS puisse re-répliquer les blobs
hors conformité automatiquement.

1. **Surveillez le drift.** Torii emet
   `overriding DA retention policy to match configured network baseline` quand
   un appelant soumet des valeurs de rétention obsolètes. Associez ce log à la
   télémétrie `torii_sorafs_replication_*` pour reperer des manques à gagner de répliques
   ou des redéploiements retardés.
2. **Intention différente vs répliques en direct.** Utilisez le nouvel assistant d'audit :

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   La commande charge `torii.da_ingest.replication_policy` depuis la config
   fourni, décoder chaque manifest (JSON ou Norito), et associer optionnellement
   les payloads `ReplicationOrderV1` par résumé du manifeste. Le signal de reprise
   deux conditions :

   - `policy_mismatch` - le profil de rétention du manifeste diverge du profil
     imposer (ceci ne devrait jamais arriver sauf si Torii est mal configuré).
   - `replica_shortfall` - l'ordre de réplication live demande moins de répliques
     que `RetentionPolicy.required_replicas` ou fournit moins d'assignations que
     sa cible.Un statut de sortie non nul indique une insuffisance active afin que
   l'automatisation CI/on-call peut téléavertir immédiatement. Joignez le rapport JSON
   au paquet `docs/examples/da_manifest_review_template.md` pour les votes du
   Parlement.
3. **Declenchez la re-replication.** Quand l'audit signale une insuffisance,
   donnez un nouveau `ReplicationOrderV1` via les outils de gouvernance décrits
   dans [Place de marché de capacité de stockage SoraFS](../sorafs/storage-capacity-marketplace.md)
   et relancez l'audit jusqu'à la convergence du set de répliques. Pour les remplacements
   d'urgence, associez la sortie CLI avec `iroha app da prove-availability` afin que
   les SRE peuvent référencer le meme digest et la preuve PDP.

La couverture de régression vit dans `integration_tests/tests/da/replication_policy.rs`;
la suite soumet une politique de rétention non conforme à `/v2/da/ingest` et vérifie
que le manifeste récupéré expose le profil impose plutôt que l'intention du caller.