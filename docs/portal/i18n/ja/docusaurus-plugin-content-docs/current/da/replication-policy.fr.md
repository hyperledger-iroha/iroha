---
lang: ja
direction: ltr
source: docs/portal/docs/da/replication-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Source canonique
Reflete `docs/source/da/replication_policy.md`. Gardez les deux versions en
:::

# Politique de replication Data Availability (DA-4)

_Statut: En cours -- Responsables: Core Protocol WG / Storage Team / SRE_

Le pipeline d'ingest DA applique maintenant des objectifs de retention
deterministes pour chaque classe de blob decrite dans `roadmap.md` (workstream
DA-4). Torii refuse de persister des envelopes de retention fournis par le
caller qui ne correspondent pas a la politique configuree, garantissant que
chaque noeud validateur/stockage retient le nombre requis d'epoques et de
replicas sans dependance a l'intention de l'emetteur.

## Politique par defaut

| Classe de blob | Retention hot | Retention cold | Replicas requises | Classe de stockage | Tag de gouvernance |
|---------------|---------------|----------------|-------------------|--------------------|--------------------|
| `taikai_segment` | 24 heures | 14 jours | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 heures | 7 jours | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 heures | 180 jours | 3 | `cold` | `da.governance` |
| _Default (toutes les autres classes)_ | 6 heures | 30 jours | 3 | `warm` | `da.default` |

Ces valeurs sont integrees dans `torii.da_ingest.replication_policy` et appliquees
a toutes les submissions `/v1/da/ingest`. Torii recrit les manifests avec le
profil de retention impose et emet un avertissement quand les callers fournissent
des valeurs incoherentes afin que les operateurs detectent des SDKs obsoletes.

### Classes de disponibilite Taikai

Les manifests de routage Taikai (`taikai.trm`) declarent une `availability_class`
(`hot`, `warm`, ou `cold`). Torii applique la politique correspondante avant le
chunking afin que les operateurs puissent ajuster les comptes de replicas par
stream sans editer la table globale. Defaults:

| Classe de disponibilite | Retention hot | Retention cold | Replicas requises | Classe de stockage | Tag de gouvernance |
|-------------------------|---------------|----------------|-------------------|--------------------|--------------------|
| `hot` | 24 heures | 14 jours | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 heures | 30 jours | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 heure | 180 jours | 3 | `cold` | `da.taikai.archive` |

Les hints manquants reviennent a `hot` afin que les broadcasts live retiennent la
politique la plus forte. Remplacez les defaults via
`torii.da_ingest.replication_policy.taikai_availability` si votre reseau utilise
des cibles differentes.

## Configuration

La politique vit sous `torii.da_ingest.replication_policy` et expose un template
*default* plus un tableau d'overrides par classe. Les identifiants de classe sont
case-insensitive et acceptent `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, ou `custom:<u16>` pour des extensions approuvees par la
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

Laissez le bloc intact pour utiliser les defaults ci-dessus. Pour durcir une
classe, mettez a jour l'override correspondant; pour changer la base pour de
nouvelles classes, editez `default_retention`.

Les classes de disponibilite Taikai peuvent etre surchargees independamment via
`torii.da_ingest.replication_policy.taikai_availability`:

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

## Semantique d'enforcement

- Torii remplace le `RetentionPolicy` fourni par l'utilisateur par le profil
  impose avant le chunking ou l'emission de manifest.
- Les manifests preconstruits qui declarent un profil de retention different
  sont rejetes avec `400 schema mismatch` afin que les clients obsoletes ne
  puissent pas affaiblir le contrat.
- Chaque evenement d'override est logge (`blob_class`, policy soumise vs attendue)
  pour mettre en evidence les callers non conformes pendant le rollout.

Voir [Data Availability Ingest Plan](ingest-plan.md) (Validation checklist) pour
le gate mis a jour couvrant l'enforcement de retention.

## Workflow de re-replication (suivi DA-4)

L'enforcement de retention n'est que la premiere etape. Les operateurs doivent
aussi prouver que les manifests live et les ordres de replication restent
alignes avec la politique configuree afin que SoraFS puisse re-repliquer les blobs
hors conformite automatiquement.

1. **Surveillez le drift.** Torii emet
   `overriding DA retention policy to match configured network baseline` quand
   un caller soumet des valeurs de retention obsoletes. Associez ce log avec la
   telemetrie `torii_sorafs_replication_*` pour reperer des shortfalls de replicas
   ou des redeployments retardes.
2. **Diff intent vs replicas live.** Utilisez le nouvel helper d'audit:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   La commande charge `torii.da_ingest.replication_policy` depuis la config
   fournie, decode chaque manifest (JSON ou Norito), et associe optionnellement
   les payloads `ReplicationOrderV1` par digest de manifest. Le resume signale
   deux conditions:

   - `policy_mismatch` - le profil de retention du manifest diverge du profil
     impose (ceci ne devrait jamais arriver sauf si Torii est mal configure).
   - `replica_shortfall` - l'ordre de replication live demande moins de replicas
     que `RetentionPolicy.required_replicas` ou fournit moins d'assignations que
     sa cible.

   Un status de sortie non zero indique une insuffisance active afin que
   l'automatisation CI/on-call puisse pager immediatement. Joignez le rapport JSON
   au paquet `docs/examples/da_manifest_review_template.md` pour les votes du
   Parlement.
3. **Declenchez la re-replication.** Quand l'audit signale une insuffisance,
   emettez un nouveau `ReplicationOrderV1` via les outils de gouvernance decrits
   dans [SoraFS storage capacity marketplace](../sorafs/storage-capacity-marketplace.md)
   et relancez l'audit jusqu'a convergence du set de replicas. Pour les overrides
   d'urgence, associez la sortie CLI avec `iroha app da prove-availability` afin que
   les SREs puissent referencer le meme digest et la preuve PDP.

La couverture de regression vit dans `integration_tests/tests/da/replication_policy.rs`;
la suite soumet une politique de retention non conforme a `/v1/da/ingest` et verifie
que le manifest recupere expose le profil impose plutot que l'intention du caller.
