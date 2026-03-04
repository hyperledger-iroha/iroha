---
lang: fr
direction: ltr
source: docs/portal/docs/da/replication-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fonte canonica
Espelha `docs/source/da/replication_policy.md`. Mantenha comme duas versoes em
:::

# Politique de réplication de la disponibilité des données (DA-4)

_Statut : En cours -- Responsaveis : Core Protocol WG / Storage Team / SRE_

Le pipeline d'ingest DA agora application métas déterministes de rétention pour chaque
classe de blob décrite dans `roadmap.md` (flux de travail DA-4). Torii persister
enveloppes de retenue fournies par l'appelant qui ne correspond pas à la politique
configuré, garantindo que cada node validador/armazenamento retenha o numero
requis par l'époque et les répliques dépendent de l'intention de l'émetteur.

## Politique padrao

| Classe de blob | Retençao chaud | Retençao froid | Répliques requises | Classe d'armement | Étiquette de gouvernance |
|---------------|--------------|--------------|-------------------------|-------------------------|-------------------|
| `taikai_segment` | 24 heures | 14 jours | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 heures | 7 jours | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 heures | 180 jours | 3 | `cold` | `da.governance` |
| _Default (toujours comme classes demais)_ | 6 heures | 30 jours | 3 | `warm` | `da.default` |Valeurs sao embauchées dans `torii.da_ingest.replication_policy` et appliquées
a todas comme soumissions `/v1/da/ingest`. Torii réenregistrer les manifestes avec le profil
de retenir l'imposture et d'émettre un avertissement lorsque les appelants ont des valeurs divergentes
pour que les opérateurs détectent les SDK désaturés.

### Cours de disponibilité Taikai

Manifestes de roteamento Taikai (`taikai.trm`) déclaré `availability_class`
(`hot`, `warm`, ou `cold`). Torii appliquer au correspondant politique avant de le faire
chunking pour que les opérateurs puissent augmenter les contagènes des répliques par flux sem
éditer un tableau global. Valeurs par défaut :

| Classe de disponibilité | Retençao chaud | Retençao froid | Répliques requises | Classe d'armement | Étiquette de gouvernance |
|---------------------------|--------------|--------------|-----------------------------------|------------------------------|-------------------|
| `hot` | 24 heures | 14 jours | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 heures | 30 jours | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 heure | 180 jours | 3 | `cold` | `da.taikai.archive` |

Conseils ausentes usam `hot` por padrao para que transmissoes ao vivo retenham a
politique plus forte. Remplacer les valeurs par défaut du système via
`torii.da_ingest.replication_policy.taikai_availability` pour que vous puissiez l'utiliser
alvos diferentes.

## ConfigurationUne politique vive sanglante `torii.da_ingest.replication_policy` et exposant un modèle
*par défaut* plus d'un tableau de remplacement par classe. Identifiants de classe nao
différence entre les principales/minusculas et l'aceitam `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, ou `custom:<u16>` pour des extensions approuvées par la gouvernance.
Classes d'armement selon `hot`, `warm`, ou `cold`.

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

Il y a un bloc intact pour rouler avec les valeurs par défaut. Pour endurer un homme
classe, actualise ou remplace le correspondant ; para mudar a base de novas classes,
modifier `default_retention`.

Les cours de Taikai disponibles peuvent être suivis de forme indépendante
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

- Torii substitui o `RetentionPolicy` fornecido pelo usuario pelo perfil imposto
  avant de faire du chunking ou de l'émission de manifeste.
- Manifestes préconstruidos qui déclarent un profil de rétention divergente sao
  reçus avec `400 schema mismatch` pour les clients obsolètes nao possam
  enfraquecer o contrato.
- Chaque événement de dérogation et d'enregistrement (`blob_class`, politique enviée ou attendue)
  pour exporter les appelants non conformes lors du déploiement.

Veja [Plan d'ingestion de disponibilité des données] (ingest-plan.md) (Liste de contrôle de validation) para
le portail est actualisé en cobrindo application de retençao.

## Workflow de re-réplication (suite DA-4)L'application de la retenue et l'apenas o primeiro passo. Les opérateurs doivent également développer
prouver que les manifestations sont en direct et les ordonnances de réplication permanentes alinhados a politica
configuré pour que SoraFS puisse répliquer des blobs pour une conformité de forme
automatique.

1. **Observez la dérive.** Torii émet
   `overriding DA retention policy to match configured network baseline` quando
   un appelant soumet des valeurs de rétention désaturées. Combiner esse log com a
   télémétrie `torii_sorafs_replication_*` pour détecter les défauts de répliques ou
   redéploye les atrasados.
2. **Intention différente par rapport aux répliques en direct.** Utilisez le nouvel assistant d'auditoire :

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   La commande Carrega `torii.da_ingest.replication_policy` de configuration
   fourni, décodifié chaque manifeste (JSON ou Norito), et faz match facultativement
   les charges utiles `ReplicationOrderV1` pour le résumé du manifeste. Le résumé sinaliza duas
   conditions :

   - `policy_mismatch` - le profil de rétention de la divergence politique manifeste
     imposta (c'est-à-dire que cela ne devrait pas arriver à moins que Torii soit mal configuré).
   - `replica_shortfall` - une commande de répliques en direct sollicitant moins de répliques
     que `RetentionPolicy.required_replicas` ou fornece moins d'attributions à ce
     ou bien.Un statut de non-zéro indique un déficit actif pour l'automatisation de
   CI/on-call peut paginar immédiatement. Annexe sur le rapport JSON avec le paquet
   `docs/examples/da_manifest_review_template.md` pour les votes du Parlement.
3. **Dispare re-replicacao.** Quand un auditoire rapporte un déficit, il émet un
   novo `ReplicationOrderV1` via les ferramentas de gouvernance décrits dans
   [Marché de capacité de stockage SoraFS](../sorafs/storage-capacity-marketplace.md)
   Nous sommes montés dans un auditorium récemment et avons mangé un ensemble de répliques converties. Para remplacements
   d'urgence, mis à part ce que dit la CLI avec `iroha app da prove-availability` pour
   que SRE peut référencer le résumé et les preuves PDP.

La couverture de régression est vive dans `integration_tests/tests/da/replication_policy.rs` ;
suite à l'envoi d'une politique de retenue divergente pour `/v1/da/ingest` et vérification
que le manifeste buscado expose le profil imposto em vez da intencao do caller.