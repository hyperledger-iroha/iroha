---
lang: es
direction: ltr
source: docs/source/nexus_privacy_commitments.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7ef8ab7f52ec333d3fb9686dd744e23a859058dc0dbb91cfafee1cb7d1452ac
source_last_modified: "2025-11-21T17:27:28.236084+00:00"
translation_last_reviewed: 2026-01-01
---

# Marco de compromisos de privacidad y pruebas (NX-10)

> **Estado:** Completado (NX-10)  
> **Responsables:** Cryptography WG / Privacy WG / Nexus Core WG  
> **Codigo relacionado:** [`crates/iroha_crypto/src/privacy.rs`](../../crates/iroha_crypto/src/privacy.rs)

NX-10 introduce una superficie comun de compromisos para lanes privadas. Cada dataspace publica un
descriptor determinista que vincula sus raices Merkle o circuitos zk-SNARK a hashes reproducibles.
El anillo global de Nexus puede entonces validar transferencias cross-lane y pruebas de
confidencialidad sin parsers a medida.

## Objetivos y alcance

- Canonizar identificadores de compromiso para que manifests de gobernanza y SDKs acuerden el slot
  numerico usado durante admission.
- Entregar helpers de verificacion reutilizables (`iroha_crypto::privacy`) para que runtimes, Torii
  y auditores off-chain validen pruebas de forma consistente.
- Vincular pruebas zk-SNARK al digest canonico de la verifying key y a codificaciones deterministas
  de public inputs. Sin transcripts ad-hoc.
- Documentar el flujo de registry/export para que bundles de lane y evidencia de gobernanza
  incluyan los mismos hashes que aplica el runtime.

Fuera de alcance para esta nota: mecanicas de fan-out DA, relay messaging y plumbing del settlement
router. Ver `nexus_cross_lane.md` para esas capas.

## Modelo de compromiso de lane

El registry almacena una lista ordenada de entries `LanePrivacyCommitment`:

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

- **`LaneCommitmentId`** - identificador estable de 16 bits registrado en manifests de lane.
- **`CommitmentScheme`** - `Merkle` o `Snark`. Variantes futuras (p. ej., bulletproofs) extienden el enum.
- **Semantica de copia** - todos los descriptores implementan `Copy` para que configs los reutilicen
  sin churn de heap.

El registry viaja junto al bundle de lanes de Nexus (`scripts/nexus/lane_registry_bundle.py`).
Cuando gobernanza aprueba un nuevo compromiso, el bundle actualiza tanto el manifest JSON como el
overlay Norito consumido por admission.

### Esquema de manifest (`privacy_commitments`)

Los manifests de lanes ahora exponen un array `privacy_commitments`. Cada entry asigna un ID y
esquema e incluye los parametros especificos del esquema:

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

El bundler del registry copia el manifest, registra las tuplas `(id, scheme)` en `summary.json`, y
CI (`lane_registry_verify.py`) reparsea el manifest para asegurar que el summary coincide con el
contenido en disco.

## Compromisos Merkle

`MerkleCommitment` captura un `HashOf<MerkleTree<[u8;32]>>` canonico y una profundidad maxima de
ruta de auditoria. Operadores exportan la raiz directamente desde su prover o snapshot del ledger;
no hay re-hashing dentro del registry.

Flujo de verificacion:

```rust
use iroha_crypto::privacy::{LanePrivacyCommitment, MerkleWitness, PrivacyWitness};

let witness = MerkleWitness::from_leaf_bytes(leaf_bytes, proof);
LanePrivacyCommitment::merkle(id, commitment)
    .verify(PrivacyWitness::Merkle(witness))?;
```

- Pruebas que exceden `max_depth` emiten `PrivacyError::MerkleProofExceedsDepth`.
- Audit paths reutilizan las utilidades `iroha_crypto::MerkleProof` para que pools shielded y
  lanes privadas Nexus compartan la misma serializacion.
- Hosts que ingieren pools externos convierten hojas shielded via
  `MerkleTree::shielded_leaf_from_commitment` antes de construir el witness.

### Checklist operativo

- Publicar la tupla `(id, root, depth)` en el summary del manifest de lane y el bundle de evidencia.
- Adjuntar la prueba de inclusion Merkle para cada transferencia cross-lane al log de admission;
  el helper reproduce la raiz byte-for-byte, asi que las auditorias solo comparan el hash exportado.
- Monitorear presupuestos de profundidad en telemetria (`nexus_privacy_commitments.merkle.depth_used`)
  para que alertas disparen antes de que los rollouts excedan los maximos configurados.

## Compromisos zk-SNARK

Las entries `SnarkCircuit` vinculan cuatro campos:

| Campo | Descripcion |
|-------|-------------|
| `circuit_id` | Identificador controlado por gobernanza para el par circuito/version. |
| `verifying_key_digest` | Hash Blake3 de la verifying key canonica (DER o encoding Norito). |
| `statement_hash` | Hash Blake3 del encoding canonico de public inputs (Norito o Borsh). |
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

