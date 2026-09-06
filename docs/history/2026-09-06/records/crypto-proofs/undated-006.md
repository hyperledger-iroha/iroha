# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-64df804da42b3c0d570bc32b382b0bf1fa8b6f6c3b1d8336d3c00969af2fe0a1"></a>

<!-- Original context: Roadmap / Privacy, ZK, and FHE -->
- Keep BFV evaluation-key metadata bounded and canonical. The crypto validator
  now caps rotation-key and Galois key bundles, rejects duplicate Galois
  automorphism powers, rejects noncanonical, delimiter-shaped, or oversized
  bootstrap key ids, and rejects zero or oversized bootstrap refresh-round
  capacities before bundle digests or refresh operations admit them. Programmed RAM-LFE public
  parameters now reject Galois, rotation, and bootstrap refresh material
  entirely because first-release identifier programs consume only the
  relinearization key.


<a id="record-310f5a702fadc89728e2a6689aa3c3b31f61159e699d5e6ebbc25ee324af219e"></a>

- Keep signed and proof-attestation identifier receipt coverage fixture-backed
  instead of local-only. The current shared fixture pins
  canonical payload bytes, Iroha prehash, resolver signature, signed/proof
  attestation bytes, and adversarial receipt/policy mutations across
  the Rust data model, Torii runtime claim-receipt signing path, JavaScript,
  Python, Swift, Kotlin/JVM, and Java Android.


<a id="record-182b6b2ff416e7d3d5af94a12011e9d26359f48e2b13dbf20bca26a1b020a28c"></a>

- Keep proof-carrying RAM-LFE policy metadata canonical and bounded. The crypto
  public-parameter parser now rejects noncanonical verifier backend/circuit ids,
  zero hidden-program digests, zero public-input schema hashes,
  empty/all-zero verifier keys, and oversized verifier keys before proof
  policies are admitted.


<a id="record-1951cc710a095bda8bdd15fe779c3ee325e970b0e0aec9a1dac9a5f4628286cf"></a>

- Keep ZK-ACE authorization SDK surfaces aligned with executable chain
  support. Python now exposes identity commitment lifecycle instructions,
  authorized-transfer submission, and fail-closed capability metadata so BOI
  Privacy Lab and other catalog consumers do not advertise ZK-ACE execution
  when the native instruction surface is stale. JavaScript prepared-proof
  builders now also enforce ZK-ACE public-input version `1` and reject
  authorization proofs whose public inputs do not match the requested
  transparent transfer fields before an instruction is emitted. Core
  chain-admission tests now cover rotated and revoked identity commitments,
  unsupported action classes, transaction digest/account substitution, and
  mutated ZK-ACE/STARK public inputs. The ZK-ACE prover fixture account helper
  now derives deterministic Ed25519 accounts through `KeyPair::try_from_seed`
  and compares the resulting account id with the checked backend public key in
  focused coverage. The shared data-model
  `OpenVerifyEnvelope` now exposes reusable admission validation with
  JS-aligned default bounds for proof bytes, public-input metadata, and
  auxiliary metadata, rejecting unsupported backends, blank circuits, zero
  verifier-key hashes, empty payloads, oversized payloads, and admission
  auxiliary bytes before backend-specific proof logic runs.


<a id="record-b66ee2450b05f96463bb5635216789ee6acd3ac539705b3a165fbf31530a96a5"></a>

- Fold focused ZK/FHE adversarial tests into the long workspace validation
  corridor, including the explicit full-domain PQ-MASP, IVM private-note, and
  maximum-batch ZK-AMS release gates in a serial, resource-isolated lane.

**Next checkpoints:** replace the generated full-bootstrap proof-key fixture
payloads with externally audited prover/verifier artifacts, carry the
descriptor/residue and native-vector fixture corridors into broader release
validation, and fold the focused ZK/FHE fixture corridor into the long workspace
validation path.


