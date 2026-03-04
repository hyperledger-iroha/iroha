---
lang: fr
direction: ltr
source: docs/portal/docs/da/replication-policy.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fuente canonica
Refleja `docs/source/da/replication_policy.md`. Mantenga ambas versions fr
:::

# Politique de réplication de la disponibilité des données (DA-4)

_État : En progreso -- Responsables : Core Protocol WG / Storage Team / SRE_

Le pipeline d'ingesta DA maintenant appliqué aux objectifs de rétention déterminés pour
Chaque classe de blob est décrite dans `roadmap.md` (flux de travail DA-4). Torii rechaza
persister les enveloppes de rétention réservées à l'appelant qui ne coïncide pas avec la
politique configurée, garantizando que cada nodo validador/almacenamiento retiene
le nombre requis d’époques et de répliques ne dépend pas de l’intention de l’émetteur.

## Politique par défaut

| Classe de blob | Rétention chaude | Rétention à froid | Répliques requises | Classe de stockage | Étiquette de gouvernement |
|--------------|---------------|----------------|---------------------|------------------------------|-------------------|
| `taikai_segment` | 24 heures | 14 jours | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 heures | 7 jours | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 heures | 180 jours | 3 | `cold` | `da.governance` |
| _Par défaut (toutes les classes)_ | 6 heures | 30 jours | 3 | `warm` | `da.default` |Ces valeurs sont incrustan en `torii.da_ingest.replication_policy` et sont appliquées à
todas las sollicitudes `/v1/da/ingest`. Torii réécrire les manifestes avec le profil de
retenir l'impôt et émettre une publicité lorsque les appelants entrent des valeurs
Il n'y a aucune coïncidence pour que les opérateurs détectent des SDK désactualisés.

### Classes de disponibilité Taikai

Les manifestes d'enrutamiento Taikai (`taikai.trm`) déclarent un
`availability_class` (`hot`, `warm`, ou `cold`). Torii application politique
correspondant avant le chunking pour que les opérateurs puissent intensifier le
conteo de répliques pour stream sans éditer la table globale. Valeurs par défaut :

| Classe de disponibilité | Rétention chaude | Rétention à froid | Répliques requises | Classe de stockage | Étiquette de gouvernement |
|------------------------------|---------------|----------------|---------------------|-------------------------|-------------------|
| `hot` | 24 heures | 14 jours | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 heures | 30 jours | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 heure | 180 jours | 3 | `cold` | `da.taikai.archive` |

Les pistes fausses utilisent `hot` par défaut pour les transmissions en vivo
retengan la politique plus forte. Surcrire les valeurs par défaut via
`torii.da_ingest.replication_policy.taikai_availability` si su red usa objectifs
différentes.

## ConfigurationLa politique vive sous `torii.da_ingest.replication_policy` et expose un modèle
*par défaut* mais un paramètre de remplacement par classe. Los identificadores de classe no
fils sensibles à mayus/minus et aceptan `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, ou `custom:<u16>` pour extensions approuvées par le gouvernement.
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

Deje el bloque intacto para usar los defaults listados arriba. Para endurant
une classe, actualise le remplacement du correspondant ; pour changer la base de nouvelles
classes, édition `default_retention`.

Les classes de disponibilité Taikai peuvent être inscrites de manière indépendante
via `torii.da_ingest.replication_policy.taikai_availability` :

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

- Torii remplace le `RetentionPolicy` à condition que l'utilisateur contienne le profil
  impuesto avant le chunking ou l’émission de manifestes.
- Les manifestes préconstruits qui déclarent un profil de rétention distinct se
  rechazan avec `400 schema mismatch` pour que les clients obsolètes ne puissent pas
  débiliter le contrat.
- Chaque événement de dérogation est enregistré (`blob_class`, politique enviée ou attendue)
  pour les appelants exponeurs non conformes lors du déploiement.

Ver [Plan de ingesta de Data Availability](ingest-plan.md) (liste de contrôle de validation)
pour la porte actualisée qui cubre l'application de la rétention.

## Flux de réplication (suite à DA-4)L’application de la détention n’est que la première étape. Les opérateurs tambien deben
vérifier que les manifestations en vivo et les ordres de réplication se maintiennent
alignés avec la politique configurée pour que SoraFS puisse re-répliquer les blobs
hors du cumul de forme automatique.

1. **Vigile el drift.** Torii émet
   `overriding DA retention policy to match configured network baseline` quand
   un appelant envoie des valeurs de rétention désactualisées. Empareje ese log con
   la télémétrie `torii_sorafs_replication_*` pour détecter les défauts de réplique
   o redéploye les demorados.
2. **Différence d'intention par rapport aux répliques en vivo.** Utilisez le nouvel assistant d'auditoire :

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   La commande charge `torii.da_ingest.replication_policy` depuis la configuration
   provista, décodifier chaque manifeste (JSON ou Norito), et éventuellement empareja
   charges utiles `ReplicationOrderV1` pour le résumé du manifeste. El CV marca dos
   conditions:

   - `policy_mismatch` - le profil de rétention du manifeste diverge de la
     politique impuesta (esto no deberia ocurrir salvo que Torii este mal
     configuré).
   - `replica_shortfall` - l'ordre de réplication en vivo demande moins de répliques
     que `RetentionPolicy.required_replicas` ou entrega moins d'attributions que vous
     objectif.Un statut de sortie sans zéro indique un actif faible pour l'automatisation
   CI/on-call peut paginer immédiatement. Ajouter le rapport JSON au paquet
   `docs/examples/da_manifest_review_template.md` pour les votes du Parlement.
3. **Dispare re-replicacion.** Lorsque l'auditoire signale un échec, il émet un
   nouvelle `ReplicationOrderV1` via les outils de gouvernance décrits en
   [Marché de capacité de stockage SoraFS](../sorafs/storage-capacity-marketplace.md)
   et vuelva à exécuter la salle jusqu'à ce que l'ensemble des répliques soit converti. Para
   remplacements d'urgence, empareje la sortie de la CLI avec `iroha app da prove-availability`
   pour que les SRE puissent référencer le même résumé et les preuves PDP.

La couverture de régression vive en
`integration_tests/tests/da/replication_policy.rs` ; la suite envoie une politique de
la rétention ne coïncide pas avec `/v1/da/ingest` et vérifie que le manifeste obtenu
expose le profil impuesto à l’endroit de l’intention de l’appelant.