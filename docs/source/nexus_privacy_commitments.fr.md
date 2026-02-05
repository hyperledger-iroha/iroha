---
lang: fr
direction: ltr
source: docs/source/nexus_privacy_commitments.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7ef8ab7f52ec333d3fb9686dd744e23a859058dc0dbb91cfafee1cb7d1452ac
source_last_modified: "2025-11-21T17:27:28.236084+00:00"
translation_last_reviewed: 2026-01-01
---

# Cadre des commitments de confidentialite et des proofs (NX-10)

> **Statut:** Termine (NX-10)  
> **Responsables:** Cryptography WG / Privacy WG / Nexus Core WG  
> **Code lie:** [`crates/iroha_crypto/src/privacy.rs`](../../crates/iroha_crypto/src/privacy.rs)

NX-10 introduit une surface commune de commitments pour les lanes privees. Chaque dataspace publie
un descripteur deterministe qui lie ses racines Merkle ou circuits zk-SNARK a des hashes
reproductibles. L'anneau global Nexus peut donc valider les transferts cross-lane et les proofs de
confidentialite sans parsers sur mesure.

## Objectifs et perimetre

- Canoniser les identifiants de commitment pour que les manifests de gouvernance et les SDKs
  s'accordent sur le slot numerique utilise a l'admission.
- Livrer des helpers de verification reutilisables (`iroha_crypto::privacy`) pour que runtimes,
  Torii et auditeurs off-chain valident les proofs de facon coherente.
- Lier les proofs zk-SNARK au digest canonique de la verifying key et a des encodages deterministes
  des public inputs. Pas de transcripts ad-hoc.
- Documenter le workflow registry/export pour que les bundles de lane et les evidences de
  gouvernance incluent les memes hashes que le runtime impose.

Hors perimetre pour cette note: mecanismes de fan-out DA, relay messaging et plumbing du settlement
router. Voir `nexus_cross_lane.md` pour ces couches.

## Modele de commitment de lane

Le registry stocke une liste ordonnee d'entrees `LanePrivacyCommitment`:

```rust
use iroha_crypto::privacy::{
    CommitmentScheme, LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment, SnarkCircuit,
    SnarkCircuitId,
};

let id = LaneCommitmentId::new(1);
let merkle = LanePrivacyCommitment::merkle(
    id,
    MerkleCommitment::from_root_bytes(root_bytes, 16),
);

let snark = LanePrivacyCommitment::snark(
    LaneCommitmentId::new(2),
    SnarkCircuit::new(
        SnarkCircuitId::new(42),
        verifying_key_digest,
        statement_hash,
        proof_hash,
    ),
);
```

- **`LaneCommitmentId`** - identifiant stable 16 bits enregistre dans les manifests de lane.
- **`CommitmentScheme`** - `Merkle` ou `Snark`. Des variantes futures (par ex., bulletproofs)
  etendent l'enum.
- **Semantique de copie** - tous les descripteurs implementent `Copy` afin que les configs les
  reutilisent sans churn de heap.

Le registry voyage avec le bundle de lane Nexus (`scripts/nexus/lane_registry_bundle.py`). Quand la
gouvernance approuve un nouveau commitment, le bundle met a jour le manifest JSON et l'overlay
Norito consomme par l'admission.

### Schema de manifest (`privacy_commitments`)

Les manifests de lane exposent maintenant un tableau `privacy_commitments`. Chaque entree assigne
un ID et un scheme et inclut les parametres propres au scheme:

```json
{
  "lane": "cbdc",
  "governance": "council",
  "privacy_commitments": [
    {
      "id": 1,
      "scheme": "merkle",
      "merkle": {
        "root": "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "max_depth": 16
      }
    },
    {
      "id": 2,
      "scheme": "snark",
      "snark": {
        "circuit_id": 5,
        "verifying_key_digest": "0x...",
        "statement_hash": "0x...",
        "proof_hash": "0x..."
      }
    }
  ]
}
```