- `hash_public_inputs` y `hash_proof` viven en el mismo modulo; los SDKs deben llamarlos al generar
  manifests o pruebas para evitar drift de formato.
- Cualquier mismatch entre el statement hash registrado y los public inputs presentados produce
  `PrivacyError::SnarkStatementMismatch`.
- Los bytes de prueba deben estar ya en su forma comprimida canonica (Groth16, Plonk, etc.). El
  helper solo verifica el binding de hash; la verificacion completa de curva queda en el servicio
  prover/verifier.

### Flujo de evidencia

1. Exportar el verifying key digest y el proof hash en el manifest del bundle de lane.
2. Adjuntar el artefacto de prueba raw al paquete de evidencia de gobernanza para que auditores
   puedan recomputar `hash_proof`.
3. Publicar el encoding canonico de public inputs (hex o Norito) junto con el statement hash para
   replays deterministas.

## Pipeline de ingestion de pruebas

1. **Manifests de lanes** declaran que `LaneCommitmentId` gobierna cada contrato o bucket de
   programmable-money.
2. **Admission** consume entries `ProofAttachment.lane_privacy`, verifica los witnesses adjuntos
   para la lane enroutada via `LanePrivacyRegistry::verify`, y pasa los commitment ids validados
   a lane compliance (`privacy_commitments_any_of`) para que reglas allow/deny de programmable-money
   impongan presencia de pruebas antes de encolar.
3. **Telemetria** incrementa contadores `nexus_privacy_commitments.{merkle,snark}` con `LaneId`,
   `commitment_id` y outcome (`ok`, `depth_mismatch`, etc.).
4. **Evidencia de gobernanza** usa el mismo helper para regenerar reportes de acceptance
   (`ci/check_nexus_lane_registry_bundle.sh` se engancha a la verificacion del bundle una vez que
   aterriza metadata SNARK).

### Visibilidad para operadores

El endpoint `/v1/sumeragi/status` de Torii ahora expone el array
`lane_governance[].privacy_commitments` para que operadores y SDKs puedan comparar el registry live
contra los manifests publicados sin re-leer el bundle. El snapshot se construye dentro de
`crates/iroha_core/src/sumeragi/status.rs`, se exporta por los handlers REST/JSON de Torii
(`crates/iroha_torii/src/routing.rs`), y lo decodifica cada cliente
(`javascript/iroha_js/src/toriiClient.js`,
`python/iroha_python/src/iroha_python/client.py`, `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`),
replicando el esquema del manifest tanto para raices Merkle como para digests SNARK.

Lanes de solo compromiso o replicas divididas ahora fallan admission si su manifest omite la
seccion `privacy_commitments`, asegurando que los flujos programmable-money no puedan iniciar hasta
que los anclajes deterministas de prueba se publiquen con el bundle.

## Registry de runtime y admission

- `LanePrivacyRegistry` (`crates/iroha_core/src/interlane/mod.rs`) hace snapshot de las estructuras
  `LaneManifestStatus` del loader de manifest y almacena mapas de compromiso por lane con claves
  `LaneCommitmentId`. La cola de transacciones y `State` instalan este registry junto con cada
  recarga de manifest (`Queue::install_lane_manifests`, `State::install_lane_manifests`), asi
  admission y la validacion de consenso siempre tienen acceso a los compromisos canonicos.
- `LaneComplianceContext` ahora lleva una referencia opcional `Arc<LanePrivacyRegistry>`
  (`crates/iroha_core/src/compliance/mod.rs`). Cuando `LaneComplianceEngine` evalua una transaccion,
  los flujos programmable-money pueden inspeccionar los mismos compromisos por lane expuestos por
  Torii antes de encolar el payload.
- Admission y la validacion en core mantienen un `Arc<LanePrivacyRegistry>` junto al handle de
  manifest de gobernanza (`crates/iroha_core/src/queue.rs`, `crates/iroha_core/src/state.rs`),
  garantizando que modulos programmable-money y futuros hosts interlane lean una vista consistente
  de los descriptores de privacidad incluso cuando rotan los manifests.

## Enforcement en runtime

Las pruebas de privacidad de lanes ahora viajan junto con attachments de transaccion via
`ProofAttachment.lane_privacy`. Torii admission y la validacion de `iroha_core` verifican cada
attachment contra el registry de la lane ruteada usando `LanePrivacyRegistry::verify`, registran
los commitment ids probados y los pasan al engine de compliance. Cualquier regla que especifica
`privacy_commitments_any_of` ahora evalua a `false` a menos que un attachment coincidente se
verifique con exito, por lo que las lanes programmable-money no pueden encolarse ni confirmarse sin
el witness requerido. La cobertura unitaria vive en `interlane::tests` y las pruebas de lane
compliance para mantener estable la ruta de attachments y
los guardrails de politica.

Para experimentacion inmediata, ver los tests unitarios en
[`privacy.rs`](../../crates/iroha_crypto/src/privacy.rs) que demuestran casos de exito y fallo para
compromisos Merkle y zk-SNARK.
