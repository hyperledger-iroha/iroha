---
lang: fr
direction: ltr
source: docs/portal/docs/da/ingest-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fonte canonica
Espelha `docs/source/da/ingest_plan.md`. Mantenha comme duas versoes em
:::

# Plan d'acquisition des données de disponibilité par Sora Nexus

_Redigido : 2026-02-20 - Responsable : Core Protocol WG / Storage Team / DA WG_

Le flux de travail DA-2 comprend Torii avec une API d'acquisition de blobs qui émettent
métadonnées Norito et semées à la réplique SoraFS. Ce document capture ou esquema
propose, une surface d'API et le flux de validation pour la mise en œuvre
avancez sem bloquear em simulacoes pendentes (seguimentos DA-1). Tous les os
les formats de charge utile DEVEM utilisent les codecs Norito ; nao sao permitidos solutions de secours
serde/JSON.

## Objets

- Aceitar blobs grandes (segments Taikai, side-cars de lane, artefatos de
  gouvernance) de forma deterministica via Torii.
- Produire les manifestes Norito canoniques qui décrivent le blob, les paramètres du codec,
  profil d’effacement et politique de rétention.
- Conserver les métadonnées des morceaux sans stockage chaud de SoraFS et enregistrer les tâches de
  réplique.
- Publier les intentions de broche + les balises politiques dans le registre SoraFS et les observateurs
  de gouvernance.
- Exporter les reçus d'admission pour que les clients récupèrent une preuve déterministe
  de publicacao.

## API Superficie (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```La charge utile est `DaIngestRequest` codifiée en Norito. En tant que réponse usam
`application/norito+v1` et retour `DaIngestReceipt`.

| Réponse | Signifié |
| --- | --- |
| 202 Accepté | Blob enfileirado pour le chunking/replicacao ; reçu retourné. |
| 400 requêtes incorrectes | Violacao de esquema/tamanho (veja validacoes). |
| 401 Non autorisé | API de jeton autorisée/invalide. |
| 409 Conflit | Copie de `client_blob_id` avec des métadonnées divergentes. |
| 413 Charge utile trop importante | Dépasser la limite configurée de tamanho do blob. |
| 429 Trop de demandes | Limite de taux atingido. |
| 500 Erreur interne | Falha inesperada (log + alerta). |

## Esquema Norito propose

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Nexus lane
    pub epoch: u64,                      // epoch blob belongs to
    pub sequence: u64,                   // monotonic sequence per (lane, epoch)
    pub blob_class: BlobClass,           // TaikaiSegment, GovernanceArtifact, etc.
    pub codec: BlobCodec,                // e.g. "cmaf", "pdf", "norito-batch"
    pub erasure_profile: ErasureProfile, // parity configuration
    pub retention_policy: RetentionPolicy,
    pub chunk_size: u32,                 // bytes (must align with profile)
    pub total_size: u64,
    pub compression: Compression,        // Identity, gzip, deflate, or zstd
    pub norito_manifest: Option<Vec<u8>>, // optional pre-built manifest
    pub payload: Vec<u8>,                 // raw blob data (<= configured limit)
    pub metadata: ExtraMetadata,          // optional key/value metadata map
    pub submitter: PublicKey,             // signing key of caller
    pub signature: Signature,             // canonical signature over request
}

pub enum BlobClass {
    TaikaiSegment,
    NexusLaneSidecar,
    GovernanceArtifact,
    Custom(u16),
}

pub struct ErasureProfile {
    pub data_shards: u16,
    pub parity_shards: u16,
    pub chunk_alignment: u16, // chunks per availability slice
    pub fec_scheme: FecScheme,
}

pub struct RetentionPolicy {
    pub hot_retention_secs: u64,
    pub cold_retention_secs: u64,
    pub required_replicas: u16,
    pub storage_class: StorageClass,
    pub governance_tag: GovernanceTag,
}

pub struct ExtraMetadata {
    pub items: Vec<MetadataEntry>,
}

pub struct MetadataEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub visibility: MetadataVisibility, // public vs governance-only
}

pub enum MetadataVisibility {
    Public,
    GovernanceOnly,
}

