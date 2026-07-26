---
lang: fr
direction: ltr
source: docs/portal/docs/da/replication-policy.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note مستند ماخذ
ریٹائر ہونے تک دونوں ورژنز کو sync رکھیں۔
:::

# Politique de réplication de disponibilité des données (DA-4)

_حالت : En cours -- Propriétaires : Core Protocol WG / Storage Team / SRE_

Pipeline d'ingestion DA comme `roadmap.md` (flux de travail DA-4) avec classe blob
Voir les objectifs de rétention déterministes Torii rétention
les enveloppes persistent et sont configurées pour que l'appelant soit configuré
politique et correspondance entre les époques et le validateur/nœud de stockage
les répliques sont censées conserver l'intention de l'émetteur

## Politique par défaut

| Classe Blob | Rétention à chaud | Rétention au froid | Répliques requises | Classe de stockage | Étiquette de gouvernance |
|------------|--------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 heures | 14 jours | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 heures | 7 jours | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 heures | 180 jours | 3 | `cold` | `da.governance` |
| _Par défaut (toutes les autres classes)_ | 6 heures | 30 jours | 3 | `warm` | `da.default` |

یہ اقدار `torii.da_ingest.replication_policy` میں intégré ہیں اور تمام
Soumissions `/v1/da/ingest` par پر لاگو ہوتی ہیں۔ Profil de rétention forcé Torii
کے ساتھ manifeste des valeurs incompatibles entre les appelants et les appelants.
Un avertissement concernant les opérateurs SDK obsolètes est également disponible### Cours de disponibilité de Taikai

Manifestes de routage Taikai (`taikai.trm`) et `availability_class` (`hot`, `warm`,
یا `cold`) déclare کرتے ہیں۔ Torii chunking et politique de correspondance
Les opérateurs de table globale modifient le flux et le nombre de répliques
échelle کر سکیں۔ Valeurs par défaut :

| Classe de disponibilité | Rétention à chaud | Rétention au froid | Répliques requises | Classe de stockage | Étiquette de gouvernance |
|------------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 heures | 14 jours | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 heures | 30 jours | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 heure | 180 jours | 3 | `cold` | `da.taikai.archive` |

Indices manquants par défaut طور پر `hot` رکھتے ہیں تاکہ diffusions en direct سب سے مضبوط
politique conserver کریں۔ Le réseau مختلف cible استعمال کرتا ہے تو
`torii.da_ingest.replication_policy.taikai_availability` par défaut
remplacer کریں۔

##Configuration

La politique `torii.da_ingest.replication_policy` est définie par la loi *par défaut*
le modèle remplace les remplacements par classe et le tableau Identificateurs de classe
insensible à la casse et `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, et `custom:<u16>` (extensions approuvées par la gouvernance)
کرتے ہیں۔ Classes de stockage `hot`, `warm`, et `cold` pour les classes de stockage

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
```Valeurs par défaut pour le bloc de commande par défaut کسی classe کو serrer
Il s'agit d'une mise à jour de remplacement. Classes de base et niveau de base
`default_retention` modifier ici

Les classes de disponibilité Taikai peuvent remplacer les classes de disponibilité via
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

- Torii `RetentionPolicy` fourni par l'utilisateur pour un profil appliqué et pour remplacer le modèle Torii fourni par l'utilisateur
  chunking یا émission manifeste سے پہلے۔
- Les manifestes prédéfinis et le profil de rétention de non-concordance déclarent `400 schema mismatch`
  Je vais rejeter mon contrat avec un client périmé et je l'affaiblirai.
- Remplacer le journal des événements (`blob_class`, politique soumise par rapport à celle attendue)
  Déploiement en cours pour les appelants non conformes

Porte mise à jour pour [Plan d'ingestion de disponibilité des données] (ingest-plan.md)
(Liste de contrôle de validation) دیکھیں جو application de la conservation et couverture کرتا ہے۔

## Workflow de réplication (suivi DA-4)

Application de la rétention صرف پہلا قدم ہے۔ Opérateurs en direct
manifeste et les ordres de réplication ont configuré la politique définie par SoraFS
les blobs non conformes peuvent être répliqués à nouveau1. **Drift پر نظر رکھیں۔** Torii
   `overriding DA retention policy to match configured network baseline` émet
   Vérifiez les valeurs de rétention obsolètes de l'appelant اس log کو
   `torii_sorafs_replication_*` télémétrie et déficits de répliques
   redéploiements retardés
2. **Intention pour les répliques en direct et les différences** Un assistant d'audit est utilisé :

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Commande `torii.da_ingest.replication_policy` pour la configuration et la configuration
   manifest (JSON en Norito) décoder le fichier en utilisant `ReplicationOrderV1`
   les charges utiles et le résumé du manifeste correspondent à la correspondance Voici le drapeau des conditions:

   - `policy_mismatch` - Politique appliquée au profil de rétention du manifeste سے مختلف ہے
     (une mauvaise configuration Torii est en cours).
   - `replica_shortfall` - ordre de réplication en direct `RetentionPolicy.required_replicas`
     Il y a des répliques, des cibles et des affectations.Statut de sortie non nul فعال déficit کی نشاندہی کرتا ہے تاکہ CI/astreinte
   page d'automatisation فوراً کر سکے۔ JSON رپورٹ کو
   Paquet `docs/examples/da_manifest_review_template.md` à joindre
   تاکہ Le Parlement vote کے لئے دستیاب ہو۔
3. **Déclencheur de réplication**** En cas de déficit d'audit
   `ReplicationOrderV1` جاری کریں via les outils de gouvernance جو
   [Marché de capacité de stockage SoraFS](../sorafs/storage-capacity-marketplace.md)
   Il s'agit d'un audit d'un ensemble de répliques convergentes
   Remplacements d'urgence par la sortie CLI pour `iroha app da prove-availability`
   Les SRE et le résumé des preuves PDP se réfèrent à eux

Couverture de régression `integration_tests/tests/da/replication_policy.rs` میں ہے؛
suite `/v1/da/ingest` et politique de rétention incompatible et vérification des détails
L'intention manifeste de l'appelant a été récupérée et le profil appliqué est exposé.