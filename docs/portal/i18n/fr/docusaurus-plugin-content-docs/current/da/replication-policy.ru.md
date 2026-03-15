---
lang: fr
direction: ltr
source: docs/portal/docs/da/replication-policy.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Cette page correspond à `docs/source/da/replication_policy.md`. Consultez les versions suivantes
:::

# Disponibilité des données politiques (DA-4)

_Status : В работе — Владельцы : Core Protocol WG / Storage Team / SRE_

DA ingest pipeline теперь применяет детерминированные цели retention для каждого
La classe blob est décrite dans `roadmap.md` (flux de travail DA-4). Torii ouvert
сохранять enveloppes de rétention, предоставленные appelant, если они не совпадают с
Politique nationale, garantie, que le validateur/l'utilisateur assume la responsabilité
Il n'y a pas d'époque et de réplique sans ouverture de nom.

## La politique politique

| Blob de classe | Rétention à chaud | Rétention au froid | Réplique de réparation | Cours de classe | Étiquette de gouvernance |
|------------|--------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 heures | 14 jours | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 heures | 7 jours | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 heures | 180 jours | 3 | `cold` | `da.governance` |
| _Par défaut (dans toutes les classes)_ | 6 heures | 30 jours | 3 | `warm` | `da.default` |Cette page est installée sur `torii.da_ingest.replication_policy` et s'ouvre avec
всем `/v1/da/ingest` отправкам. Torii manifeste les manifestations avec le premier
la rétention des profils et la prévention, si les appelants ne sont pas disponibles
maintenant, les opérateurs peuvent utiliser le SDK.

### Classes de Taikai

Manifestes de routage Taikai (`taikai.trm`) par rapport à `availability_class`
(`hot`, `warm`, ou `cold`). Torii est la solution politique pour le chunking,
Les opérateurs peuvent réaliser des répliques de montres sur le flux sans suppression
tableaux mondiaux. Valeurs par défaut :

| Classe de distribution | Rétention à chaud | Rétention au froid | Réplique de réparation | Cours de classe | Étiquette de gouvernance |
|-------------------|---------------|----------------|------------------------|----------------|----------------|
| `hot` | 24 heures | 14 jours | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 heures | 30 jours | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 heure | 180 jours | 3 | `cold` | `da.taikai.archive` |

Si vous ne pouvez pas utiliser `hot`, la diffusion en direct est la même
profil строгий. Переопределяйте defaults ici
`torii.da_ingest.replication_policy.taikai_availability`, si vous utilisez
другие цели.

## ConfigurationLa politique est basée sur `torii.da_ingest.replication_policy` et pré-établie
*par défaut* шаблон плюс массив override для каждого класса. Classes d'identification
регистронезависимы и принимают `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, utilisez `custom:<u16>` pour la mise à jour appropriée.
Les classes incluent `hot`, `warm` ou `cold`.

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

Installez le bloc sans modification, pour que vous puissiez utiliser les valeurs par défaut. Que dois-je faire?
classe, обновите соответствующий dérogation ; чтобы изменить базу для новых классов,
отредактируйте `default_retention`.

Les cours de disponibilité de Taikai peuvent être programmés à tout moment
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

## Application de la signification

- Torii correspond au `RetentionPolicy` du profil principal
  avant le chunking ou le manifeste.
- Les manifestes préalables, которые декларируют несовпадающий profil de rétention,
  En cas de fermeture avec `400 schema mismatch`, les clients ne peuvent pas utiliser
  contrat.
- Каждое событие override LOGируется (`blob_class`, отправленная политика vs
  ожидаемая), comment identifier les appelants non conformes lors du déploiement actuel.

Смотрите [Plan d'ingestion de disponibilité des données](ingest-plan.md) (Liste de contrôle de validation) pour
обновленного porte, покрывающего application rétention.

## Workflow de réplication initiale (suivi DA-4)Rétention d'application - лишь первый шаг. Les opérateurs doivent également fournir des informations sur le live
les manifestes et les ordres de réplication sont liés à la politique actuelle,
Il s'agit de SoraFS qui permettent de re-répliquer automatiquement les blobs inutiles.

1. **Следите за drift.** Torii пишет
   `overriding DA retention policy to match configured network baseline` maintenant
   l'appelant отправляет устаревшие rétention значения. Сопоставляйте этот LOG с
   Télémètre `torii_sorafs_replication_*`, pour combler les déficits de réponse
   ou des redéploiements prévus.
2. **Utilisez l'intention et les répliques dynamiques.** Utilisez un nouvel assistant d'audit :

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   La commande installe `torii.da_ingest.replication_policy` avec la configuration,
   Décoder le manifeste (JSON ou Norito) et le prendre en charge de manière optionnelle
   charges utiles `ReplicationOrderV1` sur le manifeste digest. Voici quelques situations :

   - `policy_mismatch` - profil de rétention manifeste расходится с принудительным
     le profil (il n'est pas facile de le faire, car Torii est correct).
   - `replica_shortfall` - l'ordre de réplication en direct indique cette réplique, ici
     `RetentionPolicy.required_replicas`, ou vous êtes à la recherche de cette option.Un nouveau code vous permet de détecter un manque à gagner actif, pour l'automatisation des CI/de garde
   Je dois absolument le faire. Utiliser JSON pour obtenir un paquet
   `docs/examples/da_manifest_review_template.md` pour le Parlement européen.
3. **Procédez à la re-réplication.** Si vous auditez un manque à gagner, procédez
   nouveau `ReplicationOrderV1` pour la mise en œuvre des instruments, description du
   [Marché de capacité de stockage SoraFS](../sorafs/storage-capacity-marketplace.md),
   et vous pouvez vérifier, mais la réponse n'est pas disponible. Pour les dérogations d'urgence
   Essayez CLI pour `iroha app da prove-availability`, pour que SRE puisse le faire
   на тот же digest и PDP preuves.

Couverture de régression находится в `integration_tests/tests/da/replication_policy.rs` ;
suite отправляет несовпадающую politique rétention в `/v1/da/ingest` и проверяет,
Ce manifeste manifeste présente un profil privé, un appelant non intentionnel.