pub struct DaIngestReceipt {
    pub client_blob_id: BlobDigest,
    pub lane_id: LaneId,
    pub epoch: u64,
    pub blob_hash: BlobDigest,          // BLAKE3 of raw payload
    pub chunk_root: BlobDigest,         // Merkle root after chunking
    pub manifest_hash: BlobDigest,      // Norito manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Norito-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```

> Note de mise en œuvre : en tant que représentants canoniques dans Rust pour ces charges utiles
> Je viens de vivre avec `iroha_data_model::da::types`, avec les wrappers de demande/réception
> em `iroha_data_model::da::ingest` et structure du manifeste em
> `iroha_data_model::da::manifest`.

Le champ `compression` informe les appelants de la préparation de la charge utile. Torii huile
`identity`, `gzip`, `deflate`, et `zstd`, décompression des octets avant le hachage,
chunking et vérification des manifestes opcionais.

### Checklist de validation1. Vérifiez que l'en-tête Norito correspondant à la demande correspond à `DaIngestRequest`.
2. Falhar est `total_size` différent du tamanho canonique de la charge utile (décomprimée)
   ou dépasser le maximum configuré.
3. Forcar alinhamento de `chunk_size` (puissance de deux, = 2.
5. `retention_policy.required_replica_count` doit respecter une ligne de base
   gouvernance.
6. Vérification de l'assistanat contre le hachage canonique (à l'exclusion de la signature du champ).
7. Rejeter la duplication `client_blob_id`, à l'exception du hachage de la charge utile et des métadonnées
   Forem Identicos.
8. Quand `norito_manifest` pour le fornecido, vérifier le schéma + le hachage coïncident avec
   o manifeste recalculé après o chunking ; caso contrario ou noeud gera ou manifeste
   e o armazena.
9. Rechercher une politique de réplication configurée : Torii réenregistrer le fichier
   `RetentionPolicy` envoyé avec `torii.da_ingest.replication_policy` (voir
   `replication-policy.md`) et le rejet manifeste des métadonnées préconstruites avec
   retençao nao coïncider avec le profil imposto.

### Flux de chunking et de réplication1. Lancez la charge utile dans `chunk_size`, calculez BLAKE3 par chunk + Raiz Merkle.
2. Construire Norito `DaManifestV1` (struct nova) pour capturer les compromis de
   chunk (role/group_id), layout d'effacement (contaminateurs de parité de lignes et
   colonnes plus `ipa_commitment`), politique de rétention et métadonnées.
3. Enfileirar os bytes do manifest canonico sob
   `config.da_ingest.manifest_store_dir` (Torii enregistrer les archives
   `manifest.encoded` por lane/epoch/sequence/ticket/fingerprint) pour que
   orquestração SoraFS os ingira e vincule o storage ticket aos dados
   persistants.
4. Publier les intentions de broche via `sorafs_car::PinIntent` avec la balise de gouvernance et
   politique.
5. Émettre l'événement Norito `DaIngestPublished` pour notifier les observateurs
   (clients levés, gouvernance, analyse).
6. Retornar `DaIngestReceipt` à l'appelant (assisté par le client du service DA de
   Torii) et émettre l'en-tête `Sora-PDP-Commitment` pour que les SDK capturent
   engagement codificado de imediato. O reçu agora incluant `rent_quote`
   (um Norito `DaRentQuote`) et `stripe_layout`, permettant que vous remettiez
   Exibam à la base de rendu, à la réserve, aux attentes de bonus PDP/PoTR et à la mise en page de
   effacement 2D à côté du ticket de stockage avant le fonds du compteur.

## Actualisations de stockage/registre- Estender `sorafs_manifest` avec `DaManifestV1`, analyse syntaxique autorisée
  déterministe.
- Ajouter un nouveau flux de registre `da.pin_intent` avec la version de charge utile
  référence au hash du manifeste + identifiant du ticket.
- Atualizar pipelines de observabilidade para rastrear latencia de ingestao,
  débit de chunking, arriéré de réplication et contagieux de faux.

## Stratégie testiculaire

- Tests unitarios para validacao de schema, checks de assinatura, deteccao de
  duplicados.
- Tests Golden Verificando encodage Norito de `DaIngestRequest`, manifeste e
  reçu.
- Harnais d'intégration subindo mock SoraFS + registre, validation des flux de
  morceau + épingle.
- Tests de propriétés cobrindo perfis d'effacement et combinaisons de rétention
  aléatoires.
- Fuzzing des charges utiles Norito pour protéger les métadonnées mal formées.

## Outillage de CLI et SDK (DA-8)- `iroha app da submit` (nouvelle CLI du point d'entrée) vient d'impliquer le constructeur/éditeur de
  ingestao partagé pour que les opérateurs puissent ingérer des blobs arbitrarios
  forums de flux de Taikai bundle. O commando, vive-les
  `crates/iroha_cli/src/commands/da.rs:1` et utiliser une charge utile, profil de
  Erasure/Retencao et Arquivos Opcionais de Metadata/Manifest avant l'assassinat
  `DaIngestRequest` canonique avec la configuration de la CLI. Execucoes bem-sucedidas
  persistem `da_request.{norito,json}` et `da_receipt.{norito,json}` sanglot
  `artifacts/da/submission_<timestamp>/` (remplacement via `--artifact-dir`) pour que
  les artefatos de release enregistrent les octets Norito exatos usados durante a
  ingérer.