Le registry bundler copie le manifest, enregistre les tuples `(id, scheme)` dans `summary.json`, et
CI (`lane_registry_verify.py`) re-parse le manifest pour assurer que le summary correspond au
contenu disque.

## Commitments Merkle

`MerkleCommitment` capture un `HashOf<MerkleTree<[u8;32]>>` canonique et une profondeur maximale de
chemin d'audit. Les operateurs exportent la racine directement depuis leur prover ou snapshot du
ledger; pas de re-hashing dans le registry.

Flux de verification:

```rust
use iroha_crypto::privacy::{LanePrivacyCommitment, MerkleWitness, PrivacyWitness};

let witness = MerkleWitness::from_leaf_bytes(leaf_bytes, proof);
LanePrivacyCommitment::merkle(id, commitment)
    .verify(PrivacyWitness::Merkle(witness))?;
```

- Les proofs depassant `max_depth` emettent `PrivacyError::MerkleProofExceedsDepth`.
- Les audit paths reutilisent les utilitaires `iroha_crypto::MerkleProof` pour que les pools
  shielded et les lanes privees Nexus partagent la meme serialization.
- Les hosts qui ingerent des pools externes convertissent les feuilles shielded via
  `MerkleTree::shielded_leaf_from_commitment` avant de construire le witness.

### Checklist operationnel

- Publier le tuple `(id, root, depth)` dans le summary du manifest de lane et le bundle d'evidence.
- Attacher la preuve d'inclusion Merkle pour chaque transfert cross-lane au log d'admission; le
  helper reproduit la racine byte-for-byte, donc les audits comparent uniquement le hash exporte.
- Suivre les budgets de profondeur en telemetrie (`nexus_privacy_commitments.merkle.depth_used`) pour
  que les alertes se declenchent avant que les rollouts depassent les maxima configures.

## Commitments zk-SNARK

Les entrees `SnarkCircuit` lient quatre champs:

| Champ | Description |
|-------|-------------|
| `circuit_id` | Identifiant controle par gouvernance pour la paire circuit/version. |
| `verifying_key_digest` | Hash Blake3 de la verifying key canonique (DER ou encodage Norito). |
| `statement_hash` | Hash Blake3 de l'encodage canonique des public inputs (Norito ou Borsh). |
| `proof_hash` | Hash Blake3 de `verifying_key_digest || proof_bytes`. |

Helpers runtime:

```rust
use iroha_crypto::privacy::{
    hash_proof, hash_public_inputs, LanePrivacyCommitment, PrivacyWitness, SnarkWitness,
};

let witness = SnarkWitness {
    public_inputs: encoded_inputs,
    proof: proof_bytes,
};

LanePrivacyCommitment::snark(id, circuit)
    .verify(PrivacyWitness::Snark(witness))?;
```

- `hash_public_inputs` et `hash_proof` vivent dans le meme module; les SDKs doivent les appeler
  lors de la generation des manifests ou proofs pour eviter le format drift.
- Toute divergence entre le statement hash enregistre et les public inputs presentes genere
  `PrivacyError::SnarkStatementMismatch`.
- Les bytes de preuve doivent deja etre dans leur forme compressee canonique (Groth16, Plonk, etc.).
  Le helper ne verifie que le binding de hash; la verification complete de courbe reste dans le
  service prover/verifier.

### Workflow d'evidence

1. Exporter le verifying key digest et le proof hash dans le manifest du bundle de lane.
2. Attacher l'artefact de preuve brut au package d'evidence de gouvernance pour que les auditeurs
   puissent recalculer `hash_proof`.
3. Publier l'encodage canonique des public inputs (hex ou Norito) a cote du statement hash pour des
   replays deterministes.

## Pipeline d'ingestion des proofs

1. **Manifests de lane** declarent quel `LaneCommitmentId` gouverne chaque contrat ou bucket de
   programmable-money.
