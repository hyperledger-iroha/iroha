---
lang: pt
direction: ltr
source: docs/source/nexus_privacy_commitments.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7ef8ab7f52ec333d3fb9686dd744e23a859058dc0dbb91cfafee1cb7d1452ac
source_last_modified: "2025-11-21T17:27:28.236084+00:00"
translation_last_reviewed: 2026-01-01
---

# Framework de compromissos de privacidade e provas (NX-10)

> **Status:** Concluido (NX-10)  
> **Responsaveis:** Cryptography WG / Privacy WG / Nexus Core WG  
> **Codigo relacionado:** [`crates/iroha_crypto/src/privacy.rs`](../../crates/iroha_crypto/src/privacy.rs)

NX-10 introduz uma superficie comum de compromissos para lanes privadas. Cada dataspace publica
um descritor deterministico que liga suas raizes Merkle ou circuitos zk-SNARK a hashes
reproduziveis. O anel global do Nexus pode entao validar transferencias cross-lane e provas de
confidencialidade sem parsers sob medida.

## Objetivos e escopo

- Canonizar identificadores de compromisso para que manifests de governanca e SDKs concordem com o
  slot numerico usado durante admission.
- Entregar helpers de verificacao reutilizaveis (`iroha_crypto::privacy`) para que runtimes, Torii
  e auditores off-chain validem provas de forma consistente.
- Vincular provas zk-SNARK ao digest canonico da verifying key e a codificacoes deterministicas de
  public inputs. Sem transcripts ad-hoc.
- Documentar o workflow de registry/export para que bundles de lane e evidencia de governanca
  incluam os mesmos hashes que o runtime aplica.

Fora de escopo para esta nota: mecanicas de fan-out DA, relay messaging e plumbing do settlement
router. Veja `nexus_cross_lane.md` para essas camadas.

## Modelo de compromisso de lane

O registry armazena uma lista ordenada de entries `LanePrivacyCommitment`:

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

- **`LaneCommitmentId`** - identificador estavel de 16 bits registrado em manifests de lane.
- **`CommitmentScheme`** - `Merkle` ou `Snark`. Variantes futuras (ex., bulletproofs) estendem o enum.
- **Semantica de copia** - todos os descritores implementam `Copy` para que configs reutilizem sem
  churn de heap.

O registry acompanha o bundle de lanes do Nexus (`scripts/nexus/lane_registry_bundle.py`). Quando a
governanca aprova um novo compromisso, o bundle atualiza tanto o manifest JSON quanto o overlay
Norito consumido pela admission.

### Schema de manifest (`privacy_commitments`)

Manifests de lanes agora expoem um array `privacy_commitments`. Cada entry atribui um ID e scheme
incluindo parametros especificos do scheme:

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

O bundler do registry copia o manifest, registra os tuples `(id, scheme)` em `summary.json`, e CI
(`lane_registry_verify.py`) reparseia o manifest para garantir que o summary corresponda ao
conteudo em disco.

## Compromissos Merkle

`MerkleCommitment` captura um `HashOf<MerkleTree<[u8;32]>>` canonico e uma profundidade maxima de
caminho de auditoria. Operadores exportam a raiz diretamente do prover ou snapshot do ledger; nao
ha re-hashing dentro do registry.

Fluxo de verificacao:

```rust
use iroha_crypto::privacy::{LanePrivacyCommitment, MerkleWitness, PrivacyWitness};

let witness = MerkleWitness::from_leaf_bytes(leaf_bytes, proof);
LanePrivacyCommitment::merkle(id, commitment)
    .verify(PrivacyWitness::Merkle(witness))?;
```

- Provas que excedem `max_depth` emitem `PrivacyError::MerkleProofExceedsDepth`.
- Audit paths reutilizam utilitarios `iroha_crypto::MerkleProof` para que pools shielded e lanes
  privadas Nexus compartilhem a mesma serializacao.
- Hosts que ingerem pools externos convertem folhas shielded via
  `MerkleTree::shielded_leaf_from_commitment` antes de construir o witness.

### Checklist operacional

- Publicar o tuple `(id, root, depth)` no summary do manifest de lane e no bundle de evidencia.
- Anexar a prova de inclusao Merkle para cada transferencia cross-lane ao log de admission; o helper
  reproduz a raiz byte-for-byte, entao auditorias apenas comparam o hash exportado.
- Acompanhar budgets de profundidade em telemetria (`nexus_privacy_commitments.merkle.depth_used`)
  para que alertas disparem antes de rollouts excederem os maximos configurados.

## Compromissos zk-SNARK

Entries `SnarkCircuit` vinculam quatro campos:

| Campo | Descricao |
|-------|-------------|
| `circuit_id` | Identificador controlado pela governanca para o par circuito/versao. |
| `verifying_key_digest` | Hash Blake3 da verifying key canonica (DER ou codificacao Norito). |
| `statement_hash` | Hash Blake3 do encoding canonico de public inputs (Norito ou Borsh). |
| `proof_hash` | Hash Blake3 de `verifying_key_digest || proof_bytes`. |