- O commando usa por padrao `client_blob_id = blake3(payload)` mas aceita
  remplacements via `--client-blob-id`, veuillez mapper les métadonnées JSON (`--metadata-json`)
  e manifeste pré-gerados (`--manifest`), et supporta `--no-submit` para preparacao
  hors ligne mais `--endpoint` pour les hôtes Torii personnalisés. O reçu JSON e
  impression no stdout alem de être écrit no disco, fechando o requisito de
  l'outillage "submit_blob" du DA-8 et fournit le travail de parité du SDK.
- `iroha app da get` ajouter un alias spécifié dans DA pour l'explorateur multi-source
  que ja alimenta `iroha app sorafs fetch`. Les opérateurs peuvent proposer des œuvres d'art
  de manifest + chunk-plan (`--manifest`, `--plan`, `--manifest-id`) **ou**
  passer un ticket de stockage Torii via `--storage-ticket`. Quand le chemin est-il faitticket et utilisé, une CLI baixa ou manifeste de `/v2/da/manifests/<ticket>`,
  persiste o bundle sob `artifacts/da/fetch_<timestamp>/` (remplacer com
  `--manifest-cache-dir`), dériver le hachage du blob pour `--manifest-id`, et ensuite
  exécuter l'orquestrador avec la liste `--gateway-provider` fornecida. Tous les os
  boutons avancés pour récupérer SoraFS permanecem intactos (enveloppes du manifeste,
  étiquettes de client, caches de garde, remplacements de transport anonyme, exportation de
  tableau de bord et chemins `--output`), et le point final du manifeste peut être surscrit
  via `--manifest-endpoint` pour les hôtes Torii personnalisés, entre les contrôles du système d'exploitation
  disponibilité de bout en bout vivem totalement sur l'espace de noms `da` sans duplication
  logique de l'orchestre.
- `iroha app da get-blob` baixa manifeste canonicos directement de Torii via
  `GET /v2/da/manifests/{storage_ticket}`. O comando escreve
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` et
  `chunk_plan_{ticket}.json` sanglote `artifacts/da/fetch_<timestamp>/` (ou euh
  `--output-dir` fourni à l'utilisateur) pendant l'impression de la commande exato de
  `iroha app da get` (incluant `--manifest-id`) nécessaire pour la récupération
  orchestre. Isso mantem operadores fora dos diretorios spool de manifests e
  garantie que le récupérateur utilise toujours les artefatos assinados émis par Torii. Ô
  client Torii JavaScript exécute mon flux via
  `ToriiClient.getDaManifest(storageTicketHex)`, retour des octets Norito
  décodifiés, manifeste JSON et plan de blocs pour que les appelants du SDK hidratemles séances d'orchestre sans utiliser la CLI. Le SDK Swift s'expose désormais en tant que mesmas
  superficies (`ToriiClient.getDaManifestBundle(...)` plus
  `fetchDaPayloadViaGateway(...)`), canalizando bundles para o wrapper natif do
  orchestre SoraFS pour que les clients iOS puissent se manifester, exécuter
  récupère plusieurs sources et les capture sans appeler une CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` calcule les rentes déterministes et le détail de l'habitation
  incitations pour un tamanho de stockage et une banque de stockage. Ô aide
  utiliser un `DaRentPolicyV1` ativa (JSON ou octets Norito) ou l'embutido par défaut,
  valider la politique et imprimer un résumé JSON (`gib`, `months`, métadonnées de
  politica e campos de `DaRentQuote`) pour que les auditeurs citent les charges XOR exatas
  em atas de gouvernance sem scripts ad hoc. Le commandant émet également un résumé
  Utilisez la ligne `rent_quote ...` avant la charge utile JSON pour gérer les journaux de la console
  legiveis durante exercices d'incident. Emparelhe
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` avec
  `--policy-label "governance ticket #..."` pour conserver les bons objets que
  citez le vote ou le bundle de configuration exatos ; une ligne CLI ou une étiquette personnalisée
  recusa strings vazias para que valores `policy_source` permanecam acionaveis
  nos tableaux de bord de tesouraria. Véja
  `crates/iroha_cli/src/commands/da.rs` pour la sous-commande et`docs/source/da/rent_policy.md` pour le schéma politique.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadeia tudo acima : recevoir un ticket de stockage,
  j'utilise le bundle canonique de manifeste, exécute l'orchestre multi-source
  (`iroha app sorafs fetch`) contre la liste `--gateway-provider` fornecida, persiste
  o charge utile baixado + tableau de bord sob `artifacts/da/prove_availability_<timestamp>/`,
  et invoquez immédiatement l'assistant PoR existant (`iroha app da prove`) en utilisant l'utilisateur
  octets réduits. Les opérateurs peuvent ajuster les boutons de l'orchestre
  (`--max-peers`, `--scoreboard-out`, remplace le point final du manifeste) et o
  sampler de proof (`--sample-count`, `--leaf-index`, `--sample-seed`) quant à un
  une seule commande produit les artefatos attendus par les auditoriums DA-5/DA-9 : copie du
  charge utile, preuves du tableau de bord et résumés de la preuve JSON.