2. **Admission** consomme les entries `ProofAttachment.lane_privacy`, verifie les witnesses attaches
   pour la lane routee via `LanePrivacyRegistry::verify`, et transmet les commitment ids valides a
   la lane compliance (`privacy_commitments_any_of`) afin que les regles allow/deny de
   programmable-money imposent la presence de proofs avant l'enqueue.
3. **Telemetrie** incremente les compteurs `nexus_privacy_commitments.{merkle,snark}` avec `LaneId`,
   `commitment_id` et outcome (`ok`, `depth_mismatch`, etc.).
4. **Evidence de gouvernance** reutilise le meme helper pour regenerer les rapports d'acceptance
   (`ci/check_nexus_lane_registry_bundle.sh` se branche sur la verification du bundle quand les
   metadonnees SNARK arrivent).

### Visibilite operateur

L'endpoint `/v1/sumeragi/status` de Torii expose maintenant l'array
`lane_governance[].privacy_commitments` pour que les operateurs et SDKs puissent comparer le
registry live aux manifests publies sans relire le bundle. Le snapshot est construit dans
`crates/iroha_core/src/sumeragi/status.rs`, exporte par les handlers REST/JSON de Torii
(`crates/iroha_torii/src/routing.rs`), et decode par chaque client
(`javascript/iroha_js/src/toriiClient.js`,
`python/iroha_python/src/iroha_python/client.py`, `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`),
refletant le schema du manifest pour les racines Merkle et les digests SNARK.

Les lanes commitment-only ou split-replica echouent maintenant l'admission si leur manifest omet la
section `privacy_commitments`, garantissant que les flux programmable-money ne peuvent demarrer
qu'une fois les anchors deterministes de proofs incluses dans le bundle.

## Registry runtime et admission

- `LanePrivacyRegistry` (`crates/iroha_core/src/interlane/mod.rs`) snapshot les structures
  `LaneManifestStatus` du loader de manifest et stocke des maps de commitment par lane, clees par
  `LaneCommitmentId`. La transaction queue et `State` installent ce registry a chaque reload de
  manifest (`Queue::install_lane_manifests`, `State::install_lane_manifests`), de sorte que
  l'admission et la validation de consensus aient toujours acces aux commitments canoniques.
- `LaneComplianceContext` porte maintenant une reference optionnelle `Arc<LanePrivacyRegistry>`
  (`crates/iroha_core/src/compliance/mod.rs`). Quand `LaneComplianceEngine` evalue une transaction,
  les flux programmable-money peuvent inspecter les memes commitments par lane exposes via Torii
  avant l'enqueue du payload.
- Admission et la validation core maintiennent un `Arc<LanePrivacyRegistry>` a cote du handle de
  manifest de gouvernance (`crates/iroha_core/src/queue.rs`, `crates/iroha_core/src/state.rs`),
  garantissant que les modules programmable-money et les futurs hosts interlane lisent une vue
  coherente des descripteurs de confidentialite meme quand les manifests tournent.

## Enforcement runtime

Les proofs de confidentialite de lane transitent maintenant via `ProofAttachment.lane_privacy`.
L'admission Torii et la validation `iroha_core` verifient chaque attachment contre le registry de
la lane routee avec `LanePrivacyRegistry::verify`, enregistrent les commitment ids prouves, puis
les transmettent au moteur compliance. Toute regle `privacy_commitments_any_of` evalue a `false`
tant qu'un attachment correspondant ne verifie pas avec succes, ainsi les lanes programmable-money
ne peuvent etre enqueue ni commit sans witness requis. La couverture unitaire vit dans
`interlane::tests` et les tests de
lane compliance pour stabiliser le chemin des attachments et les guardrails de policy.

Pour experimentation immediate, voir les tests unitaires dans
[`privacy.rs`](../../crates/iroha_crypto/src/privacy.rs) qui montrent des cas de succes et d'echec
pour les commitments Merkle et zk-SNARK.