Helpers de runtime:

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

- `hash_public_inputs` e `hash_proof` vivem no mesmo modulo; SDKs devem chama-los ao gerar manifests
  ou provas para evitar format drift.
- Qualquer mismatch entre o statement hash registrado e os public inputs apresentados gera
  `PrivacyError::SnarkStatementMismatch`.
- Bytes de prova devem estar na forma comprimida canonica (Groth16, Plonk, etc.). O helper apenas
  verifica o binding de hash; a verificacao completa de curva fica no servico prover/verifier.

### Workflow de evidencia

1. Exportar o verifying key digest e o proof hash no manifest do bundle de lane.
2. Anexar o artefato de prova raw ao pacote de evidencia de governanca para que auditores possam
   recomputar `hash_proof`.
3. Publicar o encoding canonico de public inputs (hex ou Norito) junto com o statement hash para
   replays deterministas.

## Pipeline de ingestao de provas

1. **Manifests de lanes** declaram qual `LaneCommitmentId` governa cada contrato ou bucket de
   programmable-money.
2. **Admission** consome entries `ProofAttachment.lane_privacy`, verifica os witnesses anexados
   para a lane roteada via `LanePrivacyRegistry::verify`, e alimenta os commitment ids validados em
   lane compliance (`privacy_commitments_any_of`) para que regras allow/deny de programmable-money
   imponham presenca de provas antes de enfileirar.
3. **Telemetria** incrementa contadores `nexus_privacy_commitments.{merkle,snark}` com `LaneId`,
   `commitment_id` e outcome (`ok`, `depth_mismatch`, etc.).
4. **Evidencia de governanca** usa o mesmo helper para regenerar relatorios de acceptance
   (`ci/check_nexus_lane_registry_bundle.sh` conecta a verificacao do bundle quando a metadata
   SNARK chega).

### Visibilidade para operadores

O endpoint `/v2/sumeragi/status` do Torii agora expoe o array
`lane_governance[].privacy_commitments` para que operadores e SDKs comparem o registry live com os
manifests publicados sem reler o bundle. O snapshot e construido em
`crates/iroha_core/src/sumeragi/status.rs`, exportado pelos handlers REST/JSON do Torii
(`crates/iroha_torii/src/routing.rs`), e decodificado por cada cliente
(`javascript/iroha_js/src/toriiClient.js`,
`python/iroha_python/src/iroha_python/client.py`, `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`),
espelhando o schema do manifest tanto para raizes Merkle quanto para digests SNARK.

Lanes commitment-only ou split-replica agora falham admission se o manifest omitir a secao
`privacy_commitments`, garantindo que fluxos programmable-money nao possam iniciar ate que
ancoras deterministas de provas sejam entregues com o bundle.

## Registry de runtime e admission

- `LanePrivacyRegistry` (`crates/iroha_core/src/interlane/mod.rs`) faz snapshot das estruturas
  `LaneManifestStatus` do manifest loader e armazena mapas de compromisso por lane com chave
  `LaneCommitmentId`. A transaction queue e `State` instalam este registry junto com cada
  recarregamento de manifest (`Queue::install_lane_manifests`, `State::install_lane_manifests`),
  assim admission e a validacao de consenso sempre tem acesso aos compromissos canonicos.
- `LaneComplianceContext` agora carrega uma referencia opcional `Arc<LanePrivacyRegistry>`
  (`crates/iroha_core/src/compliance/mod.rs`). Quando `LaneComplianceEngine` avalia uma transacao,
  fluxos programmable-money podem inspecionar os mesmos compromissos por lane expostos via Torii
  antes de enfileirar o payload.
- Admission e a validacao core mantem um `Arc<LanePrivacyRegistry>` ao lado do handle de manifest
  de governanca (`crates/iroha_core/src/queue.rs`, `crates/iroha_core/src/state.rs`), garantindo que
  modulos programmable-money e futuros hosts interlane leiam uma visao consistente dos descritores
  de privacidade mesmo com rotacao de manifests.

## Enforcement em runtime

Provas de privacidade de lane agora viajam junto a attachments de transacao via
`ProofAttachment.lane_privacy`. Admission do Torii e a validacao de `iroha_core` verificam cada
attachment contra o registry da lane roteada usando `LanePrivacyRegistry::verify`, registram os
commitment ids provados e os alimentam no engine de compliance. Qualquer regra que especifica
`privacy_commitments_any_of` agora avalia `false` a menos que um attachment correspondente verifique
com sucesso, entao lanes programmable-money nao podem ser enfileiradas nem confirmadas sem o
witness exigido. Cobertura unitaria vive em `interlane::tests` e nos lane compliance tests para
manter o caminho de attachments e os guardrails de policy estaveis.

Para experimentacao imediata, veja os testes unitarios em
[`privacy.rs`](../../crates/iroha_crypto/src/privacy.rs) que demonstram casos de sucesso e falha
para compromissos Merkle e zk-SNARK.
