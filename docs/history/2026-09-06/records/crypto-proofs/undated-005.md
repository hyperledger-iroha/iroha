# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-ea669926367467f9723509ba95206eafe48ccff8ba44500c65e4eca6c806e8ae"></a>

<!-- Original context: Roadmap / Privacy, ZK, and FHE -->
- Keep BFV encrypted-input SDK vectors shared instead of local-only. The current
  fixture set covers the baseline identifier envelope plus Soracloud three-input
  Add and Multiply operand envelopes in JavaScript, Swift, Kotlin/JVM, and Java
  Android test surfaces, with deterministic `{0, t, -t}` error-polynomial
  sampling instead of zero-error ciphertexts, and exact/bounded seeded
  encryption resamples inert all-zero ternary ephemeral masks before ciphertext
  construction. Exact/bounded key generation and key-switch generation resample
  inert all-zero public `a` limbs, and shared key-switch validators reject
  all-zero public `b` or `a` entry components. Bounded keygen, encryption,
  Galois keygen, bootstrap refresh-round seed derivation, and full-bootstrap
  sample-extraction switch-key derivation now use exact/bounded mode-separated
  deterministic RNG streams so same-seed artifacts do not reuse public limbs,
  ephemeral masks, or refresh-round seeds across modes, including
  same-public-key exact/bounded bootstrap refresh derivations that are accepted
  only by their matching transcript validators, transcript digests, and
  transcript-bound proof statements; crypto regressions now pin the bootstrap
  round seed preimage to `domain || key_id || max_rounds || seed ||
  round_index` and prove it is distinct from rotation seed derivation, and the
  Soracloud bootstrap-key proof public-input schema advertises those
  exact/bounded rotation and bootstrap refresh-round seed-derivation domains
  with its regression parsing seed-domain, statement-domain,
  refresh-transcript-domain, refresh-material, and proof-statement-material
  objects and binding their fields to the authoritative `iroha_crypto`
  constants. The full-bootstrap material/execution schema regressions also
  parse proof-key commitment-domain objects and bind material/pair fields to
  the same crypto constants, and structurally parse release-audit evidence,
  signoff, record, manifest, and package metadata so
  versions, field counts, digest domains, manifest scope, reviewer-id bounds,
  and audit byte bounds stay tied to crypto constants. The execution schema
  regression now also parses the execution witness, arithmetic trace, arithmetic
  AIR, native AIR envelope, artifact bundle, and release-prover input sections
  so row-shape constants, composition challenge layout, and verifier-obligation
  flags stay tied to `iroha_crypto`; the material schema regression parses
  `material_proof_input` so its proof-input material layout and required
  material/artifact/statement obligations are structurally pinned too. The
  input-admission and public-key schema regressions now parse their small
  bound/domain/material objects so exact/bounded proof domains and statement
  material layouts stay tied to the same constants. The typed crypto proof
  public-input schema artifact now carries the same release-prover digest-domain
  labels, proof-key material/pair commitment domains, and explicit
  separated-domain flags, with adversarial validation rejecting
  placeholder/canonical drift before those domains can diverge from the
  Soracloud execution schema. Release-audit proof-profile records now mirror
  those release-prover digest-domain and proof-key commitment-domain labels,
  advertise field count `58`, and reject stale, placeholder, or non-separated
  domain metadata before release evidence can be digested or accepted.
  Those SDK lanes now
  also validate the shared operation fixture's component-level
  evaluation-key metadata so
  missing, zeroed, duplicate, or count-drifted key-component vectors are caught
  outside Rust, while the Rust executor consumes the same fixture for
  operation-output digests and plaintext-slot checks. Crypto
  identifier-envelope admission now
  rejects structurally valid but unregistered BFV parameter profiles before
  identifier encryption, decryption, or downstream Torii/core validation, caps
  `max_input_bytes` at the registered 63-byte/64-slot RAM-LFE identifier
  profile across Rust, JS, Swift, Kotlin/JVM, and Java Android clients, computes
  identifier envelope slot counts with checked max-input-plus-length-slot
  arithmetic, and identifier slot encoding now reports byte-length and
  slot-index conversion failures through `BfvError` instead of panic-only
  assumptions. Always-built
  BFV scalar modular addition, multiplication, and coefficient reduction now
  avoid post-reduction `expect` conversions while preserving max-width
  `u64::MAX` modulus behavior, and the RAM-LFE default programmed BFV hidden
  program now uses profile-sized `u16` constants instead of runtime
  `usize`-to-`u16` conversion assumptions; programmed BFV hidden-program
  admission now also rejects `LoadInput` indexes above the encrypted envelope's
  advertised `max_input_bytes`; programmed BFV memory RNG transcript
  derivation now binds `u64` step values directly instead of converting through
  a panic-only `expect`; BFV/RAM-LFE domain-separated digest, receipt, and
  RNG-seed transcripts now stream hash chunks directly while preserving the
  previous contiguous byte layout; the feature-gated BFV acceleration selector now
  deterministically falls back to scalar schoolbook multiplication for zero or
  overflowed derived convolution lengths, and the CRT-NTT helper path now
  rejects invalid operand lengths, unsupported NTT lengths, and CRT
  reconstruction overflow before using that same fallback instead of panicking
  on degree or NTT arithmetic.
  Programmed RAM-LFE BFV bundle construction now also keeps only fallible
  production constructors that reject unregistered identifier profiles and
  invalid proof metadata before public-parameter digests are emitted, and
  programmed BFV public-parameter decoding rejects encrypted-envelope
  capacities above the canonical profile slot count. BFV now also has a registered RAM-LFE
  v1 RNS coefficient-modulus chain descriptor whose validation requires
  bounded, strictly increasing odd-prime, NTT-friendly, pairwise-coprime limbs,
  bound primitive `2n`-th negacyclic NTT roots for the registered profile, a
  checked product that covers the active ciphertext modulus, a stable
  domain-separated chain digest, and a separate exact-lift compatibility bound
  while the full BFV-RNS arithmetic engine is still pending. The shared RNS
  validator now also validates BFV parameters directly, so exact-lift and exact
  `Z_q` coverage helpers reject malformed parameter profiles before inspecting
  chain arithmetic bounds, and validated limbs must have bounded concrete
  negacyclic NTT root support. The descriptor now also supports checked limb-major
  polynomial decomposition and CRT
  reconstruction, with malformed residue-shape and unreduced-residue rejection
  before arithmetic code can consume those residues, plus deterministic scalar
  residue addition and per-limb NTT-backed negacyclic multiplication with
  bounded primitive-root discovery and a scalar fallback in the RNS chain
  product ring, and the shared Soracloud
  operation fixture now binds that descriptor, digest,
  decomposition/reconstruction corridor, and residue addition/multiplication
  hashes across Rust plus lightweight JavaScript, Swift, Kotlin/JVM, and Java
  Android shape checks. The same fixture now also binds the canonical Galois
  key-switching bundle shape, including automorphism powers and per-entry
  `b`/`a` coefficient-vector hashes, across those SDK lanes. The RNS chain now
  also exposes guarded exact
  ciphertext-modulus polynomial addition and negacyclic multiplication for
  sufficiently wide chains, plus exact RNS-backed ciphertext addition,
  multiplication, relinearization, and Galois key-switch bridges that match the
  scalar evaluator on small wide-chain profiles; the registered bounded affine
  evaluator now composes public weights, row accumulation, and output biases
  through registered bounded public-term helpers. The registered RAM-LFE chain
  is now wide enough for that guarded exact `Z_q` bridge, so Rust exercises
  exact RNS ciphertext addition, multiplication/relinearization, and Galois
  key-switching against the production RAM-LFE parameters while retaining a
  separate rejection test for the narrower exact-lift compatibility corridor.
  The programmed RAM-LFE BFV runtime now uses that registered exact RNS bridge
  for ciphertext add, subtract, multiply/relinearization, and `SelectEqZero`
  exponentiation/selection arithmetic; plaintext-scalar operations remain
  scalar because they do not require RNS polynomial products. The public
  Soracloud BFV job executor now uses the same registered exact RNS bridge for
  Add, Multiply, packed and outer `RotateLeft`, and bounded Bootstrap refresh
  rounds, keeping operation-output vectors on the production job path. The
  deterministic BFV baseline now also has packed-polynomial Galois
  automorphism keys that switch `sigma_k(s)` ciphertexts back to the original
  secret key after applying `x -> x^k`, with regressions covering canonical
  odd powers, malformed key rejection, plaintext automorphism parity, exact-RNS
  scalar parity, and registered-chain exact-RNS parity. Scalar and exact-RNS
  key-switch primitives now also validate full decomposition-entry inventories
  before operands, switching components, or digit polynomials, so malformed key
  material cannot silently truncate or mask a switch. The registered
  batch-friendly `t = 257`, `n = 64` profile now also has deterministic packed
  plaintext slot encoding/decoding plus shared Soracloud scalar and packed
  Galois execution vectors that encrypt deterministic inputs, apply the public
  Galois key-switch, and verify output ciphertext/plaintext digests plus the
  packed-slot permutation across Rust and SDK fixture-shape checks. JavaScript,
  Swift, Kotlin/JVM, and Java Android now parse `compact-v1` BFV Norito length
  encoding and reproduce the Rust-compatible compact operation-input encryption
  stream for the non-packed Soracloud Add, Multiply, outer `RotateLeft`, and
  Bootstrap input vectors, while packed-slot operation inputs remain Rust
  execution vectors plus SDK fixture-shape/digest checks outside the
  identifier-envelope builders. Public
  bootstrap refresh keys now also bind an explicit `max_refresh_rounds` and
  carry domain-separated public refresh ciphertexts for each authorized round;
  Soracloud runtime rejects `Bootstrap` jobs whose requested count exceeds that
  key capacity, computes bootstrap residual admission through the same
  key-aware capacity check, and consumes refresh material by round index.
  Soracloud runtime now also routes single-ciphertext packed
  `RotateLeft` envelopes through public Galois key switching, including masked
  schedules for rotations that are not one automorphism; raw packed rotation
  helpers validate the complete supplied Galois-key slice for bounds,
  duplicates, and malformed entries before scheduled-key lookup, while missing
	  schedule keys fail closed. Shared BFV key validators also validate parameter
	  sets before key shapes, so malformed profiles cannot reach decomposition
	  math through direct secret/public/rotation/evaluation/bootstrap key checks,
	  bootstrap-key validators reject declared round-refresh count mismatches
	  before inspecting refresh ciphertext shapes, exact and bounded refresh-only
	  bootstrap key constructors reject inert all-zero public-key material before
	  deriving encrypted-zero refresh masks and advertise the same public-key
	  preflight in the bootstrap-key zero-refresh schema, refresh-only bootstrap
	  direct execution rejects inert public-key digest metadata while
	  transcript/proof-statement validation rejects stale or placeholder
	  public-key digest metadata, and parameter validation now uses
	  checked raw/scaled exact-arithmetic products instead of saturating
	  accumulator guards.
  Key-owner diagnostics now also verify that generated public rotation and
  bootstrap refresh ciphertexts decrypt to zero under the matching secret key,
  including a bundle-level check over every rotation and bootstrap refresh
  mask, and public bootstrap admission requires a verifier-backed statement
  proof envelope. Binding-only STARK AIR is rejected after metadata preflight;
  admission remains closed until a dedicated zero-refresh witness AIR proves
  the encrypted-zero and round-consistency relation. Public deterministic
  transcript checks now recompute
  rotation and bootstrap encrypted-zero refresh material from the advertised
  seed, public key, key id, and round count, rejecting wrong-seed,
  key-id-drifted, or tampered refresh ciphertexts without requiring a secret
  key; the same check now runs at the evaluation-key bundle level so admission
  cannot accidentally validate only a subset of public rotation/bootstrap
  refresh masks. The validated transcript inventory now also binds public seed
  metadata bounded by the shared BFV deterministic seed cap, bootstrap key-id
  metadata bounded by the shared BFV bootstrap key cap, direct and
  binary-decorated placeholder rejection for deterministic and refresh
  transcript seeds, and rotation inventory metadata bounded by the shared BFV
  evaluation-key rotation cap plus a stable
  domain-separated digest over the parameter set, public key, evaluation-key
  digest, and transcript seed metadata, giving governance/admission code a
  canonical value to bind in the bootstrap-key proof envelope. The crypto layer
  now also exposes exact-lift and bounded-noise transcript-bound
  bootstrap-key zero-refresh proof statement digests that bind parameters,
  public key, evaluation-key digest, refresh-transcript digest, bootstrap
  transcript seed/key id/round capacity, and every public refresh ciphertext
  under mode-separated domains. Raw bootstrap-key zero-refresh statement
  regressions also pin exact and bounded domain constants to the same typed
  bootstrap-key material. Crypto now also exposes exact-lift and
  bounded-noise ciphertext proof statement digests that bind parameters,
  public key material, public-key digest, ciphertext bytes, a non-inert
  ciphertext digest, and the declared residual/noise bound under
  mode-separated domains, with regressions pinning both exact and bounded
  domains to the same typed Norito statement material, rejecting all-zero
  ciphertext sentinels before a verifier-facing statement hash can be emitted.
  Exact ciphertext statement
  hashing now also runs the exact seeded-encryption residual headroom preflight,
  matching exact public-key statement admission so structurally valid but
  non-admissible exact profiles cannot emit verifier-facing ciphertext
  statement hashes. Data-model refresh transcripts
  now derive the exact-lift or bounded-noise ciphertext statement digest from
  their public key, reject inert all-zero transcript public keys in the
  public-key and ciphertext proof-statement wrappers, and the input-admission
  public-input schema advertises the ciphertext statement digest
  domains/material, including the exact seeded-encryption capacity preflight on
  per-ciphertext statement hashes, under schema hash
  `828907b1ebc7d05e38e8528109feff1c92a7761ff8dda725ba1486e513bb84ad`.
  Portable validation now rejects inert all-zero proof public keys, and coverage
  pins proof public-key presence, ciphertext digest-list arity/sentinel checks,
  rejection of `fhe_public_key_digest` metadata on non-FHE state rows, and
  all-zero `fhe_public_key_digest` placeholders on FHE rows.
  Core input-admission preflight recomputes those per-slot statement digests
  from the decoded payload and proof public key, but a binding-only AIR is
  rejected at the missing dedicated ciphertext-witness boundary before any FHE
  row, public-key digest, or claimed bound metadata is persisted. The existing
  input loader still rejects corrupt persisted rows whose public-key digest
  mismatches the governed job key or whose BFV payload is an all-zero sentinel.
  `RunSoracloudFheJob` now carries an
  optional bootstrap-key proof attachment, provenance signs it, and Core
  requires it for bootstrap execution while checking the policy-bound
  statement hash and active Soracloud STARK verifier record before failing
  closed at the missing dedicated zero-refresh witness AIR boundary. The
  verifier registry now rejects canonical
  Soracloud bootstrap verifier records whose registry id, namespace, circuit
  version, public-input schema hash, gas schedule, or active inline key
	  material drift from the governed v1 profile, moving those rollout failures
	  to `RegisterVerifyingKey`/`UpdateVerifyingKey` admission. BFV bootstrap keys
	  now carry an explicit `RefreshOnlyV1` mode, and `FullBootstrapV1` keys carry
	  versioned circuit/key-material commitments that bind the canonical circuit id,
	  registered BFV parameter digest, RNS modulus-chain digest, key-switch
	  decomposition-chain digest, bootstrap artifact digests, and proof
	  public-input schema/prover-key/verifier-key digests plus typed
	  prover/verifier key-material commitments. The material validator rejects
	  zero commitments, duplicate artifact/proof/key-material commitments, and
	  artifact or proof commitments that reuse registered profile digests, keeping each
	  governed digest role partitioned at admission. Bundle admission and
	  digesting bind that material, while refresh/proof paths and direct
	  no-artifact registered execution fail closed with an explicit governed
	  artifact requirement, so the current refresh bridge cannot be mislabeled as
	  full bootstrapping. Direct
	  key-authorized refresh execution, bootstrap output-bound helpers, and
	  Soracloud exact/bounded bootstrap execution now use the same mode-aware
	  request preflight, so reserved full-bootstrap keys are rejected before
	  round-count, bound-capacity, ciphertext-shape, or refresh-key entry errors.
	  Bundle validation/digesting applies the same public metadata preflight before
	  the mode/material gate and before transcript-bound bootstrap proof statements
	  can be produced. The crypto layer also exposes a domain-separated
	  full-bootstrap material proof-statement digest that binds the parameter set,
	  public key, evaluation-key bundle digest, bootstrap-key metadata, and
	  material digest for governed prover inventories. The data-model refresh
	  transcript wrapper can derive the same full-bootstrap material statement for
	  manifest callers, and execution policies now require bootstrap-capable
		  bundles to bind exactly one bootstrap statement class: exact or
		  bounded-noise zero-refresh for `RefreshOnlyV1`, or full material for
		  `FullBootstrapV1`. Full-bootstrap
	  refresh transcript digesting omits deterministic zero-refresh bootstrap
	  transcript seeds while still checking the bootstrap public-key digest against
	  the supplied public key, and Core rejects missing, mismatched, stale, or
	  cross-mode policy statement bindings before execution. The data model now
	  also exposes a
	  distinct full-bootstrap material proof attachment with canonical
	  STARK/`OpenVerifyEnvelope` circuit id, public-input schema, byte bounds,
	  verifier-key commitment, statement public input, and envelope-hash checks, so
	  governed material proofs no longer reuse the zero-refresh bootstrap proof
	  envelope. Core now decodes material proofs through that material-specific
	  attachment context, and all Soracloud FHE STARK wrappers reject non-empty
	  all-zero native envelope bytes before backend verifier dispatch.
	  `RunSoracloudFheJob` and Torii signed FHE job requests now carry
	  an optional distinct full-bootstrap material proof attachment, provenance
	  signs it, and Core requires it for policy-bound full-bootstrap jobs before
	  dispatching through the active Soracloud verifier record or preverified-proof
	  cache path. Runtime admission rejects absent, mismatched, non-bootstrap, and
	  unverified fake full-material proofs, and
	  `RegisterVerifyingKey`/`UpdateVerifyingKey` admission rejects canonical
	  full-material verifier-profile drift before job execution. Job admission now
	  also requires the material proof schema digest and verifier key-material
	  commitment to match the canonical Soracloud proof schema and proof
	  attachment verifier commitment through the BFV crypto proof-profile
	  validator, and rejects
	  supplied full-material proof attachments that omit `vk_commitment` at the
	  material/profile gate before backend verifier lookup. The Rust, Swift,
	  Kotlin/JVM, and Java Android shared Soracloud BFV operation-fixture validators
	  now pin the full-bootstrap material/profile digest, verifier-key material
	  commitment, artifact-envelope digest, and statement vector so SDK/release
	  validation can reject fixture drift before
		  artifact-aware execution or proof verification. Full-mode exact
		  and bounded runtime bootstrap paths now use dedicated crypto preflight
		  helpers that validate governed material commitments, registered profile
		  digests, ciphertext shape, and exact/bounded metadata before direct
				  no-artifact entry points return the governed-artifact requirement. Crypto now also exposes a typed
				  full-bootstrap artifact bundle validator/digest and artifact-aware execution
				  preflight that bind concrete evaluator/proof-profile bytes to those governed
				  commitments. Each artifact byte field is now a Norito role/profile envelope
				  that declares the canonical circuit id, registered parameter/RNS/decomposition
				  digests, and max bootstrap depth, so malformed, role-swapped, stale-profile,
				  and empty-payload artifact attachments fail before artifact-aware output
					  execution. Coefficient-to-slot and slot-to-coefficient artifacts now carry
					  typed diagonal packed-slot linear transforms, and crypto keeps exact and
					  bounded deterministic evaluator/bound helpers for those transforms on the
							  crate-private registered RNS trace path. The blind-rotation artifact now
							  carries canonical packed-slot rotation schedules bound to the governed
							  accumulator artifact, and its exact/bounded registered-RNS execution and
							  bound-propagation helpers are crate-private internal trace stages that
							  consume those governed selector schedules directly. The
							  sample-extraction artifact now carries typed source/output
							  ciphertext shape and extracted-coefficient metadata, rejects opaque,
							  wrong-slot-count, bad-component-count, or out-of-range payloads. Raw
							  LWE-style sample extraction, validation, and exact/bounded raw-sample bound
							  helpers are crate-private, so canonical trace reconstruction can still
							  extract the selected `c0 + c1 * s` coefficient internally without exposing
							  a standalone raw-sample proof-material API.
							  Crypto now composes the governed coefficient-to-slot, blind-rotation, and
							  raw sample-extraction artifacts into an exact/bounded execution-prefix trace
							  with propagated bounds, coefficient-zero diagnostic repack output,
							  slot-to-coefficient diagnostic execution, and missing-key fail-closed checks.
							  Artifact-aware
							  exact/bounded final-output entry points now execute that prefix or its bound
							  propagation through governed sample-switch and slot-to-coefficient output,
							  so missing Galois keys and malformed executable artifacts fail before
							  output. The coefficient-zero raw-sample repack diagnostic bridge and its
							  exact/bounded coefficient-zero bounds are crate-private, so external
							  callers cannot treat that bridge as a standalone output API.
							  Deterministic exact and bounded raw-sample switch-key material,
							  secret-consistency checks, governed artifact carriage, and artifact-aware
							  full-bootstrap output/bound helpers still run through slot-to-coefficient;
							  standalone linear-transform, blind-rotation, switch execution, and direct
							  output-bound helpers are crate-private internal trace stages. Full-bootstrap
							  artifact-bundle validation now requires executable sample-extraction
							  switch-key material rather than accepting metadata-only sample-extraction
							  payloads in the governed bundle. Direct no-artifact registered entrypoints
							  now validate preflight, then fail with an explicit governed-artifact
							  requirement; the real proof verifier/prover backend remains unfinished. The
							  accumulator artifact now
							  carries typed packed-slot test-vector material and rejects opaque,
							  wrong-slot-count, malformed, or all-zero accumulator payloads. The proof
							  public-input schema and prover/verifier key artifacts now also carry typed
							  proof-profile payloads that bind the canonical backend, key format,
							  circuit id, statement-hash layout, governed schema digest, and inner
							  prover/verifier key role, with domain-separated key-material commitments
							  over the backend-native key bytes, while
							  rejecting opaque schema/key bytes, empty or all-zero key material, and
							  duplicate prover/verifier key material. Crypto now also exposes a domain-separated
							  full-bootstrap execution proof statement
							  digest that validates and binds the public key, governed bootstrap
							  key/material, concrete artifact bundle, input/output ciphertexts, exact or
							  bounded proof mode, input/output bound metadata, and execution-witness
							  digest for the verifier; the current exact and bounded statement
							  goldens are
							  `6eb4c58c6a1968b7fc39f36e9dfc0735f5f35506ee794d60428e94bc736f7c89`
							  and
							  `7e2c989e5b27cd0c3057999097f28b709bcf256bac643977bb48198bd0e8db8d`,
							  with both modes checked against self-describing Norito statement
							  material. The Soracloud execution proof public-input
							  schema and stable hash now advertise that witness digest, so verifier
							  records cannot retain the pre-witness claim layout by metadata accident.
							  `RunSoracloudFheJob` now carries optional full-bootstrap artifacts plus
							  an ordered execution-proof vector; provenance signs both, and Core routes
							  exact/bounded full-mode jobs through artifact-aware full-bootstrap execution
							  and bound propagation through sample-switch and slot-to-coefficient output
							  before requiring one governed execution proof per output slot.
							  Torii signed job-run requests now validate those verifier-backed proof
							  attachments locally before instruction construction, so malformed signed
							  wrappers fail as bad requests before reaching Core. Data-model and Core
							  `OpenVerifyEnvelope` admission now also reject all-zero native STARK
							  envelope bytes for Soracloud FHE input, bootstrap-key, full-bootstrap
							  material, and full-bootstrap execution proofs, while the shared
							  data-model OpenVerify admission guard rejects non-empty all-zero outer
							  proof bytes and, after enforcing configured public-input byte bounds,
							  all-zero public-input metadata before any backend verifier dispatch;
							  Core STARK verifier-dispatch and preverify coverage now pin those
							  generic rejections before backend-native proof decoding or dedup/cache
							  admission. Core's full-bootstrap material/execution proof gates now
							  share one statement-bound `OpenVerifyEnvelope` preflight for the
							  canonical STARK backend, circuit id, public-input schema, wrapper
							  version, statement-hash public inputs, and native envelope byte
							  checks, so top-level and backend-time admission cannot drift. Data-model proof
							  envelopes plus FHE parameter-set, execution-policy, Soracloud
							  uploaded-model, private-execution, agent-apartment/autonomy,
							  training metrics, HF source/shared-lease/violation evidence,
							  model provenance, and host request/response envelope digest fields
							  reject the zero prehash statement sentinel before verifier dispatch,
							  parameter admission, policy admission, job input admission,
							  ciphertext-state admission, uploaded-model receipt admission, or model
							  artifact admission.
							  `zk-stark` full-bootstrap fixtures now install governed
							  artifact-backed STARK verifier keys and generate backend-verified
							  binding-AIR `OpenVerifyEnvelope` payloads only as rejection fixtures:
							  the active full-bootstrap material and execution verifier gates reject
							  them before backend dispatch because they do not prove the BFV
							  bootstrap arithmetic. Input-admission, public-key, and bootstrap-key
							  binding-AIR payloads are likewise rejection fixtures: their public
							  binding does not prove the required BFV ciphertext, key-generation, or
							  zero-refresh witness relations.
							  Full-bootstrap material and execution proof verification now also
							  requires the dedicated native STARK/AIR verifier before acceptance:
							  non-`zk-stark` builds fail closed after envelope/verifier-record
							  binding, and preverified cache or generic backend verification is no
							  longer a fallback acceptance path for those full-bootstrap proof
							  types.
								  Core now also exposes `zk-stark` full-bootstrap material and
								  execution proof constructors that take the canonical statement hash,
								  preflight the supplied verifier-key backend, circuit id,
								  production-floor STARK/FRI shape, SHA-256 selector, and nonzero
								  statement hash, then fail closed at the dedicated BFV full-bootstrap arithmetic prover
								  boundary until the production prover is available.
								  Companion helpers now derive those statement hashes from the
								  production BFV inputs before invoking the fail-closed constructors:
								  material proofs use the
								  refresh-transcript public key and governed evaluation-key bundle, and
									  execution proofs build one slot-indexed input/output ciphertext claim
									  per output slot with the signed bound mode and bound metadata while
									  rejecting empty input slots, missing/surplus output slots, and
									  caller-supplied verifier keys that do not match the governed
									  artifact-derived execution verifier key before proof construction.
								  Canonical Soracloud FHE STARK verifier-key admission now covers input
								  admission, bootstrap-key, full-bootstrap material, and
								  full-bootstrap execution records with a shared production-floor
								  STARK/FRI payload validator, so below-floor inline verifier keys fail
								  during `RegisterVerifyingKey`/`UpdateVerifyingKey` before runtime
								  proof verification can depend on them.
								  Full-bootstrap material proof verification also preflights the active
								  record's stored STARK/FRI verifier-key payload against the canonical
								  material circuit before backend dispatch, so corrupted state cannot
								  retarget the material verifier key to the execution circuit.
								  Governed full-bootstrap execution verifier-key artifacts also decode
								  and validate the inner STARK/FRI verifier-key payload against that
								  canonical execution circuit under `zk-stark`, so opaque, below-floor,
								  or circuit-retargeted artifact bytes fail before a governed
								  `VerifyingKeyBox` is derived.
								  Core also decodes Soracloud FHE input-admission,
								  bootstrap-key, full-bootstrap material, and full-bootstrap execution
								  native `StarkVerifyEnvelopeV1` payloads before backend verification
								  and adversarially rejects transcript-label, domain-tag, missing-AIR,
								  circuit-id, trace-width, opening-count, composition-root, and
								  public-digest drift. The governed material-native AIR verifier and
								  release-native execution active verifier now also have drift coverage
								  for transcript labels, STARK parameters, trace roots, composition
								  roots, public digests, and opened composition values. For
								  full-bootstrap material and execution proofs,
								  generic binding-AIR fixtures are fully validated before being rejected
								  at the dedicated arithmetic-AIR boundary, while non-generic AIR labels
								  remain fail-closed until the production arithmetic verifier is
								  available. Execution native-AIR builder replay also rejects trace-root,
								  composition-root, and FRI base-root drift before BFV-native proof
								  wrapping, and the active release-prover execution verifier path rejects
								  the same composition-root and FRI base-root drift plus duplicated or
								  truncated sampled public-opening sets before native proof acceptance.
								  Crypto-side AIR evaluation validation now recomputes the
								  trace-bound composition vector before accepting release-prover input
								  material. The `zk-preverify` path now has poisoned-cache regressions
								  for input-admission and bootstrap-key native AIR drift plus
								  full-bootstrap material-native AIR drift, execution BFV-native AIR
								  root drift, required governed execution material, and
								  material/execution generic AIR drift, so cache hits cannot bypass
								  native envelope binding,
								  verifier-owned material checks, the required material context, or the
								  dedicated arithmetic-AIR boundary.
								  `zk-preverify` full-bootstrap regressions now prove preverified cache
								  hits cannot bypass the dedicated arithmetic-AIR boundary for material
								  or execution proof batches.
							  The confidential verifier-call defaults now admit one such Soracloud
							  full-bootstrap execution batch without an operator override.
							  Torii signed job-run preflight now resolves every signed parameter-set
							  descriptor against the registered BFV profile and runs the shared
							  policy/job admission validators plus BFV evaluation-key and
							  refresh-transcript digest checks before proof/artifact validation,
							  recomputes policy proof-statement digests from the signed
							  key/transcript material, requires policy-bound bootstrap-key and
							  full-bootstrap material proofs to be present with matching statement
							  hashes, validates supplied full-bootstrap artifact bundles against
							  the governed request material before instruction construction,
							  requires full-bootstrap execution requests to carry signed circuit
							  artifacts and a non-empty execution-proof vector whose count cannot exceed
							  the signed parameter-set slot count, rejects
							  full-bootstrap material/execution proof attachments outside
							  full-bootstrap job/key context, and rejects execution proofs that omit
							  signed artifact bundle bytes. Parameter, policy, job, key,
							  transcript, digest, or descriptor drift now fails locally before proof
							  or artifact decoding.
							  Crypto now also exposes artifact-aware validation for externally
							  held full-bootstrap execution witness, proof-input, and
							  release-prover input material, recomputing the governed prefix trace
							  from concrete artifacts and Galois keys and requiring prover/verifier
							  proof-key bytes to match the governed artifacts before callers rely
							  on those packages; the artifact-aware release-prover input digest also
							  rejects role-swapped governed proof-key artifacts directly, typed
							  release-prover packages reject prover/verifier proof-key role swaps
							  before shape-only digesting, and material-proof plus artifact-aware
							  execution package replay rejects caller-supplied BFV
							  parameter-profile, bootstrap-key, artifact-bundle, proof-key artifact,
							  and Galois-key-set retargeting;
							  material-proof caller-bound replay also rejects public-key,
							  governed-artifact, and evaluation-key retargeting, while the
							  execution witness/proof-input and release-prover replay reject
							  role-spliced evaluator artifact envelopes before package digesting.
							  Core's release-prover execution proof handoff now invokes that
							  artifact-aware prover-input validation before native AIR envelope
							  emission, so self-consistent stale prefix traces, caller-owned stale
							  Galois-key sets, stale proof-key artifacts, and role-spliced evaluator
							  artifact packages fail against the governed artifacts.
							  The lower-level material prover and execution proof helper also
							  reject stale proof-key artifacts, while the execution helper rejects
							  role-spliced evaluator artifact packages during governed verifier-key
							  derivation before deriving proof statements.
							  The public release-audit-gated material and execution provers also
							  reject role-spliced evaluator artifact packages and stale proof-key
							  artifacts at the audit
							  package boundary before proof generation starts.
							  Release-audit-gated execution proof generation also pins the requested
							  ciphertext bound mode to the matching refresh transcript mode before
							  proof material is emitted, so exact-lift transcript packages cannot be
							  replayed into bounded-noise release proofs.
							  Core governed execution verifier-key derivation now validates the
							  complete artifact bundle before decoding verifier-key material, so
							  drifted non-verifier artifacts fail at that helper boundary.
							  Core regressions now also prove that correctly shaped full-bootstrap
							  execution proof attachments fail closed before backend verification
							  when the governed verifier record is missing or withdrawn.
							  The Core proof helper now also reruns local job-shape validation,
							  requires input-bound metadata to match the input envelope count,
							  and rejects missing/surplus output slots before deriving proof
							  statements, so stale bound sidecars, stale output sidecars, and
							  multi-input bootstrap drift cannot reach proof verification.
							  It also rejects full-bootstrap execution circuit artifacts outside
							  full-bootstrap proof context even when no execution-proof attachments are
							  supplied, so artifact-only bypass attempts fail at the proof boundary.
							  Core regressions also pin full-bootstrap execution verifier-record
							  metadata drift across namespace, backend, curve, public-input schema,
							  circuit/version, gas schedule, active circuit mapping, proof byte caps,
							  key presence/length, commitment, and governed verifier-key byte binding.
							  Core now also forges governed verifier-key artifacts with empty and
							  all-zero backend-native key bytes, proving inert key material fails
							  before verifier-record lookup even when artifact digest/commitment
							  metadata is recalculated.
							  Release-audit trusted reviewer ids now also share the external
							  audit-artifact placeholder scanner, so draft, fake, to-do,
							  pending-audit, sample, template, example, and not-production-ready
							  reviewer labels fail before signoff construction, trusted-reviewer
							  checks, package validation, or
							  governed artifact validation can mask malformed trust anchors.
							  Standalone release-audit manifest validation now also uses the
							  full reviewer public-key payload preflight, so empty or all-zero
							  manifest reviewer keys fail before manifest digesting can bless
							  malformed trust anchors.
							  Soracloud's material and execution public-input schemas now advertise
							  the same `rejects_placeholder_reviewer_ids` release-audit contract
							  and pin the updated schema hashes.
							  The typed proof-key artifact encoder now also rejects stale declared
							  key-material commitments before emission, and crypto regressions pin
							  artifact-derived commitment rejection for role swaps, depth drift,
							  stale commitments, and inert backend-native key bytes.
							  Material proof-statement regressions now also drift prover/verifier
							  key-material commitments directly, proving those commitments change
							  both the crypto statement digest and Soracloud's transcript-derived
							  policy digest.
							  Full-bootstrap execution proof statements now bind the zero-based output
							  slot index, and Core rejects slot-position replay even when duplicate
							  ciphertext slots would otherwise produce identical input/output claims.
							  Full-bootstrap jobs now also require `bootstrap_count == 1` in Core
							  execution, bound propagation, proof verification, and Torii
							  signed-request preflight, so the one-proof-per-output-slot statement
							  cannot be replayed as a multi-round full-bootstrap claim.
							  Core now also preflights full-bootstrap execution-proof material after
							  loading FHE inputs and before artifact-aware execution, rejecting proof
							  vectors whose length does not match the actual input/output slot count
							  before the heavier arithmetic path runs.
							  Exact and bounded-noise Core runtime coverage now also rejects drifted
							  signed artifact bundles, role-swapped artifact envelopes, and stale
							  prover/verifier key-material commitments before Galois-key availability
							  or final output execution.
							  Full-bootstrap proof-key payloads now also bind the canonical execution
							  public-input layout and a generated prover/verifier pair commitment;
							  governed material stores that pair commitment and Core/Torii recompute
							  it from decoded proof-key artifacts before accepting signed material.
							  Native proof-key payload shape validation now rejects blank payload bytes
							  and known direct/delayed placeholder or inert native payload sentinels
							  before Norito decoding, so raw prover/verifier payload validators and
							  material constructors fail closed at the payload boundary. Outer native
							  proof-key material bytes and proof-key material envelope bytes now reuse
							  the same raw text-sentinel guard before Norito decoding, and generated
							  native circuit body validation applies it before digest and
							  canonical-body comparison, so digest-correct template or handoff text
							  cannot fall through to generic material, envelope, or body-drift
							  handling.
							  Torii signed-request preflight coverage now also rejects full-bootstrap
							  artifact attachments outside full-bootstrap context and binds a matching
							  signed material digest to a role-swapped artifact envelope before
							  rejecting the wrong declared role or stale prover/verifier key-material
							  commitments locally before instruction construction.
							  The legacy no-artifact Core execution helpers are test-only, so production
							  full-mode jobs must pass through the governed artifact-aware path; the
							  no-artifact residual-bound wrapper is also test-only, keeping the
							  non-test Core path on artifact-aware execution and bound propagation.
							  Direct exact and bounded no-artifact crypto helpers now also reject drifted
							  governed full-bootstrap material before artifact availability, so stale
							  material cannot be masked by the expected artifact-required boundary.
							  Full-bootstrap artifact-bundle digests now use typed digest material with
							  version, artifact-digest count, and per-role artifact hashes, with valid
							  alternate-artifact regressions pinning every mutable artifact role that
							  can vary under the first-release profile.
							  Full-bootstrap execution proof statement tests now also pin canonical
							  exact and bounded proof-mode digest goldens for that typed artifact-bundle
							  layout.
									  Full-bootstrap proof-profile schema/key tests now flip every required
									  statement/claim binding, native Merkle/FRI replay and AIR-root FRI query
									  binding flag, and proof-key
								  commitment component, pinning the prover/verifier artifact contract plus
								  canonical proof schema artifact
								  (`a1354821e8d00ab90629e00a685827151076b813d132cb10e7684a4ab84b556b`),
								  governed parameter/profile/depth-bound prover/verifier key-material
									  commitments, and prover-key commitment digests before release-grade keys
									  are admitted. Data-model
								  tests also pin the Soracloud FHE public-input schema hashes that
								  verifier records use for input admission, bootstrap-key proof,
								  full-bootstrap material proof, and full-bootstrap execution proof gates.
								  The input-admission schema now advertises the exact-residual and
								  bounded-noise bound modes, capacity validation, and ciphertext
								  proof-statement digest domains/material, including the exact
								  seeded-encryption capacity preflight on per-ciphertext statement hashes,
								  under schema hash
								  `828907b1ebc7d05e38e8528109feff1c92a7761ff8dda725ba1486e513bb84ad`.
									  The typed crypto schema and full-bootstrap execution schema now advertise
										  artifact-bound release-prover input validation, stale
										  Galois-key-set replay including embedded digest retargeting,
										  stale proof-key artifact rejection,
											  transcript-derived opening schedule/public-padding replay, native
											  Merkle/FRI verifier replay, AIR-root FRI query binding, and canonical
											  base transcript-label plus suffixed-label alias rejection.
											  Release-audit evidence now exposes those replay-policy guarantees in
											  its proof-profile record with field count 58, including release-prover
											  digest domains and proof-key material/pair commitment domains; native
											  proof-circuit fingerprint material binds the same guarantees with field
											  count 48, generated circuit bodies carry them with field count 49, and
											  the current release-audit package schema validates proof-profile
											  field-count/label-obligation/domain markers, generated-body byte length/hex,
											  evaluator artifact hex for coefficient-to-slot, slot-to-coefficient,
											  blind-rotation, sample-extraction, and accumulator artifacts, proof
											  public-input schema and arithmetic AIR artifact hex, native
												  prover/verifier payload hex, governed prover/verifier artifact hex,
												  release-audit evidence per-artifact digest bindings for those evaluator,
												  proof-schema, and arithmetic-AIR artifacts, and same-field signed
													  commitment containment with standalone label tokens,
													  explicit value separators, standalone value tokens, and
													  relabel/cross-field/punctuation/conflicting- or same-value duplicate replay
													  rejection, plus byte-leading lowercase external-review report/archive
													  markers, required marker-colon separators, printable-ASCII
													  reviewer-id-labelled non-empty
														  marker statements with bare reviewer-id prose, duplicate/conflicting
														  reviewer-id label, lowercase/case-drifted/separator-alias reviewer-label
														  rejection, and reviewer-id-only statement rejection,
													  missing-colon/padded-colon-alias/empty/generic-statement rejection,
													  raw-byte, colon-separator, uppercase signed-digest/label, and same-value duplicate signed-commitment rejection, and
												  machine-generated or separator-obfuscated machine-generated audit-body rejection.
												  Release-audit report/archive body regressions now pin marker tokens split across
												  non-text bytes before report byte construction, signed package validation, or
												  package digesting. Raw native
											  prover/verifier proof-key payloads now reject
											  binary-decorated and binary-fragmented placeholder text before Norito decoding or digest-catalog
												  fallback, and the Core/Torii shared FHE native-envelope preflight now rejects
												  placeholder text in printable spans split by binary framing before runtime
												  proof attachment admission while caching collapsed marker variants. The current
														  placeholder matcher caches collapsed marker material during
														  mixed-binary artifact scans. Shared full-bootstrap material digest
														  sentinels also include direct and delayed `0xff`-framed variants of
														  the known placeholder preimages plus evaluator-artifact-set
														  domain-scoped `0xff`-framed transient digests, and native payload plus
														  external audit artifact digest sentinels reject delayed binary-framed
														  variants too, and Soracloud execution-policy admission rejects direct,
														  delayed, and leading-whitespace delayed `0xff`-framed caller-pinned
														  release-audit package digest placeholders before package validation
														  or digest mismatch can mask policy errors.
														  material/execution schema hashes are
															  `fdfe1d3454a0f3fac98684f24af74bbb0286dda732e7d34e1d99677fcbbc5acb`
															  and
															  `0f8fcf3c6cb5f2889d471dc7b656d8f6174289ce0d1ab40b13076da5fdd5443d`.
											  Registered bounded-noise compatibility wrappers for multiplication, Galois
											  switching, outer-slot rotation, packed rotation, and bootstrap refresh now
											  delegate to the registered target-limb basis-extension corridor, so older
											  registered API names still validate the production
											  decomposition/evaluator-chain binding. Bounded outer-slot rotation public-bound
											  propagation now also has direct and registered target-limb
											  basis-extension wrappers, and Core uses the registered wrapper for
											  multi-slot bounded `RotateLeft` bound checks.
											  Core release-audited material and execution prover entrypoints now also
											  reject release packages that downgrade AIR-root FRI query-binding before
											  native proof emission.
											  Shared Soracloud operation vectors now install constructor-built
											  no-refresh `FullBootstrapV1` keys and keep governed full-bootstrap
											  material pinned to the crypto proof schema artifact digest
											  `a1354821e8d00ab90629e00a685827151076b813d132cb10e7684a4ab84b556b`,
											  with prover-key digest
											  `a138d4ba7125de0ff8a368d82d13c697986ced91ed8b8b9c468bc3b694a26929`,
											  prover-key material commitment
											  `66c2f9dbdabcc89150468d3369d1ff7c78824c01211091bc99bed51c4d5d0977`,
											  material digest
											  `3452f02a52628f6a78bfdac707e2fa698264cd7b35ca93ff1cbb5081dc65e5bd`,
											  and statement digest
											  `99682800da76658dc2801ee1db9896edf9803d4d5f8b374bf888584401848f7d`.
								  Bootstrap-key zero-refresh proof statements now also encode a v1
								  statement-material header plus bootstrap refresh-round count,
								  zero-refresh digest, and indexed per-round refresh digests, and the
								  public-input schema hash
								  `39809de5a8ac82f115fc3df08abffb3629adbf9dd227bccf7f9816cbc86e8563`
								  advertises those transcript, refresh-summary, and exact/bounded
								  raw/transcript statement-domain plus refresh-transcript-domain bindings,
								  including the v1 refresh-transcript material header, with the schema
								  regression checking the exported crypto material and digest-domain
								  constants directly.
								  Full-bootstrap execution claims now also carry a deterministic witness
								  digest derived from the governed artifact-aware arithmetic prefix trace
								  and its exact/bounded bound trace. Core proof generation and verification
								  recompute that digest before statement hashing, so output ciphertext,
								  output-bound, or witness-digest drift fails before STARK envelope
								  verification.
								  The proof public-input schema and prover/verifier key profile now also
								  advertise and commit to the witness digest domain, witness material
								  version/count, ciphertext trace stage count, and bound trace stage count,
								  so stale witness-layout metadata fails schema validation, key validation,
								  and key-commitment checks before backend proof work.
								  Proof-key `key_material` now carries a canonical Norito envelope that
								  binds backend-native key bytes to the role, backend, key format, circuit,
								  registered BFV profile, governed schema digest, statement/claim layout,
								  witness layout, hash shape, and supported bound modes; opaque key blobs,
								  all-zero native key bytes, envelope metadata drift, and duplicate native
								  prover/verifier key material fail before governed artifact admission.
								  The envelope's `native_key_material` is now itself typed Norito material:
								  transparent STARK/FRI prover parameters are deterministic, verifier
								  payloads must decode to the canonical SHA-256/Goldilocks FRI floor, and
								  raw, opaque, payload-digest-drifted, non-SHA, or below-floor native bytes
								  fail before Core, Torii, or data-model fixtures admit the governed
								  proof-key pair. The native proof-key envelope now also carries a
								  deterministic full-bootstrap proof-circuit fingerprint. Its material has
								  field count 48 and binds artifact-bound prover-input validation,
								  stale Galois-key-set/proof-key artifact replay rejection, and
								  transcript-derived public-opening policy plus canonical base transcript-label enforcement and suffixed-label alias rejection, so
								  circuit-shape or replay-policy drift fails before governed proof-key
									  material admission, report/archive validation requires
									  the proof-profile field count, transcript-label obligations, release-prover
									  digest domains, and proof-key material/pair commitment domains, and generated pair validation rejects
								  prover/verifier native-circuit mismatch before deriving or admitting a
								  proof-key pair commitment. Native proof-key
									  material now also rejects noncanonical native payload circuit ids
									  outright, including digest-correct embedded prover/verifier payload
									  plus generated-circuit-body circuit-id/payload-id and outer proof-key
									  envelope circuit-id replays, and governed circuit material now uses
									  the same canonical preflight along with evaluator artifact-set
									  digest material, arithmetic trace profiles, arithmetic AIR material, and
									  proof public-input schemas, proof-key governed-material checks, and artifact envelopes,
									  pinning release artifacts to
									  `iroha_bfv_full_bootstrap_v1`
									  instead of merely requiring prover/verifier pair consistency.
									  Release-audit evidence circuit-id validation now uses the same
									  canonical preflight before release package commitments are derived, and
									  release-audit signoff payloads and manifests use it before signature or
									  manifest verification. Proof-key backend/key-format, proof-key envelope
									  backend/key-format, native prover/verifier payload
									  backend/key-format/proof-system/field, native proof-key material
									  backend/key-format/proof-system/field/payload-kind labels,
									  release-audit proof-profile labels, key-evidence payload-kind labels,
									  and manifest scope now use canonical text-label preflight before
									  mismatch checks.
									  Trace-profile witness digest, arithmetic AIR composition, proof public-input
									  schema, proof-key witness, proof-key envelope witness, and generated-body
									  witness domains now use canonical byte-label preflight before
									  trace/AIR/schema/key/body digesting. Generated circuit-body
									  backend/key-format/proof-system/field labels now use canonical
									  text-label preflight before body comparison.
									  Native verifier payloads now also carry the canonical field count,
								  backend, key format, proof system, and field labels, and crypto/Core
								  validation rejects relabeled STARK/FRI verifier payloads before
								  governed proof-key material admission or artifact-derived verifier-key
								  canonicalization. Core's native-verifier fallback now also rejects
								  field-count drift before rewriting native verifier payloads into
								  canonical STARK verifier-key bytes.
								  The arithmetic trace layout is now explicit
								  `BfvFullBootstrapArithmeticTraceProfileV1` material with a canonical
								  digest bound by the proof public-input schema, proof-key material
								  envelope, native prover/verifier payloads, native proof-key material,
								  and native proof-circuit fingerprint. Crypto and Core reject
								  trace-profile digest drift before governed artifact admission or
								  verifier-key canonicalization. The profile now also binds active
								  coefficient rows as private witness rows, public deterministic padding
								  rows, and the rules that transparent native proofs must not open unmasked
									  private rows or duplicate sampled public rows; Crypto's public
									  padding-row helpers now reject zero, direct-placeholder, and
									  leading-whitespace delayed-placeholder statement hashes before constructing
									  or validating verifier-facing openings, and Core's native BFV AIR
										  public-padding verifier shares that gate while validating opened
										  public padding rows against canonical
										  statement/slot/mode headers and rejects zero statement hashes,
										  empty/all-zero AIR roots, or auxiliary generic composition-value
										  commitments before the dedicated verifier fallback.
								  Release prover input now has a typed
								  `BfvFullBootstrapMaterialProofInputMaterialV1` boundary for governed
								  full-bootstrap material proofs that binds concrete artifact bundles
								  against governed material, and the material proof public-input schema
								  and stable hash now advertise that typed input contract, including
								  governed full-bootstrap material, public-key, evaluation-key, concrete
								  artifact-bundle, statement-hash, and material proof input package
								  digest-domain bindings. Crypto also exposes a domain-separated Norito
								  digest helper for that typed material proof input package, with
								  regressions pinning the digest to the encoded self-describing Norito
								  proof-input material, and that
								  digest path now rejects role-spliced material artifact envelopes even
								  when matching digest metadata and statement hashes are recomputed. Core's
								  material proof builder now invokes that caller-bound artifact check before
								  native material proof attachments are emitted. Release prover input also
								  has a typed
								  `BfvFullBootstrapExecutionProofInputMaterialV1` boundary that binds the
								  public key, validated execution witness material, and canonical statement
								  hash before a dedicated arithmetic prover can consume the material; its
								  package digest is likewise pinned to the encoded self-describing Norito
								  proof-input material under the execution proof-input digest domain.
								  Release execution prover input now also has a typed
								  `BfvFullBootstrapExecutionProverInputMaterialV1` package that binds the
								  proof input, canonical row-major arithmetic trace material/digest,
								  canonical AIR contract digest, governed AIR artifact digest,
								  zero-residual AIR evaluation material/digest, trace-bound
								  public-opening material/digest, and governed generated
								  prover/verifier proof-key pair before the dedicated prover boundary; the
								  trace material, AIR evaluation material, public-opening material, and proof-key-bound prover-input
								  package digests are pinned to encoded self-describing Norito material under
								  their dedicated domains.
								  Crypto and Core reject stale trace digests, stale AIR
								  contract/artifact/evaluation material digests, non-zero composition
								  values, stale trace rows, trace/proof-input splicing, and unrelated
								  proof-key material or pair commitments before proof generation is
								  attempted. Core proof-emitting material and execution helpers are now
								  internal, so the callable production material and batch paths validate a
								  release audit package against governed material, concrete artifacts, the
								  caller-trusted reviewer id/key, and caller-pinned package digest, including
								  zero, known placeholder, record/manifest alias, and signed inner
								  commitment alias pinned-digest rejection; the
								  lower crypto layer now also offers release-audit-gated exact and bounded
								  artifact-aware execution and bound helpers that enforce the same trusted
								  reviewer and caller-pinned package digest boundary before output or bound
								  derivation, and Core's release-audited execution prover recomputes
								  caller-supplied outputs and bounds through those audited helpers before
								  native proof construction;
								  internal typed prover-input path still requires the caller-supplied
								  verifier key to match the verifier proof key embedded in the release
								  prover package. Core now canonicalizes governed
									  native verifier-key payloads before caller/prover-input and
									  helper/governed-artifact comparisons, so native BFV verifier-key artifacts
									  and canonical STARK boxes follow the same binding path. The proof public-input schema and Soracloud stable
									  schema hash now also advertise release-prover verifier-key binding,
									  BFV arithmetic AIR contract layout/enforcement flags, including
									  row-kind partitioning, active-row/witness consistency,
									  full-bootstrap arithmetic constraints, nonzero statement
									  hashes, and trace output/bound claim matching, and the duplicate-free
									  native opening policy, execution proof input package digest domain,
									  release-prover AIR constraint-system digest/artifact binding, and the
									  typed crypto schema validates those AIR, release-prover, and execution
									  proof input package digest-domain plus exact-residual and bounded-noise
									  admission proof-input material bindings for public statement hashes,
									  secret-key witnesses, decrypted plaintext, scaled coefficients, exact
									  residual multiples or centered-noise polynomials, declared bounds,
									  nonzero public-key/ciphertext residual/noise witnesses, and
									  resampled nonzero exact/bounded error/noise generation, plus
									  role/mode-separated canonical proof-input package digests and
									  canonical uncompressed public material, evaluation-key-bundle
									  material, governed circuit material, arithmetic trace-profile,
									  AIR contract, and proof public-input schema material,
									  artifact-bundle archives, proof-statement, and proof-input
									  material byte admission, with
									  public-key and input-admission schema regressions parsing those
									  proof-input sections plus nonzero witness and generation-resampling
									  obligations against the crypto constants
									  advertised AIR/release-prover terms directly. The
											  release-audit proof-profile record advertises those replay-policy
											  and AIR evaluation material layout/digest/zero-composition terms,
											  and the native proof-circuit fingerprint material binds the
											  replay-policy terms with field count 48. The AIR constraint-system
									  digest is also bound through the typed public schema, native
									  prover/verifier payloads, proof-key material envelope, native
									  proof-key material, and native proof-circuit fingerprint. The
									  AIR constraint-system material is now a public typed Norito artifact
									  with a canonical validator and digest-from-material helper for
									  release tooling.
								  Core typed material proof helpers now derive and validate typed
								  material before emitting a material-native STARK/FRI proof; the
								  hash-only material/execution constructors are crate-scoped internal
								  compatibility helpers that remain fail-closed at the dedicated-prover
								  boundary, leaving release-audit-gated entry points as the public production
								  prover surfaces. The test-only typed execution proof helper also derives
								  and validates the canonical
									  row-major arithmetic trace material from proof input, so stale governed
									  material, witness, statement material, or native trace rows are rejected
									  before proof generation is attempted. The native AIR fixture path now
									  uses a deterministic STARK/FRI envelope builder that commits
									  caller-validated trace rows and explicit typed AIR evaluation
									  composition values, and the Soracloud release-prover handoff builds
									  that BFV-native envelope directly from
									  `BfvFullBootstrapExecutionProverInputMaterialV1`. The AIR residual
									  evaluator now validates every opened row coordinate as a canonical
									  Goldilocks field element before returning the first nonzero
									  coordinate residual, so malformed tail coordinates cannot be masked by
									  an earlier mismatch and coordinated same-row trace drifts cannot cancel
									  back to the zero-composition vector. The active Soracloud
									  verifier path now reconstructs the governed arithmetic trace and AIR
									  evaluation material from the public execution proof input and rejects
									  trace/composition root drift plus opened rows, next rows, or composition
									  values that do not match that verifier-derived material before
									  Merkle/FRI validation or the dedicated-verifier fallback. The BFV AIR
									  composition challenge stream now binds the public statement hash,
									  canonical row-major trace-material digest, row index, and column
									  index, reduces the full 32-byte digest into Goldilocks, remaps zero
									  challenges to one, rejects zero, direct-placeholder, or
									  leading-whitespace delayed-placeholder digest inputs before challenge
									  reduction, and the typed AIR contract plus data-model
									  Soracloud execution proof public-input schema advertise that exact
									  challenge domain and binding policy with AIR material field count 33
									  and refreshed stable schema hashes. The shared explicit STARK AIR
									  builder now self-verifies generated row/composition envelopes before
									  returning proof bytes to BFV native AIR callers and rejects reserved
									  BFV/ZK-ACE/IVM circuit aliases on the generic row/composition helper
									  surface unless the caller uses an explicit reserved-circuit helper; the Soracloud
									  release-prover handoff replays encoded envelope bytes against the exact
									  typed trace rows and AIR evaluation composition values before returning
									  proof bytes, while the lower-level BFV native AIR proof builder is
									  crate-scoped so public proof generation remains on Soracloud's
									  artifact-bound release-prover path. Core material native-AIR replay
									  regressions now also pin
										  composition-root reconstruction drift, FRI
										  base-root/composition-root mismatch, and sampled governed-material
										  opening row, next-row, and composition-value drift before wrapping
										  material proof bytes. Core execution native-AIR replay now likewise
											  uses the crypto-canonical execution AIR domain tag with the canonical
											  base transcript label, and requires governed trace rows and AIR composition values at the lower
											  BFV boundary before root reconstruction, sampled row-path
											  shape/root, duplicate/truncated opening-set replay,
											  first-layer FRI replay, or verifier fallback. Release-audit
											  trust anchors now reject non-Ed25519 reviewer keys across signoff, record,
											  manifest, and package validation, and the Soracloud full-bootstrap schemas
											  advertise the Ed25519 reviewer-key requirement. Release-audit artifact
											  bodies now reject delayed nested audit report/archive headers anywhere in
											  the body, while digest-only schema claims stay limited to exact known
											  nested-header sentinels that do not require inspecting unknown preimages.
										  Remaining native-AIR production work is the BFV
										  arithmetic proof-producing backend plus release-grade generated
										  prover/verifier artifacts and audit evidence. The hand-built-root,
										  unbound opening/composition, statement-only challenge, and
										  prefix-truncated challenge gaps are already closed by the shared
										  typed AIR/native-envelope replay path.
										  Crypto release tooling can now derive governed full-bootstrap circuit
										  material directly from concrete artifact bundles by recomputing every
										  artifact digest, proof-key material commitment, and generated pair
										  commitment before validating the bundle against the derived material.
										  Release-audit archive validation indexes label/value fields once per
										  body before checking signed commitments, proof-profile obligations, and
										  governed artifact hex, and placeholder-marker scans skip directly
										  between candidate marker starts.
										  Standalone release audit evidence validation also recomputes the
										  evaluator-artifact-set digest, full artifact-bundle digest, and
									  canonical native proof-circuit fingerprint from its advertised fields,
									  so stale-but-distinct digest summaries or a matched stale
									  prover/verifier fingerprint pair cannot pass as shape-valid release
									  evidence. Standalone signoff payloads and machine-checkable manifests
									  now also recompute that canonical native proof-circuit fingerprint from
									  the release circuit id before accepting or digesting the object.
									  Stale proof-key pair commitments are rejected during derivation even
									  when individual proof-key material commitments are refreshed, and the
									  crypto/Core sample material helpers now fail hard instead of
								  synthesizing malformed sample pair commitments.
									  Core now also requires the full-bootstrap material proof verifier record
									  to carry the canonical material-proof gas schedule id and has
									  adversarial verifier-record drift coverage matching the execution-proof
									  gate. Input-admission and bootstrap-key proof verifier records now
									  likewise require their canonical gas schedule ids rather than any
									  non-empty schedule id. Full-bootstrap material and execution proof
									  statements now encode the advertised statement material version and
									  field count in the canonical hashed bytes, and the Soracloud public-input
									  schemas advertise those self-describing statement headers. The execution
									  public-input schema and stable hash now also advertise the arithmetic
									  trace private/public row policy and proof-key-bound release prover input
									  package.
									  Governed full-bootstrap artifact payloads now also reject blank text
									  plus placeholder, pending/to-do, handoff,
									  non-production, template, and example sentinels at the shared
									  payload guard before role-specific Norito decoding, while avoiding a
									  raw `sample` substring ban so sample-extraction role names remain
									  valid.
									  Full-mode bootstrap keys now carry a domain-separated BFV public-key
									  digest, and material/execution statement derivation rejects governed
									  public-key drift before hashing or proof-helper execution. Execution
									  witness material validation also recomputes the artifact-bundle digest
									  implied by governed full-bootstrap material commitments, including the
									  arithmetic AIR constraint-system artifact digest, so nonzero stale
									  artifact-bundle digests fail before public witness hashing or release-prover
										  input packaging, reconstructs the raw extracted sample and raw-sample bound
										  from the blind-rotation stage, and recomputes the deterministic
										  coefficient-zero repack ciphertext plus the coefficient-zero and
										  sample-switch bounds from the raw extracted sample before accepting typed
										  witness material.
										  Core's shared FHE STARK native-envelope preflight now rejects blank
										  text bodies plus case-insensitive placeholder,
										  non-production, handoff, sample, template, and example sentinels, including
										  dash/underscore variants, before Norito decoding. Data-model material and
										  execution proof validation plus Core material/execution preflight now pin
										  binary-fragmented placeholder text as well, including marker tokens split
										  across non-text bytes, so `0xff`-split native-envelope attachments fail
										  closed at the raw native-envelope boundary. The material and execution
										  public-input schemas also publish their dedicated native-AIR
										  envelope contracts, including statement-bound domain tags, governed
										  trace/composition root replay, query/opening count, Merkle/FRI binding,
										  auxiliary composition sidecar rejection, verifier-owned trace-material replay
										  for execution, and the same placeholder native-envelope text gates.
										  BFV-shaped native AIR envelopes now preflight the canonical
										  transcript label, statement-bound domain tag, STARK/FRI metadata,
												  public digest binding, proof/commitment version tags,
												  commitment/root shape, exact duplicate-free canonical opening/query
												  count, opened row/path shape, Merkle path-to-root binding,
												  FRI query-chain Merkle/fold validation, auxiliary
											  generic composition-value commitment rejection, AIR-to-FRI
											  base value binding, execution public-padding context, opened public
										  padding-row semantics, and the no-unmasked-private-row plus
										  duplicate-free opening policies before the current dedicated verifier
										  boundary is reported;
										  non-generic full-bootstrap native envelopes with missing, foreign, or
										  contextless BFV AIR sections now fail before that unavailable-verifier
										  boundary. The active Soracloud execution verifier now reconstructs the
										  governed arithmetic trace and AIR evaluation material from public proof
										  input and requires those governed rows plus composition values before
										  explicit STARK/FRI replay, so missing governed material, root drift, or
										  opened row/next-row/composition drift fails closed before verifier
										  acceptance.
								  Refresh-only proof and execution paths still reject `FullBootstrapV1`.
										  `FullBootstrapV1` keys are now no-refresh keys as well:
										  governed constructors and admission require `max_refresh_rounds = 0`
										  with empty `zero_refresh`/`round_refreshes`, and encrypted-zero refresh
										  material remains confined to `RefreshOnlyV1`; crypto regressions now
										  also mutate legacy `zero_refresh` and `round_refreshes` material
										  independently while `max_refresh_rounds = 0`, confirming artifact-aware
										  preflight plus exact/bounded direct execution and bound preflights fail
										  before artifact fallback.
											  Remaining work is the audited full-bootstrap arithmetic witness
											  constraint/proof-producing backend plus release-grade generated
											  proving/verifying artifacts for the actual BFV bootstrap circuit.
											  The Core verifier, proof-key, verifier-key material envelope binding, public-schema/release-prover input,
											  digest-sentinel rejection, AIR contract material/digest binding,
											  verifier-record floor, and governed proof-key-pair corridors are
											  already shipped.
	  Soracloud transcript digesting now preflights the advertised BFV public-key
	  shape before evaluation-key bundle validation, so malformed transcript key
	  material is reported at the public-key boundary instead of being masked by
	  unrelated bundle-shape errors. The crypto bundle validator
  applies the same public metadata
  preflight for direct callers. Standalone refresh-key transcript
  generators/validators also reject empty or oversized public seeds before
  deriving or recomputing encrypted-zero masks. Soracloud FHE execution
  policies now carry the refresh-transcript inventory digest,
  `RunSoracloudFheJob` signs the transcript inventory in the provenance
  payload, and core rejects jobs whose supplied refresh transcript does not
  match the governance-bound digest. This hardens the current refresh path
  while the full BFV bootstrapping engine remains open. The same
  bundle-level owner diagnostic now verifies relinearization entries against
  scaled `s^2` residues and Galois entries against scaled automorphed-secret
  residues, rejecting non-plaintext-multiple key-switch residuals and residual
  multiples above the current exact error bound; standalone Galois key
  generation now applies that residual self-check before returning key
  material. Rotation and bootstrap encrypted-zero refresh diagnostics now also
  reject zero-plaintext masks whose residual multiples exceed the deterministic
  `(2n + 1)E` refresh bound for the first-release seeded encryption format. The
  bounded-noise counterparts now also reject zero-plaintext
  rotation/bootstrap refresh masks whose centered rounded noise exceeds the
  fresh BFV noise bound, and bundle-level bounded diagnostics now identify
  indexed rotation/bootstrap refresh masks when nonzero plaintext or oversized
  rounded noise is detected.
  Seeded key generation and public-key encryption now also reject parameter
  sets whose centered `q/t` capacity is below that bound, so structurally valid
  but too-narrow profiles cannot produce first-release ciphertext/key material,
  and deterministic BFV keygen, encryption, Galois-key generation, and
  identifier seed helpers reject empty or oversized seeds before deriving RNG
  material; Soracloud refresh transcript admission derives its public seed,
  bootstrap key-id, and rotation inventory caps from the same crypto constants.
  Registered BFV profile validation and the production digest path now enforce
  the same capacity invariant before admitting the RAM-LFE profile. A separate
  rounded BFV path now generates small-noise public keys, encodes plaintexts as
  `(q / t) * m`, decrypts by deterministic rounding, rejects too-narrow
  decoding capacity, and reports owner-side centered-noise/headroom profiles as
  the migration entry point for the pending BFV-RNS evaluator; rounded
  ciphertext add/subtract, rounded plaintext-scalar addition, plaintext-scalar
  multiplication, and plaintext-polynomial multiplication also have checked
  centered-noise bound propagation. Rounded ciphertext-ciphertext
  multiplication now has a scalar exact-product bridge that performs `t/q`
  scale-and-rounding, bounded-noise relinearization, and conservative output
  budget validation, and rounded Galois key switching now has small-noise key
  generation, secret-key consistency checks, automorphism application, and
  output-bound propagation. Rounded packed `RotateLeft` now also wires those
  bounded-noise Galois switches through the public packed-selector schedule
  with matching output-bound validation before the final RNS basis-extension
  pipeline lands. RNS polynomials now also have an exact CRT basis-extension
  bridge between validated chains with target-product coverage checks, giving
  the BFV-RNS evaluator a deterministic reconstructable conversion primitive
  alongside the target-limb key-switch path. A deterministic target-limb
  basis-extension helper now computes the CRT quotient correction exactly with
  integer arithmetic and reduces source representatives into target limbs
  without requiring the target product to cover the source product; narrow
  target reconstruction remains visibly lossy, while centered target-limb
  tests pin the half-product tie as nonnegative and the first above-half
  residue as negative. Key-switch
  components now decompose directly into RNS digit polynomials, exact RNS key
  switching consumes those digits internally, and basis-extended digit inputs
  are validated against canonical decomposition ranges before use. An explicit
  target-limb basis-extension key-switch path now decomposes in a source
  chain, rejects decomposition chains that can alias base digits,
  basis-extends canonical key-switch digits through the digit-specific
  basis-extension helper without requiring the evaluator target to cover the
  full source-chain product, rejects basis-extended digit-count and RNS
  limb-shape drift at validation, and drives rounded multiplication, Galois,
  and packed `RotateLeft` bridges while matching the scalar bounded-noise
  outputs. Direct key-switch component decomposition and digit
  basis-extension helpers now enforce source/target decomposition-base
  coverage before malformed polynomial shapes can mask the public chain
  descriptor failure. Exact, target-limb, and digit basis-extension helpers now
  validate their constructed RNS polynomial outputs against the target chain
  before returning, keeping malformed target residues from escaping future
  basis-conversion changes. Full-bootstrap artifact-bundle alias preflight now
  reuses the canonical typed digest-material validator before hashing, so
  duplicate artifact-role digests fail before material proof input hashing.
  Rounded ciphertext multiplication now has an RNS exact raw-product bridge
  that decomposes ciphertext components as centered residues, reconstructs
  signed negacyclic products before `t/q` scale-and-rounding, and relinearizes
  the scaled quadratic component through the RNS digit/key-switch path while
  matching the scalar bounded-noise multiplication output. The RNS chain now
  also exposes an explicit exact scale-round helper for centered RNS product
  polynomials at the rounded BFV `t/q` boundary, and rounded RNS ciphertext
  multiplication uses that helper for direct product components plus a centered
  two-product sum helper for `c1` cross terms, with exact product-sum coverage
  rejecting aliasing before scale-and-rounding. Rounded Galois key
  switching and packed `RotateLeft` now also have RNS exact bridge entry points
  that match the scalar bounded-noise schedule and reject too-narrow chains.
  Outer-slot rotation and bootstrap refresh material can now also be generated
  and publicly transcript-validated with rounded bounded-noise encrypted-zero
  ciphertexts, refreshed through scalar or exact RNS addition, routed through
  registered target-limb RNS basis-extension wrappers for bounded production
  Bootstrap execution, and propagated with centered-noise output bounds.
  Evaluation-key bundles can now validate and digest the bounded-noise
  rotation/bootstrap transcript inventory under a separate domain from the
  exact-lift refresh path, and owner diagnostics can validate bounded relin/Galois key-switch residuals with bundle-owned
  relinearization labels and bundle-indexed Galois diagnostics plus every
  bounded refresh mask in one bundle check.
  Soracloud FHE execution policies now bind the
  refresh transcript mode, data-model digesting routes through exact-lift or
  bounded-noise transcript derivation explicitly, and core runtime admission
  rejects mode/digest mismatches before job execution. Soracloud bounded-noise
  jobs now dispatch to the bounded-noise RNS bridge for Add, outer
  `RotateLeft`, and encrypted-zero Bootstrap refresh when policy/input metadata
  are explicitly bounded, while Multiply and packed `RotateLeft` now call
  registered `iroha_crypto` helper entry points that select the smallest
  registered key-switch decomposition prefix inside the crypto layer before
  invoking the target-limb basis-extension bridge. The crypto layer now exposes
  that registered decomposition chain plus a role-separated digest so runtime
  and admission paths can share the canonical target-limb key-switch source
  basis. Registered helper entry points for bounded-noise Multiply, Galois key
  switching, and packed `RotateLeft` now derive both the canonical evaluator
  chain and source basis inside `iroha_crypto` before invoking the target-limb
  bridge, so runtime callers no longer pass evaluator RNS chains into those
  registered bounded-noise entry points. The explicit basis-extension
  key-switch path now rejects decomposition source chains that are not
  evaluator-chain prefixes while leaving the lower-level target-limb residue
  conversion primitive available for checked RNS arithmetic. Soracloud FHE
  parameter governance plus input-admission statement
  hashes now bind that digest beside the parameter and evaluator RNS-chain
  digests. Portable FHE input-admission proof validation now rejects cheap
  attachment metadata before BFV bound capacity (backend consistency,
  canonical verifier id, verifier-key commitment metadata, and envelope-hash
  presence), while still rejecting over-capacity BFV bounds before decoded
  `OpenVerifyEnvelope` admission, verifier dispatch, and verifier-record
  lookup. Soracloud registered bounded-noise runtime coverage now also drives
  two-round Bootstrap through the registered RNS refresh bridge, verifies the
  decrypted multi-slot output, and checks the propagated key-authorized
  centered-noise bound at the runtime boundary; the same bounded wrapper
  coverage now pins Multiply and packed `RotateLeft` propagated output bounds
  while decrypting the registered target-limb outputs. These deterministic
  evaluator helpers retain arithmetic and bound-propagation coverage, while
  ledger-level `RunSoracloudFheJob` coverage asserts binding-only proofs emit no
  output row or audit event until the dedicated public-key and bootstrap-key
  witness AIRs are available. The crypto layer now owns
  scalar and exact-RNS multi-round Bootstrap refresh helpers for exact and
  bounded-noise ciphertexts, rejects zero or over-capacity refresh counts before
  applying any round, and single-round scalar/RNS refresh helpers now preflight
  requested round indices before ciphertext addition. Soracloud routes exact
  and bounded-noise Bootstrap jobs plus shared operation-vector checks through
  those helpers. Registered
  exact and bounded-noise Add/Subtract, exact and bounded-noise Multiply,
  exact and bounded-noise plaintext-polynomial selector products, exact and
  bounded-noise affine row evaluators, exact and bounded-noise packed
  `RotateLeft`, outer-slot `RotateLeft`, and round-zero, indexed-round, and
  consecutive-round Bootstrap refresh helper entry points now derive the
  canonical evaluator RNS chain inside `iroha_crypto`, and Soracloud exact and
  bounded-noise runtime dispatch uses those helpers instead of passing the
  chain through core. The registered-helper rejection regression now also
  covers the decomposition-chain helpers plus exact and bounded-noise
  Subtract, exact and bounded-noise plaintext-polynomial selector products,
  exact and bounded-noise affine row evaluators, exact and bounded-noise
  Bootstrap refresh forms, and the bounded target-limb Multiply, Galois, and
  packed `RotateLeft` entry points, proving structurally
  valid but unregistered profiles fail closed before caller-supplied key
  material is inspected. Direct exact-RNS bounded-noise Add/Subtract,
  affine-row, outer-slot `RotateLeft`, and Bootstrap refresh helpers now share
  a rounded-decoding plus exact-addition RNS corridor preflight before
  supplied-chain accumulation, refresh-key checks, or ciphertext-shape checks,
  while direct exact-RNS bounded-noise Multiply, Galois key-switch, and packed
  `RotateLeft` fallback helpers now also have registered production wrappers,
  so exact-reconstruction and target-limb basis-extension paths both derive
  canonical evaluator chains before inspecting caller-controlled key material.
  Registered full-bootstrap sample-switch, prefix execution, direct execution,
  and direct bound surfaces now derive the production profile first as well, so
  malformed governed artifact/key material cannot mask an unregistered BFV
  profile.
  Bounded-noise RNS
  packed-selector products now also route through a bounded
  plaintext-polynomial RNS helper with a registered production wrapper, so
  packed `RotateLeft` mask multiplication shares the same rounded-capacity
  preflight in direct RNS, target-limb basis-extension, and registered
  target-limb paths. Public scalar addition and multiplication now also expose
  exact and bounded-noise registered helper entry points that derive the
  canonical BFV evaluator chain before plaintext/ciphertext checks, so public
  plaintext terms fail closed on unregistered profiles; the bounded scalar
  path still preflights rounded decoding capacity before applying public terms
  to bounded ciphertexts. Bounded public affine rows now reuse those helpers with
  registered RNS accumulation and owner-side rounded-noise row-bound
  propagation, so weighted public-row evaluation no longer has only an
  exact-lift surface. Bounded registered Add/Subtract, outer-slot `RotateLeft`,
  and multi-round Bootstrap wrappers now derive the registered evaluator chain
  before bounded-noise capacity checks, keeping production rejection on the
  governed profile gate for structurally valid but unregistered profiles.
  Public bounded-noise output-bound propagation now also preflights fresh
  rounded BFV noise capacity before public arithmetic, key-switch, affine,
  rotation, or bootstrap bound math, so inadmissible rounded profiles fail
  consistently across admission helpers. Scalar bounded-noise ciphertext
  multiplication now also preflights that fresh rounded capacity before
  operand or relinearization-key shape checks, matching the exact-RNS bounded
  multiply bridge, and bounded refresh-transcript validation applies the same
  preflight before bundle key-shape checks. Key-authorized bounded Bootstrap
  output-bound admission now also rejects too-narrow rounded profiles before
  bootstrap-key shape checks and shares the bootstrap round-count validator,
  and the exact residual-bound counterpart now rejects oversized input residual
  bounds or invalid refresh-round metadata before bootstrap-key shape checks.
  Exact and bounded multiply bound propagation now rejects oversized public
  input/output bounds before validating caller-supplied relinearization key
  material.
  Exact and bounded add bound propagation now validates supplied public input
  bounds before enforcing the minimum two-input shape, so oversized bound
  metadata cannot be hidden by an undersized input list.
  Exact and bounded plaintext-polynomial bound propagation now rejects
  oversized public input bounds before validating caller-supplied plaintext
  polynomial shape.
  Exact and bounded Galois key-switch bound propagation now also rejects
  oversized public input bounds before Galois-key shape checks.
  Exact/bounded packed `RotateLeft` bound propagation now also rejects
  oversized public input bounds or invalid rotation schedules before validating
  caller-supplied Galois key sets, and the exact, RNS, bounded-noise, and
  bounded basis-extension execution helpers now perform the same public
  schedule preflight before ciphertext or Galois-key shape checks.
  Exact/bounded outer-slot `RotateLeft` bound propagation now rejects oversized
  public input bounds or full-cycle rotations before validating
  caller-supplied rotation-key refresh ciphertexts.
  Exact/bounded public affine bound propagation now rejects oversized public
  input bounds before validating caller-supplied circuit row and coefficient
  shape, and exact, registered RNS, bounded RNS, and registered bounded affine
  execution helpers now validate public circuit metadata before malformed input
  ciphertext shapes.
  Exact, registered RNS, bounded-noise, direct RNS, and bounded basis-extension
  Galois key-switch execution helpers now validate public automorphism metadata
  and key-switch entries before malformed ciphertext shapes.
  Exact, registered RNS, bounded-noise, direct RNS, and registered bounded
  public scalar/plaintext-polynomial execution helpers now validate scalar
  ranges and plaintext coefficient metadata before malformed ciphertext shapes.
  Exact and bounded-noise seeded encryption, plus identifier envelope
  encryption, now validate public plaintext/input, non-empty non-all-zero
  deterministic seed, and identifier envelope metadata before malformed
  public-key shapes.
  Exact/bounded plaintext-scalar bound propagation now rejects oversized public
  input bounds before validating the public scalar range.
  Key-authorized bounded-noise bootstrap output-bound propagation now rejects
  oversized public input bounds or zero-round requests before validating full
  bootstrap-key ciphertext shape.
  Direct exact/bounded bootstrap refresh output-bound propagation now validates
  supplied public input bounds before rejecting zero-round requests, so
  oversized input-bound metadata cannot be hidden by invalid direct refresh
  counts.
  Bounded full-bootstrap linear-transform, raw-sample, and sample-switch bound
  propagation now preflights public artifact metadata before rounded-capacity
  errors while keeping full switch-key entry validation after the capacity
  gate.
  Direct no-artifact bounded full-bootstrap execution and bound helpers now
  preflight FullBootstrapV1 key/material metadata before rounded-capacity
  errors, and artifact-aware bounded full-bootstrap prefix execution/bound
  helpers share that key/material preflight before concrete artifact or
  ciphertext validation.
  Bounded raw-sample coefficient-zero repack and owner diagnostic helpers now
  reject malformed raw-sample metadata before rounded-capacity errors.
  Bounded raw-sample extraction and sample-switch execution helpers now do the
  same for sample/key metadata and key/sample consistency before inspecting
  ciphertexts or full switch-key entries.
  Bootstrap refresh execution now also validates public key metadata plus
  requested round index/count before full refresh-key ciphertext shape across
  scalar, bounded-noise, direct RNS, and registered RNS paths, so malformed
  `round_refreshes` vectors cannot mask out-of-capacity refresh requests.
  Packed `RotateLeft` execution helpers now also preflight Galois key-set
  public metadata and key-switch entries before ciphertext shape, while
  scalar/RNS key-switch primitives preflight full entry inventories before
  malformed switching components.
  Evaluation-key bundle validation and digest admission now preflight public
  rotation, Galois, and bootstrap inventory metadata before malformed
  relinearization or refresh/key-switch entry shapes.
  Relinearized ciphertext multiplication now preflights public
  relinearization-key digit and entry-polynomial inventories before malformed
  ciphertext operands across exact, registered RNS, bounded-noise, direct RNS,
  and bounded basis-extension paths.
  Direct exact/bounded refresh-transcript validation and digest admission now
  preflight transcript metadata, then advertised public-key shape, before
  evaluation-key bundle validation.
  Owner-side decrypt/profile/residual and bounded-noise diagnostics now validate
  ciphertext shape before secret-key shape, and exact/bounded rotation and
  bootstrap refresh-key generators validate public metadata, non-empty
  non-all-zero deterministic seeds, and public-key shape before deriving
  encrypted-zero refresh masks.
  Soracloud exact and bounded-noise multiply metadata wrappers now preflight
  declared public bounds before their own multiply-arity checks, so oversized
  single-input metadata reports the bound-capacity failure instead of a wrapper
  shape error.
  Soracloud FHE parameter-set admission now rejects non-BFV schemes and
  unregistered BFV backend labels at the shared data-model layer, and
  execution-policy admission now rejects unsupported deterministic rounding
  modes, so first-release BFV manifests cannot carry ignored scheme, backend,
  or rounding metadata.
  Exact and bounded Galois keygen now rejects invalid public automorphism powers
  and non-empty non-all-zero deterministic seed metadata before malformed
  secret-key shapes, and exact/bounded public-key consistency diagnostics reject
  malformed public keys before malformed secret keys. Bounded
  relinearization/Galois consistency
  diagnostics now also reject malformed public evaluation keys before malformed
  owner secrets, and bounded decrypt/profile/ciphertext diagnostics plus
  rotation, bootstrap, and bundle zero-refresh owner diagnostics reject
  too-narrow public rounded BFV profiles and oversized public rounded-noise
  bounds before malformed owner secrets. Exact residual-bound owner diagnostics
  now also reject oversized public residual bounds before malformed owner secrets
  while keeping ciphertext-shape preflight first. Exact bundle/rotation/bootstrap
  zero-refresh owner diagnostics now reject too-narrow public seeded-refresh
  residual profiles before malformed owner secrets while keeping refresh
  ciphertext-shape preflight first. Registered exact and bounded bootstrap
  refresh wrappers now also have round-index/count preflight coverage before
  malformed bootstrap-key or ciphertext shapes, and exact scalar/RNS bootstrap
  execution rejects too-narrow public seeded-refresh profiles before applying
  refresh masks. Exact and bounded direct and bundle refresh-transcript
  admission now preflights public capacity before malformed public-key,
  bundle-key, or refresh-ciphertext entry shapes. Scalar bounded bootstrap
  execution now rejects invalid public key-id and refresh-round requests before
  rounded-capacity failures. Bounded rotation/bootstrap refresh-key generation
  now rejects public step, key-id, round-count, and transcript seed metadata
  before rounded-capacity failures. Exact and bounded seeded keygen/encryption
  now reject public seed and plaintext metadata before exact residual or rounded
  capacity failures. Bounded Galois key generation now rejects public
  automorphism and seed metadata before rounded-capacity failures.
  Bounded Galois switch and packed `RotateLeft` execution wrappers now reject
  public Galois-key metadata, rotation schedules, and key-set metadata before
  rounded-capacity failures. Bounded outer `RotateLeft` execution wrappers now
  reject public rotation metadata before rounded-capacity failures. Bounded
  affine execution wrappers now reject public circuit metadata before
  caller-supplied RNS/capacity corridor failures. Key-authorized exact and
  bounded bootstrap bound propagation now rejects public bootstrap key-id and
  round-count metadata before caller input-bound failures while preserving full
  refresh-key shape validation after public bound checks. Bounded
  plaintext scalar and polynomial execution wrappers now reject public
  scalar/plaintext metadata before rounded-capacity failures. Bounded scalar and
  plaintext-polynomial bound propagation now rejects invalid public
  scalar/plaintext metadata before rounded-capacity failures while preserving
  oversized input-bound precedence on otherwise valid profiles. Bounded
  ciphertext multiplication bound propagation now rejects invalid public
  relinearization-key metadata before rounded-capacity failures while preserving
  oversized input-bound precedence on otherwise valid profiles. Bounded Galois
  key-switch and packed `RotateLeft` bound propagation now reject invalid public
  Galois metadata, rotation schedules, and key-set metadata before
  rounded-capacity failures while preserving oversized input-bound precedence on
  otherwise valid profiles. Bounded affine, outer `RotateLeft`, and bootstrap
  refresh bound propagation now rejects invalid public circuit, rotation,
  round-count, and bootstrap-key-id metadata before rounded-capacity failures
  while preserving the existing valid-profile precedence: oversized input bounds
  remain first for affine, outer-slot, and direct bootstrap bounds, and
  key-authorized bootstrap bounds keep key-id/round metadata ahead of full
  bootstrap-key shape.
  Soracloud BFV refresh-transcript admission now also derives its deterministic
  seed, bootstrap key-id, rotation-transcript, and bootstrap max-round caps from
  the public `iroha_crypto` constants.
  Bounded-noise FHE input-admission envelopes now receive bound-capacity,
  statement-hash, shared `OpenVerifyEnvelope` admission-shape, active-verifier,
  and native binding metadata preflight, then fail closed because no dedicated
  ciphertext-witness AIR exists. They do not persist bounded metadata. The
  data-model proof validator
  also rejects exact and bounded-noise input-admission bounds that exceed
  registered RAM-LFE BFV capacity before runtime admission, and persisted FHE
  state rows now reject exact or bounded bound metadata that exceeds the same
  registered capacity. FHE input-admission proof attachments now also require
  `vk_ref.name` to be the canonical v1 circuit id, a supported STARK/FRI v1
  proof backend label from the shared data-model ZK classifier, a decoded STARK
  `OpenVerifyEnvelope` with the canonical v1 circuit/schema, a
  v1 STARK public-input wrapper whose single public input matches the proof
  `statement_hash`, a `vk_commitment` that matches the embedded
  `OpenVerifyEnvelope.vk_hash`, and an `envelope_hash` that matches the embedded
  `OpenVerifyEnvelope` bytes at both data-model validation and Soracloud
  runtime admission; the Core attachment helper now applies the shared
  structural guard before decoding the envelope, and core runtime admission and
  backend pre-verification now also reject matching but unsupported STARK/FRI
  backend labels and portable but non-canonical FHE circuit ids before
  verifier-record lookup. Data-model validation and Core runtime preverification
  now also share Soracloud-specific byte caps for the encoded `OpenVerify`
  envelope, STARK public-input wrapper, and backend-native STARK envelope bytes,
  so proof-carrying ciphertext admission cannot alias the verifier id,
  omit/forge the verifier-key, statement, circuit, or envelope binding, or push
  unbounded proof bytes toward verifier lookup.
  The backend verifier now decodes the
  `OpenVerifyEnvelope` from the attachment proof bytes itself, then re-checks
  the STARK envelope shape, public-input schema, statement public input,
  verifier-id and attachment bindings, plus the single supported v1 verifier
  record version, before verifier lookup, so direct verifier use cannot bypass
  the envelope or statement-hash preflight. Data-model validation, Core
  envelope validation, and backend preverification now also reject empty
  backend-native STARK `envelope_bytes`, so admission cannot carry only
  statement metadata without a native proof envelope. Data-model validation and
  Core runtime admission now also share the exported Soracloud
  `OpenVerifyEnvelope` bounds helper, keeping outer envelope, STARK wrapper,
  canonical circuit/schema ceilings, and auxiliary-byte policy in one place
  before verifier lookup. The Core FHE input-admission verifier helper now also
  recomputes the actual payload length and payload commitment before BFV shape
  checks, statement-hash derivation, envelope validation, or verifier lookup, so
  direct helper use cannot bypass mutation-executor payload metadata binding.
  Core input-admission regressions now also mutate the proven bound value and
  bound mode independently, proving both public bound fields are statement-bound
  before verifier lookup.
  FHE job execution admission now computes deterministic
  output payload-size projections with checked `u64` arithmetic and rejects
  overflow before comparing the projection with `max_ciphertext_bytes`; the
  legacy infallible projection helper remains conservative by returning
  `u64::MAX` for unrepresentable projections. Direct service-state upserts and
  FHE job output persistence now share checked binding state-total projection,
  so inconsistent existing-item accounting and `u64` total overflows fail
  closed before max-total admission checks. Centered target-limb RNS basis
  extension now preserves signed raw-product representatives in narrower target
  limbs while keeping the canonical nonnegative digit path separate, and
  target-limb scale-round bridge helpers now carry signed products and
  two-product cross-term sums into the deterministic `t/q` rounded BFV
  boundary. Direct and registered bounded-noise target-limb multiplication now
  derive a role-separated centered scale-round source chain before key-switch
  decomposition, and full-bootstrap circuit material, evaluator artifact-set
  summaries, native generated circuit bodies, native prover/verifier payloads,
  native proof-circuit fingerprints, proof-key payloads/envelopes, proof-key
  pair commitments, release-audit proof profiles, release-audit proof-key
  evidence records, release-audit evidence, release-audit signoff payloads, and
  release-audit manifests now bind that source-chain digest; caller-pinned
  package digest alias checks also reject reusing the signed source-chain digest
  as the package digest. Soracloud material/execution public-input schemas
  advertise the matching field counts, source-chain binding flags, external
  audit signed-commitment distinctness, and caller-pinned signed-commitment
  package-digest alias rejection, plus package audit-body requirements for the
  signed evidence, artifact-bundle, evaluator-artifact-set, centered
  source-chain, generated-body, native-fingerprint, proof-key-pair, prover-key,
  and verifier-key commitments. The
  production bounded-noise
  admission circuit/prover rollout, broader target-limb BFV-RNS evaluator
  hardening, and audited full-bootstrap proof-producing/verifier artifacts
  remain pending.
  Owner-side
  evaluated-output diagnostics can now validate ciphertexts against
  caller-declared exact residual-multiple bounds and reject plaintext-preserving
  residual inflation, with checked helper APIs deriving exact add-output and
  public bootstrap refresh-output bounds before those diagnostics run; the same
  helper surface now covers exact subtract, plaintext addition,
  plaintext-scalar multiplication, plaintext-polynomial multiplication, and
  public affine-circuit row propagation. Outer ciphertext-slot `RotateLeft`
  now also propagates rotated per-slot bounds and one public encrypted-zero
  refresh bound per output slot. Packed `RotateLeft` now also propagates
  conservative exact bounds through the current Galois key-switch bridge,
  plaintext-mask products, and schedule addition, with capacity rejection for
  too-narrow profiles. Soracloud FHE state rows carry optional exact
  residual-multiple metadata, and deterministic evaluator helpers propagate
  Add, balanced Multiply/relinearization, outer/packed `RotateLeft`, and
  Bootstrap bounds while rejecting missing or over-capacity inputs. Ledger
  persistence remains fail-closed at the proof-witness boundary; the exact
  packed `RotateLeft` evaluator regression decrypts the scheduled output and
  asserts its conservative residual bound. Client FHE state mutations without
  proof-carrying input admission remain metadata-free and cannot feed FHE jobs.
  Proof-carrying Upsert mutations have a canonical Soracloud FHE
  input-admission statement, provenance binding, STARK/FRI verifier-key lookup,
  canonical V1 circuit binding, restored verifier metadata drift checks,
  ciphertext-shape validation, registered identifier slot-cap enforcement, and
  residual-capacity preflight. Binding-only proofs then fail closed before Core
  persists residual metadata; FHE job input loading continues to apply the same
  slot cap to any persisted row before execution.
  Persisted FHE rows with public bound metadata must now explicitly advertise
  exact-residual or bounded-noise semantics, and bound-only legacy/corrupt rows
  fail closed before execution.
  Public production circuit and governed key-material
  rollout for that noise-admission proof remains part of the pending BFV-RNS
  engine.
  BFV key generation now self-checks freshly generated public keys by
  verifying that `b + a*s` is a plaintext-modulus multiple within that bound,
  and checks generated relinearization entries before returning key material;
  secret-key admission now rejects non-ternary coefficients outside the
  generated `{0, 1, q - 1}` domain before owner-side residual diagnostics run;
  public-key owner diagnostics reject shape-valid wrong-secret,
  non-plaintext-multiple, or oversized residuals before publication, while
  public refresh-transcript admission now rejects empty or unbounded seed
  inventories, zero/duplicate rotation steps, noncanonical bootstrap key-id
  metadata, and zero/over-budget bootstrap round metadata before recomputation
  and digest comparison. Public-key proof statement digests now bind the
  parameter set, public key, and public-key digest under exact-lift and
  bounded-noise domains, and Soracloud refresh-transcript helpers derive the
  same statements while `SoracloudFhePublicKeyProofV1` validates the canonical
  `soracloud_fhe_public_key_v1` STARK/OpenVerify envelope, schema hash, and
  public-input shape for verifier-backed proof handoff. Core policy-bound
  admission now requires and signs public-key proof attachments in FHE job
  provenance, derives the expected public-key statement from the refresh
  transcript, and validates active Soracloud verifier records before rejecting
  binding-only AIR at the missing key-generation witness AIR boundary.
  Shared FHE execution-policy validation, production FHE governance-bundle
  admission, Core FHE job admission, and Torii signed FHE job preflight now
  require `public_key_proof_statement_digest`; Core and Torii derive the same
  statement from the signed refresh transcript before admission, and the
  canonical Soracloud FHE execution-policy/governance-bundle fixtures carry
  that digest so deployed governance profiles force runtime public-key
  statement binding for policy-bound key material admission. Torii's signed
  FHE job preflight and `RunSoracloudFheJob` now require `public_key_proof`,
  include it in the canonical job provenance payload, and validate proof
  envelopes against the policy-bound statement hash before Core verifier-backed
  admission.
  Plaintext, ciphertext, polynomial, Galois-power, affine-circuit, and
  RNS-polynomial shape validators now use the same parameter preflight before
  inspecting caller-controlled shapes. BFV parameter admission now also
  requires enough ciphertext-modulus headroom to keep the deterministic
  `+t`/`-t` error representatives distinct under the configured error bound.
  Key-switch decomposition digit counting now validates parameters and uses
  checked coverage arithmetic, so malformed profiles cannot silently saturate
  relinearization/Galois digit generation; multiply and key-switch residual
  admission also uses checked `t - 1` and decomposition-base-minus-one bounds.
  Secret-key diagnostics now also expose the exact centered residual multiples
  and remaining centered-modulus headroom for the current plaintext-lift
  evaluator, while full bounded-RLWE noise budgeting remains part of the
  pending BFV-RNS engine.
  Outer ciphertext-slot `RotateLeft` now also
  rejects empty slot lists and full-cycle step counts before applying
  rotation-key refresh material, and exact, registered RNS, bounded-noise, and
  bounded RNS execution helpers preflight that public metadata before refresh
  key, caller-supplied RNS corridor, or slot ciphertext shapes; key-owner
  zero-refresh diagnostics now also keep malformed ciphertext shapes ahead of
  capacity while failing too-narrow profiles before inert all-zero refresh
  material. Packed `RotateLeft` execution helpers now also
  reject invalid public rotation schedules before ciphertext or Galois-key
  shape checks across exact, RNS, bounded-noise, and bounded basis-extension
  paths, and now preflight Galois key-set public metadata and key-switch entries
  before ciphertext shape, while scalar/RNS key-switch primitives preflight full
  entry inventories before malformed switching components.
  Evaluation-key bundle validation and digest admission now preflight public
  rotation, Galois, and bootstrap inventory metadata before malformed
  relinearization or refresh/key-switch entry shapes.
  Relinearized ciphertext multiplication now preflights public
  relinearization-key digit and entry-polynomial inventories before malformed
  ciphertext operands across exact, registered RNS, bounded-noise, direct RNS,
  and bounded basis-extension paths.
  Direct exact/bounded refresh-transcript validation and digest admission now
  preflight transcript metadata, then advertised public-key shape, before
  evaluation-key bundle validation.
  Galois key-switch execution helpers now reject invalid public
  automorphism metadata and malformed key-switch entries before ciphertext shape
  checks across exact, RNS, bounded-noise, registered, and basis-extension paths.
  Public affine execution
  helpers now likewise reject invalid row or
  coefficient metadata before input ciphertext shape checks across exact,
  registered RNS, bounded RNS, and registered bounded paths. Public
  scalar/plaintext-polynomial execution helpers now reject invalid scalar ranges
  or plaintext coefficient metadata before ciphertext shape checks across exact,
  RNS, bounded-noise, and registered bounded paths. Seeded exact/bounded
  encryption and identifier envelope encryption now reject invalid public
  plaintext/input, deterministic seed, and identifier envelope metadata before
  public-key shape checks. Owner-side decrypt/profile/residual and
  bounded-noise diagnostics now validate ciphertext shape before secret-key
  shape, and exact/bounded rotation and bootstrap refresh-key generators
  validate public metadata, deterministic seeds, and public-key shape before
  deriving encrypted-zero refresh masks. Soracloud Multiply
  now uses a deterministic
  balanced ciphertext tree and rejects jobs whose declared multiplication depth
  underestimates that tree during job-spec validation and again before
  ciphertext evaluation through the same crypto planner, whose operation
  constructors and budget validation now also reject zero-input plans,
  single-input nonzero-depth plans, single-input Add plans, multi-input
  RotateLeft plans, and zero-round or non-single-input Bootstrap plans; the
  planner rejects zero-round Bootstrap metadata before input-shape errors and
  over-budget depth/refresh metadata before secondary operation-shape checks.
  Soracloud Bootstrap job-spec validation and runtime planner admission now
  also reject zero `bootstrap_count` metadata before non-single-input shape
  errors, and Add, Multiply, RotateLeft, and Bootstrap operation metadata is
	  rejected before secondary arity/input-shape errors across manifest
	  validation and runtime planner admission.
	  Soracloud FHE parameter sets and execution policies now also reject
	  advertised multiplication/bootstrap budgets above the exact evaluator budget
	  before governance admission. Soracloud decryption requests and ciphertext
	  query responses now also reject zero-prehash digest sentinels across
	  ciphertext commitments, optional consent evidence, governance linkage,
	  state-key digests, query hashes, and inclusion proof leaves/anchors before
	  private-state admission or query evidence can be trusted. Service
	  deployment, app-infra service/audit, service audit, runtime/Inrou runtime
		  state, service-state governance linkage, mailbox, and runtime-receipt
		  records now reject zero-prehash digest sentinels across manifest,
		  governance, mailbox, receipt, placement, and artifact hashes before those
			  records can be trusted. Container bundle hashes, service/agent
			  container-manifest references, service artifact references, and HF
			  placement identifiers/seeds now reject zero-prehash digest sentinels
			  before deployment, artifact, or placement evidence is admitted.
			  Training job records/audit events, HF source records,
			  shared-lease pool/member/audit records, and
			  model-host violation evidence now reject zero-prehash digest
			  sentinels across metric, source, normalized-runtime, pool,
			  placement, evidence, and slash hashes before training, lease, or
			  host-violation evidence is trusted. Agent apartment records and
			  audit events now reject zero-prehash manifest, mailbox payload,
			  autonomy request/result, runtime receipt, journal, and checkpoint
			  hashes, and apartment records canonical-check embedded manifest and
			  mailbox payload hashes against their preimages before accepting
			  authoritative agent state.
			  Agent apartment runtime records and audit events now reject
			  zero-prehash digest sentinels across manifest, mailbox payload,
			  autonomy request, execution result, runtime receipt, journal, and
			  checkpoint hashes before agent runtime or audit evidence is trusted.
			  Soracloud host request/response envelopes now validate nested
			  operation-specific payloads, operation/payload pairing, host paths,
			  found/payload consistency, delete mutation payload absence, and
			  payload/body hash preimages, while Core host syscalls run that
			  request validation before runtime fallback.
			  Bootstrap fixtures now pin both the zero refresh and each round-indexed
			  public refresh ciphertext. Full bootstrapping circuit/key-material
	  commitments now have a Rust admission/digest/proof-statement surface,
  artifact-aware execution path, verifier-backed Core fixtures, and a
  data-model material proof envelope, with manifest-level generated-circuit
  body digest binding in release-audit packages, while audited prover/verifier
  artifact vectors remain an open release item.

