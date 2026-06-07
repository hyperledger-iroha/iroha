# Engineering Backlog (Detailed Open Work)

Last updated: 2026-06-07

The public roadmap lives in [`../../roadmap.md`](../../roadmap.md). Completed
history lives in [`../../status.md`](../../status.md). This file should only
track detailed unfinished engineering work.

## SCCP launch-scope note

- Substrate/Polkadot-family networks are not supported in the current SCCP
  launch scope, including Kusama, Polkadot, SORA Kusama, SORA Polkadot, and
  SORA2. Existing Substrate-family runtime wrappers, evidence helpers, and
  relay notes are diagnostic/backlog material only; they should not be treated
  as remaining release blockers or advertised as production network support
  unless governance explicitly re-opens that scope.

Current ISO 20022 operator tooling already versions digest-bound XSD, canary,
trust-bundle, and receipt-verifier summaries and rejects missing or unsupported
versions in evidence and production-readiness gates. Schema-critical integer
metadata such as versions, receipt status codes, and notary record counts reject
JSON boolean aliases before evidence can be archived, and bounded child-process
output byte caps reject boolean aliases before verifier subprocesses run.
Regular-file and rail payload byte caps now also reject boolean or non-integer
aliases before filesystem metadata is inspected.
Archived child-command evidence rejects value-taking flags whose separate or
equals-form values are empty or another flag token, keeping canary command
evidence unambiguous before production archiving.
Direct ISO CLI path preflights now also treat missing, empty, following
`--flag`, or `--path-flag=--flag` path values as missing before any file or
network work.
Live rail-gateway `--torii-base-url` and audit-notary `--endpoint` flags now
also reject missing, empty, or flag-looking URL values before argparse parsing.
Direct ISO numeric CLI preflights now reject malformed, empty, flag-looking, or
secret-looking numeric values before argparse can echo operator-provided input.
All ISO operator entry points now also reject secret-looking raw CLI tokens
before argparse can echo unknown arguments; the scanner covers bearer tokens,
private keys, passwords/passphrases, API/access/session keys, client secrets,
cookies, and Iroha signatures.
Secret scanning now also checks repeated percent-decoded forms, so encoded or
double-encoded secret-looking key/value material is rejected in CLI paths,
unknown JSON keys, recursive JSON values, compact summary paths, and remote
response previews/errors without echoing the decoded material.
ISO URL path validators now also reject secret-looking key/value material in
literal, percent-encoded, or double-encoded path segments before live network
delivery, archived evidence ingestion, or readiness rollup.
Local path, raw CLI, summary-path, artifact-path, and URL-path validators now
also reject narrow identifier-style secret path material such as
`token-*-secret` and strong key markers without treating ordinary token-file
operator paths as secret-bearing by name alone.
ISO URL port parser failures now report only label-level invalid-port
diagnostics instead of including parser exception text that may contain the raw
operator-provided port string.
ISO URL host validators now reject secret-looking hostname labels, and non-port
URL parser failures use label-only diagnostics before malformed URL text can be
echoed by parser exceptions.
XSD profile-catalog validation now recursively rejects secret-looking strings
and identifier-style values before rail, signature-policy, reference-dataset,
address-mode, profile-id, or version diagnostics can echo catalog-provided
values.
XSD manifest schema and fixture `payload_root` values now reject secret-looking
material before namespace/root mismatch diagnostics can echo manifest-provided
payload names.
Checked-in XSD `targetNamespace` attributes now also reject secret-looking
material before schema namespace mismatch diagnostics can echo schema-provided
attribute values.
XSD and XML payload identifiers, schema-root attribute names, and unsupported
foreign child namespaces now use label-only secret-looking diagnostics instead
of echoing schema-provided names or namespace URIs.
XML fixture contents are scanned before optional `xmllint` validation, and
secret-looking validator output is redacted before it can be reflected in
XSD preflight diagnostics.
Secret-looking field-name markers now also normalize hyphenated
`private-key` and underscore-form `x_iroha_signature` spellings across ISO
validators, and receipt JSON secret-field checks recurse through nested objects
and arrays before receipt semantics are evaluated.
All ISO JSON duplicate-key hooks now report only that a duplicate key exists,
without echoing the repeated key name.
Secret-looking unknown JSON field names are also rejected with label-only
unknown-key diagnostics while ordinary unknown-field typos still list the
field names for operator ergonomics.
Direct ISO boolean CLI flags reject attached `--flag=value` spellings and
separate non-option values before argparse can echo the value or reinterpret
the option.
Evidence and production-readiness context flags reject missing, empty,
flag-looking, or secret-looking provider/environment values before argparse,
summary loading, or mismatch diagnostics can reflect them.
Canary runbook provider/environment labels now reject secret-looking
identifier-style strings before plan-only output or executed summaries can
preserve them.
Trust-bundle `--max-source-age-days` now rejects missing, empty, flag-looking,
malformed, or secret-looking freshness budgets before argparse or bundle reads.
Trust-bundle profile IDs, rails, environments, embedded signature policies,
source authority/version strings, DER labels, and recursively scanned field
names reject secret-looking identifiers before trust summaries or profile
overrides can persist them.
Trust-bundle SHA-256 pins, declared DER digests, and certificate policy OIDs
also reject secret-looking marker strings before canonical SHA/OID diagnostics.
Archived evidence and readiness rollups apply the same no-echo identifier check
to compact canary provider/environment fields, evidence policy context, trust
profile IDs/rails/environments, trust embedded-signature policies,
profile-override policies, trust source authority/version strings, and archived
trust DER labels before release summaries can preserve those values.
Archived evidence and readiness SHA-256 fields, including trust bundle digests,
profile-override pins, and receipt payload/anchor/index digests, reject the same
markers before digest-shape diagnostics or blockers can preserve them.
Rail sidecar `profile` and `rail_message_id` identifiers, plus archived rail
receipt `profile` and `rail_message_id` values, now reject secret-looking
identifier-style strings before network delivery, receipt emission, receipt
verification, or receipt-summary rollup.
Rail sidecar `message_type` and `payload_sha256` values, and archived rail
receipt `message_type` values, also apply no-echo secret-looking checks before
unsupported-type, digest-mismatch, or receipt-summary diagnostics can preserve
operator-provided marker strings.
Receipt verifier, evidence, and readiness `receipt_kind` values reject
secret-looking identifier-style markers before unsupported-kind diagnostics or
blockers can preserve forged archive values.
Archived canary stage names in evidence and readiness rollups also reject
secret-looking identifier-style markers before unsupported-stage, ordering, or
stage-window diagnostics can preserve forged values.
The live rail-gateway, audit-notary, canary, and XSD fixture tools also reject
secret-looking key/value material in local output paths before those paths can
be persisted into receipts or archived summaries.
The receipt verifier scans raw receipt strings for secret-looking material
before version or receipt-kind dispatch, so malformed receipt kinds cannot echo
runtime tokens in unsupported-kind diagnostics.
Recursive trust-bundle, receipt, evidence, and readiness secret-material
scanners now report label-only forbidden-field failures, and receipt value
secret checks no longer echo the receipt field name that carried the rejected
material; those recursive scanners use the same expanded secret marker set for
secret-looking field names and values.
Rail and notary adapters also redact failed remote response previews and
receipt errors when upstreams return token, password, private-key, or cookie
material, and the receipt verifier rejects archived previews/errors containing
the same marker set.
Live rail sidecars now run the same recursive secret-material scan on known
fields before unsupported message type, profile, payload digest, or
rail-message-id validation can echo operator-provided values.
Duplicate record, list, digest, OID, archived receipt-reuse, and trust-material
diagnostics now report field/index labels without echoing the rejected duplicate
value.
Rail-gateway and audit-notary bearer-token file failures now also report the
credential input label instead of echoing runtime token file paths.
Remaining production work still depends on operator-supplied live rail evidence,
redistributable schemas, and official trust/revocation bundles.

## FHE/RAM-LFE first-release follow-ups

- Replace the current deterministic plaintext-modulus-multiple BFV-shaped
  evaluator with the full BFV-RNS engine planned for release: bounded RLWE
  noise, RNS modulus chains, real relinearization, packed-slot Galois-key
  switching, and full BFV bootstrapping. The current pass makes Torii/Soracloud consume and
  persist real ciphertext envelopes, evaluates `SelectEqZero` correctly over
  all byte values in the `F_257` RAM-LFE profile, and keeps evaluators
  secret-key free. BFV key generation, relinearization-key generation, and
  encryption now use deterministic error polynomials sampled from
  `{0, t, -t}` modulo the ciphertext modulus so exact coefficient-wise
  plaintext decoding remains stable while zero-error ciphertexts are no longer
  emitted. Parameter validation now also requires enough ciphertext-modulus
  headroom to keep the configured positive and negative plaintext-multiple
  error representatives distinct. Secret-key diagnostics now expose the exact
  centered residual multiples and remaining centered-modulus headroom for the
  current plaintext-lift evaluator, without treating that diagnostic as a full
  bounded-RLWE noise budget. BFV key generation now self-checks freshly
  generated public keys by verifying that `b + a*s` is a plaintext-modulus
  multiple within the current exact evaluator error bound, and checks generated
  relinearization entries against scaled `s^2` residues before returning key
  material. Soracloud RotateLeft now requires public
  rotation-key refresh material for the outer ciphertext-slot envelope, and Bootstrap
  applies validated, domain-separated public encrypted-zero refresh material
  by round index instead of reusing one refresh ciphertext. Key-owner
  diagnostics now also verify that generated rotation and bootstrap public
  refresh ciphertexts decrypt to zero under the matching secret key, including
  a bundle-level check over every rotation and bootstrap refresh mask, and
  public bootstrap admission now requires a verifier-backed statement proof
  envelope.
  Public deterministic transcript checks now recompute rotation and bootstrap
  encrypted-zero refresh material from the advertised seed, public key, key id,
  and round count, rejecting wrong-seed, key-id-drifted, or tampered refresh
  ciphertexts without requiring a secret key; the same check now runs at the
  evaluation-key bundle level so admission cannot accidentally validate only a
  subset of public rotation/bootstrap refresh masks. The validated transcript
  inventory now also has nonzero duplicate-free rotation step metadata,
  public seed metadata bounded by the shared BFV deterministic seed cap,
  canonical bootstrap key-id metadata bounded by the shared BFV bootstrap key
  cap, and rotation inventory metadata bounded by the shared BFV evaluation-key
  rotation cap plus bounded nonzero bootstrap refresh round metadata and a
  stable
  domain-separated digest over the parameter set, public key, evaluation-key
  digest, and transcript metadata, giving governance/admission code a
  canonical value to bind in the bootstrap-key proof envelope. The crypto layer
  now also exposes exact-lift and bounded-noise transcript-bound
  bootstrap-key zero-refresh proof statement digests that bind parameters,
  public key, evaluation-key digest, refresh-transcript digest, bootstrap
  transcript seed/key id/round capacity, and every public refresh ciphertext
  under mode-separated domains. `RunSoracloudFheJob` now carries an
  optional bootstrap-key proof attachment, provenance signs it, and Core
  requires it for bootstrap execution while checking the policy-bound
  statement hash against an active Soracloud STARK verifier record or
  preverified proof cache entry. The verifier registry now rejects canonical
  Soracloud bootstrap verifier records whose registry id, namespace, circuit
  version, public-input schema hash, gas schedule, or active inline key
  material drift from the governed v1 profile, moving those rollout failures
  to `RegisterVerifyingKey`/`UpdateVerifyingKey` admission. BFV bootstrap keys
  now carry an explicit `RefreshOnlyV1` mode, and reserved full-bootstrap mode
  fails closed until real bootstrapping circuit material exists, so the current
  refresh bridge cannot be mislabeled as full bootstrapping. Bundle
  validation/digesting applies the same mode gate before transcript-bound
  bootstrap proof statements can be produced. Remaining production work is the
  full BFV bootstrapping path. Direct crypto
  refresh-transcript validation/digesting and Soracloud transcript digesting
  now also preflight the advertised BFV public-key shape
  before evaluation-key bundle validation, so malformed transcript key material
  cannot be masked by unrelated bundle-shape errors. The lower-level crypto
  bundle validator now enforces the same public metadata preflight for direct
  callers, and relinearized multiply execution now rejects malformed public
  relinearization digit inventories before malformed ciphertext operands
  across exact, RNS, bounded-noise, and bounded basis-extension paths while
  keeping full entry-polynomial validation after operand-shape checks.
  Standalone refresh-key transcript
  generators/validators reject the same empty or oversized public seed
  metadata before deriving or recomputing encrypted-zero masks. Soracloud FHE
  execution policies now carry the refresh-transcript inventory digest,
  `RunSoracloudFheJob` signs the transcript inventory in the provenance
  payload, and core rejects jobs whose supplied refresh transcript is
  unbounded or does not match the governance-bound digest. This hardens the
  current refresh path while the full BFV bootstrapping engine remains open.
  Bundle-level owner diagnostics now also verify that relinearization entries
  decrypt to scaled `s^2` residues and Galois entries decrypt to scaled
  automorphed-secret residues under the matching secret key, with key-switch
  residuals constrained to the current plaintext-multiple error bound;
  standalone Galois key generation now applies that same residual self-check
  before returning generated key-switch material. Rotation and bootstrap
  encrypted-zero refresh diagnostics also reject zero-plaintext masks whose
  residual multiples exceed the deterministic `(2n + 1)E` refresh bound for
  the first-release seeded encryption format. The bounded-noise counterparts
  now also reject zero-plaintext rotation/bootstrap refresh masks whose
  centered rounded noise exceeds the fresh BFV noise bound, and bundle-level
  bounded diagnostics now identify indexed rotation/bootstrap refresh masks
  when nonzero plaintext or oversized rounded noise is detected.
  Public-key owner diagnostics now also reject shape-valid wrong-secret,
  non-plaintext-multiple, or oversized residuals before publication, while
  future public admission still needs proof-carrying key-material checks.
  Seeded key generation and public-key encryption now also fail closed unless
  the parameter set's centered `q/t` capacity covers the same deterministic
  encrypted-zero refresh bound, so structurally valid but too-narrow profiles
  cannot produce first-release ciphertext/key material; deterministic BFV
  keygen, encryption, Galois-key generation, and identifier seed helpers now
  also reject empty or oversized seeds before deriving RNG material. Registered
  BFV profile validation and the production digest path now enforce the same capacity
  invariant before admitting the RAM-LFE profile, and BFV parameter validation
  uses checked exact-arithmetic products for raw and plaintext-scaled scalar
  accumulator bounds rather than relying on saturating overflow guards. The
  key-switch decomposition digit count now also validates parameters and uses
  checked coverage arithmetic, so invalid or future-widened profiles fail with
  `BfvError` instead of silently saturating digit generation; BFV residual-bound
  helpers likewise use checked `t - 1` and decomposition-base-minus-one bounds
  instead of saturating those admission inputs. Identifier envelope slot counts
  now also use checked max-input-plus-length-slot arithmetic instead of
  saturating the reserved length-slot calculation. The crypto crate now also
  has a separate rounded BFV path for the pending BFV-RNS replacement:
  bounded-noise public-key generation samples small centered error, plaintext
  is encoded as `(q / t) * m`, decryption rounds back into `Z_t`, and owner
  diagnostics report centered noise/headroom against the rounded-decoding
  capacity. Rounded
  ciphertext addition now also has conservative centered-noise bound
  propagation tested against real rounded ciphertext addition; subtract,
  rounded plaintext-scalar addition, plaintext-scalar multiplication, and
  plaintext-polynomial multiplication have the same bounded-noise propagation
  coverage. Rounded ciphertext-ciphertext multiplication now has a scalar
  semantic bridge that computes centered raw products before `t/q`
  scale-and-rounding, then relinearizes with bounded-noise key-switch entries
  and validates a conservative output noise budget. Rounded Galois key
  switching now also has small-noise key generation, secret-key consistency
  checks, automorphism application, and output-bound propagation over rounded
  ciphertexts. Rounded packed `RotateLeft` now wires that bounded-noise Galois
  path through the public packed-selector schedule with matching output-bound
  propagation. This still needs Soracloud evaluator migration/broader
  propagation and full bootstrapping before Soracloud can leave the exact-lift
  bridge.
  RNS polynomials can now be exactly basis-extended between validated modulus
  chains by canonical CRT reconstruction plus target-limb reduction, with
  target-product coverage checks to reject aliasing; this is a deterministic
  reconstructable bridge alongside the target-limb key-switch path rather than
  the final approximate basis-extension algorithm. A deterministic target-limb
  basis-extension helper now computes the CRT quotient correction exactly with
  integer arithmetic and reduces source representatives into target limbs
  without requiring the target product to cover the source product; narrow
  target reconstruction remains visibly lossy. Key-switch components now also
  decompose directly into RNS digit polynomials, exact RNS key switching
  consumes those digit polynomials internally, and basis-extended digits are
  rejected if they no longer reconstruct to canonical decomposition digits. An
  explicit target-limb basis-extension key-switch path now decomposes in a
  source chain, verifies that the source cannot alias decomposition digits,
  basis-extends canonical key-switch digits through the digit-specific
  basis-extension helper without requiring the evaluator target to cover the
  full source-chain product, rejects basis-extended digit-count and RNS
  limb-shape drift at validation, and drives rounded multiplication, Galois,
  and packed `RotateLeft` bridges while matching the scalar bounded-noise
  outputs. Direct key-switch component decomposition and digit
  basis-extension helpers now enforce source/target decomposition-base
  coverage before malformed polynomial shapes can mask the public chain
  descriptor failure.
  Rounded ciphertext multiplication now also has an RNS exact raw-product
  bridge that decomposes ciphertext components as centered residues,
  reconstructs signed negacyclic products before `t/q` scale-and-rounding, and
  relinearizes the scaled quadratic component through the RNS digit/key-switch
  path while matching the scalar bounded-noise multiplication output. Rounded
  Galois key switching and packed `RotateLeft` now also have RNS exact bridge
  entry points that match the scalar bounded-noise schedule and reject
  too-narrow chains. Outer-slot rotation and bootstrap refresh material can now
  also be generated and publicly transcript-validated with rounded
  bounded-noise encrypted-zero ciphertexts, refreshed through scalar or exact
  RNS addition, and propagated with centered-noise output bounds. Evaluation-key
  bundles can now validate and digest the bounded-noise rotation/bootstrap
  transcript inventory under a separate domain from the exact-lift refresh
  path, and owner diagnostics can validate bounded relin/Galois key-switch
  residuals with bundle-owned relinearization labels and bundle-indexed Galois
  diagnostics plus every bounded refresh mask in one bundle check. Soracloud FHE
  execution policies now bind the refresh transcript mode, data-model digesting
  routes through exact-lift or bounded-noise transcript derivation explicitly,
  and core runtime admission rejects mode/digest mismatches before job
  execution. Soracloud bounded-noise jobs now dispatch to the bounded-noise RNS
  bridge for Add, outer `RotateLeft`, and encrypted-zero Bootstrap refresh
  when policy/input metadata are explicitly bounded, while Multiply and packed
  `RotateLeft` now call registered `iroha_crypto` helper entry points that
  select the smallest registered key-switch decomposition prefix inside the
  crypto layer before invoking the target-limb basis-extension bridge. The
  crypto layer now exposes that registered decomposition chain and a
  role-separated digest so runtime/admission code can share the same canonical
  target-limb key-switch source basis; registered helper entry points for
  bounded-noise Multiply, Galois key switching, and packed `RotateLeft` now
  derive both the canonical evaluator chain and source basis inside
  `iroha_crypto` before invoking the target-limb bridge, so runtime callers no
  longer pass evaluator RNS chains into those registered bounded-noise entry
  points. The explicit basis-extension key-switch path now also rejects
  decomposition source chains that are not evaluator-chain prefixes, while the
  lower-level target-limb residue conversion primitive remains available for
  checked RNS arithmetic. Soracloud FHE
  parameter-set governance now stores that digest beside the parameter and
  evaluator RNS-chain digests, and input admission statement hashes bind the
  key-switch decomposition-chain digest so proof-carrying ciphertext admission
  cannot drift onto a different decomposition basis. Soracloud registered
  bounded-noise runtime coverage now also exercises two-round Bootstrap through
  the registered RNS refresh bridge,
  decrypts refreshed multi-slot outputs, and checks the propagated
  key-authorized centered-noise bound at the core runtime boundary; the same
  bounded wrapper coverage now pins Multiply and packed `RotateLeft` propagated
  output bounds while decrypting the registered target-limb outputs, and
  ledger-level `RunSoracloudFheJob` coverage now persists bounded Multiply,
  packed `RotateLeft`, and two-round Bootstrap output rows with the expected
  bound mode, bound value, payload commitment, and decrypted plaintext. The crypto
  layer now owns scalar and exact-RNS multi-round Bootstrap refresh helpers for
  exact and bounded-noise ciphertexts, rejects zero or over-capacity refresh
  counts before applying any round, and single-round scalar/RNS refresh helpers
  now preflight the requested round index before entering ciphertext addition.
  Soracloud routes exact and
  bounded-noise Bootstrap jobs plus shared operation-vector checks through
  those helpers. Registered exact and bounded-noise Add/Subtract, exact and
  bounded-noise Multiply, exact and bounded-noise plaintext-polynomial
  selector products, exact and bounded-noise affine row evaluators, exact and
  bounded-noise packed `RotateLeft`, outer-slot `RotateLeft`, and
  round-zero, indexed-round, and consecutive-round Bootstrap refresh helper
  entry points now derive the canonical evaluator RNS chain inside
  `iroha_crypto`,
  and Soracloud exact and bounded-noise runtime dispatch uses those helpers
  instead of passing the chain through core. The registered-helper rejection
  regression now also covers the decomposition-chain helpers plus exact and
  bounded-noise Subtract, exact and bounded-noise plaintext-polynomial
  selector products, exact and bounded-noise affine row evaluators, exact and
  bounded-noise Bootstrap refresh forms, and the bounded target-limb Multiply,
  Galois, and packed `RotateLeft` entry points,
  proving structurally valid but unregistered profiles fail closed before
  caller-supplied key material is inspected.
  Direct exact-RNS bounded-noise Add/Subtract, affine-row, outer-slot
  `RotateLeft`, and Bootstrap refresh helpers now share a rounded-decoding plus
  exact-addition RNS corridor preflight before supplied-chain accumulation,
  refresh-key checks, or ciphertext-shape checks, and the direct exact-RNS
  bounded-noise Multiply, Galois key-switch, and packed `RotateLeft` fallback
  helpers now also have registered production wrappers, so both
  exact-reconstruction and target-limb basis-extension paths derive canonical
  evaluator chains before inspecting caller-controlled key material.
  Bounded-noise RNS packed-selector products now also route through a bounded
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
  before bounded-noise capacity checks, so structurally valid but unregistered
  profiles fail with the production registration error first. Public
  bounded-noise output-bound propagation now also preflights fresh rounded BFV
  noise capacity before public arithmetic, key-switch, affine, rotation, or
  bootstrap bound math, so profiles that cannot admit a fresh bounded-noise
  ciphertext fail closed consistently across admission helpers. Scalar
  bounded-noise ciphertext multiplication now shares that fresh-capacity
  preflight before operand or relinearization-key shape checks, matching the
  exact-RNS bounded multiply bridge, and bounded refresh-transcript validation
  now applies the same preflight before bundle key-shape checks. Key-authorized
  bounded bootstrap output-bound admission now also rejects too-narrow rounded
  profiles before bootstrap-key shape checks and shares the bootstrap
  round-count validator, and the exact residual-bound counterpart now rejects
  oversized input residual bounds or invalid refresh-round metadata before
  bootstrap-key shape checks. The key-authorized bounded-noise bootstrap
  output-bound helper now also rejects oversized public input bounds or
  zero-round requests before validating full bootstrap-key ciphertext shape.
  Direct exact and bounded bootstrap refresh output-bound helpers now validate
  supplied public input bounds before rejecting zero-round requests, so
  oversized input-bound metadata cannot be hidden by invalid direct refresh
  counts.
  Exact and bounded multiply bound propagation now
  rejects oversized public input/output bounds before validating
  caller-supplied relinearization key material. Soracloud exact and
  bounded-noise multiply metadata wrappers now preserve that preflight before
  their own multiply-arity checks, so oversized single-input metadata cannot be
  hidden by wrapper shape errors. Soracloud FHE parameter-set admission now
  rejects non-BFV schemes and unregistered BFV backend labels at the shared
  data-model layer, and execution-policy admission now rejects unsupported
  deterministic rounding modes, so first-release BFV manifests cannot carry
  ignored scheme, backend, or rounding metadata. Exact and bounded Galois
  keygen now rejects invalid public automorphism powers and deterministic seed
  metadata before malformed secret-key shapes, and exact/bounded public-key
  consistency diagnostics reject malformed public keys before malformed secret
  keys. Bounded relinearization/Galois consistency diagnostics now also reject
  malformed public evaluation keys before malformed owner secrets, and bounded
  decrypt/profile/ciphertext diagnostics plus rotation, bootstrap, and bundle
  zero-refresh owner diagnostics reject too-narrow public rounded BFV profiles
  and oversized public rounded-noise bounds before malformed owner secrets.
  Exact and bounded add bound propagation now
  validates supplied public input bounds before enforcing the minimum two-input
  shape, so oversized bound metadata
  cannot be hidden by an undersized input list. Exact residual-bound owner
  diagnostics now also reject oversized public residual bounds before malformed
  owner secrets while keeping ciphertext-shape preflight first. Exact
  bundle/rotation/bootstrap zero-refresh owner diagnostics now reject too-narrow
  public seeded-refresh residual profiles before malformed owner secrets while
  keeping refresh ciphertext-shape preflight first. Registered exact and bounded
  bootstrap refresh wrappers now also have round-index/count preflight coverage
  before malformed bootstrap-key or ciphertext shapes, and exact scalar/RNS
  bootstrap execution rejects too-narrow public seeded-refresh profiles before
  applying refresh masks. Exact and bounded direct and bundle refresh-transcript
  admission now preflights public capacity before malformed public-key,
  bundle-key, or refresh-ciphertext entry shapes. Scalar bounded bootstrap
  execution now rejects invalid public key-id and refresh-round requests before
  rounded-capacity failures. Bounded rotation/bootstrap refresh-key generation
  now rejects public step, key-id, round-count, and transcript seed metadata
  before rounded-capacity failures. Exact and bounded seeded keygen/encryption
  now reject public seed and plaintext metadata before exact residual or rounded
  capacity failures. Bounded Galois key generation now rejects public
  automorphism and seed metadata before rounded-capacity failures. Exact and
  bounded plaintext-polynomial bound propagation now rejects oversized public
  input bounds before validating
  caller-supplied plaintext polynomial shape. Exact and bounded Galois
  key-switch bound propagation now also rejects oversized public input bounds
  before Galois-key shape checks, and exact/bounded packed `RotateLeft` bound
  propagation rejects oversized public input bounds or invalid rotation
  schedules before validating caller-supplied Galois key sets. Bounded Galois
  switch and packed `RotateLeft` execution wrappers now reject public
  Galois-key metadata, rotation schedules, and key-set metadata before
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
  Packed `RotateLeft` execution helpers now also preflight Galois key-set public
  metadata before ciphertext shape while keeping full key-switch entry
  validation after ciphertext shape. Evaluation-key bundle validation and
  digest admission now preflight public rotation, Galois, and bootstrap
  inventory metadata before malformed relinearization or refresh/key-switch
  entry shapes. Exact/bounded
  outer-slot `RotateLeft` bound propagation now rejects oversized public input
  bounds or full-cycle rotations before validating caller-supplied rotation-key
  refresh ciphertexts, and exact/bounded public affine bound propagation now
  rejects oversized public input bounds before validating caller-supplied
  circuit row and coefficient shape; exact, registered RNS, bounded RNS, and
  registered bounded affine execution helpers now validate public circuit rows
  and coefficients before parsing malformed input ciphertext shapes. Exact,
  registered RNS, bounded-noise, direct RNS, and bounded basis-extension Galois
  key-switch execution helpers now validate public automorphism metadata before
  parsing malformed ciphertext shapes. Exact,
  registered RNS, bounded-noise, direct RNS, and registered bounded public
  scalar/plaintext-polynomial execution helpers now validate scalar ranges and
  plaintext coefficient metadata before parsing malformed ciphertext shapes.
  Exact and bounded-noise seeded encryption, plus identifier envelope
  encryption, now validate public plaintext/input, deterministic seed, and
  identifier envelope metadata before malformed public-key shapes.
  Exact/bounded plaintext-scalar bound propagation now rejects oversized public
  input bounds before validating the public scalar range. Bootstrap
  refresh execution now also validates public key metadata plus requested round
  index/count before full refresh-key ciphertext shape across scalar,
  bounded-noise, direct RNS, and registered RNS paths, so malformed
  `round_refreshes` vectors cannot mask out-of-capacity refresh requests.
  Owner-side decrypt/profile/residual and bounded-noise diagnostics now validate
  ciphertext shape before secret-key shape, and exact/bounded rotation and
  bootstrap refresh-key generators validate public metadata, deterministic
  seeds, and public-key shape before deriving encrypted-zero refresh masks.
  Soracloud BFV
  refresh-transcript admission now also
  derives its deterministic seed, bootstrap key-id, rotation-transcript, and
  bootstrap max-round caps from the public `iroha_crypto` constants.
  Verifier-backed bounded-noise FHE input-admission envelopes now persist
  bounded metadata after statement-hash, shared `OpenVerifyEnvelope`
  admission-shape, active-verifier, and backend proof checks; portable proof
  validation now rejects cheap attachment metadata before BFV bound capacity
  (backend consistency, canonical verifier id, verifier-key commitment
  metadata, and envelope-hash presence), while retaining BFV bound-capacity
  rejection before decoded `OpenVerifyEnvelope` admission, expensive verifier
  dispatch, and verifier-record lookup. The data-model proof validator also
  rejects exact and bounded-noise input-admission bounds that exceed registered
  RAM-LFE BFV capacity before runtime admission, and persisted FHE state rows
  now reject exact or bounded bound metadata that exceeds the same registered
  capacity.
  FHE input-admission proof attachments now also require `vk_ref.name` to be the
  canonical v1 circuit id, a supported STARK/FRI v1 proof backend label from
  the shared data-model ZK classifier, a decoded STARK `OpenVerifyEnvelope` with
  the canonical v1 circuit/schema, a v1 STARK public-input wrapper whose single
  public input matches the proof `statement_hash`, a `vk_commitment` that
  matches the embedded `OpenVerifyEnvelope.vk_hash`, and an `envelope_hash`
  that matches the embedded `OpenVerifyEnvelope` bytes at both data-model
  validation and Soracloud runtime admission; the Core attachment helper now
  applies the shared structural guard before decoding the envelope, and core
  runtime admission and backend pre-verification now also reject matching but
  unsupported STARK/FRI backend labels and portable but non-canonical FHE
  circuit ids before verifier-record lookup, so proof-carrying ciphertext
  admission cannot alias the verifier id or omit/forge the verifier-key,
  statement, circuit, or envelope binding. The
  backend verifier now decodes the `OpenVerifyEnvelope` from the attachment
  proof bytes itself, then re-checks the STARK envelope shape, public-input
  schema, statement public input, verifier-id and attachment bindings, plus the
  single supported v1 verifier record version, before verifier lookup, so
  direct verifier use cannot bypass the envelope or statement-hash preflight.
  The data-model validator, Core envelope helper, and backend preverification
  path now also reject STARK wrappers whose backend-native `envelope_bytes` are
  empty, so proof-carrying FHE admission cannot reach verifier lookup with only
  statement metadata and no native proof envelope. Those same data-model and
  Core preverification paths now share Soracloud-specific byte caps for the
  encoded `OpenVerify` envelope, STARK public-input wrapper, and backend-native
  STARK envelope bytes through the exported data-model bounds helper before
  verifier lookup, with Soracloud-sized canonical circuit/schema ceilings, so
  outer envelope, STARK wrapper, canonical metadata, and auxiliary-byte policy
  cannot drift between portable validation and runtime admission. The Core FHE
  input-admission verifier helper now also recomputes the actual payload length
  and payload commitment before BFV shape checks, statement-hash derivation,
  envelope validation, or verifier lookup, so direct helper use cannot bypass
  the same payload metadata binding performed by the mutation executor.
  FHE job execution admission now computes deterministic output payload-size
  projections with checked `u64` arithmetic and rejects output-size overflow
  before comparing the projection with `max_ciphertext_bytes`; the legacy
  infallible projection helper remains conservative by returning `u64::MAX`
  for unrepresentable projections. Direct service-state upserts and FHE job
  output persistence now share checked binding state-total projection, so
  inconsistent existing-item accounting and `u64` total overflows fail closed
  before max-total admission checks.
  The production
  bounded-noise admission circuit/prover
  rollout, broader target-limb BFV-RNS evaluator hardening, and full
  bootstrapping circuit/key material remain pending.
  Registered RNS chain selection now also preflights exact-addition and exact
  negacyclic-product coverage before exposing the chain or its production digest. Public RNS
  exact evaluator entry points now also preflight their required chain coverage
  before late operation-specific checks, while indexed Bootstrap helpers now
  preflight the requested round capacity before malformed ciphertext shapes can
  enter the addition path; malformed RNS context is still rejected before
  invalid refresh rounds, no-op packed rotations, or key-switch scheduling can
  short-circuit validation. Bounded exact-RNS ciphertext
  multiplication now reuses the same exact evaluator-chain preflight, including
  exact-addition coverage, before operand or relinearization-key shape checks.
  Refresh transcript digest assembly
  now also returns structured shape errors for missing or unmatched rotation
  transcript seeds instead of relying on a post-validation panic invariant.
  Owner-side evaluated-output
  diagnostics can now validate a ciphertext against a caller-declared exact
  residual-multiple bound and reject plaintext-preserving residual inflation,
  while checked helper APIs derive exact add-output and public bootstrap
  refresh-output residual bounds before those diagnostics run. Those helpers
  now also cover exact subtract, plaintext addition, plaintext-scalar
  multiplication, plaintext-polynomial multiplication, and public affine-circuit
  row bounds. Outer ciphertext-slot `RotateLeft` now also propagates rotated
  per-slot bounds and one public encrypted-zero refresh bound per output slot.
  Packed `RotateLeft` now also has conservative exact-bound propagation for the
  current Galois key-switch bridge and plaintext-mask schedule, including
  capacity rejection for parameter profiles whose centered modulus cannot cover
  key-switch residuals. Soracloud service-state rows now carry optional exact
  BFV residual-multiple metadata for FHE ciphertexts, and `RunSoracloudFheJob`
  persists propagated bounds for Add, balanced Multiply/relinearization,
  outer/packed `RotateLeft`, and Bootstrap outputs while rejecting
  missing or over-capacity input bounds before execution. The exact packed
  `RotateLeft` runtime regression now decrypts the scheduled packed output and
  asserts the persisted conservative residual bound. Client-provided FHE state
  mutations without proof-carrying input admission intentionally remain
  metadata-free and cannot feed FHE jobs. Upsert mutations may now carry a
  canonical Soracloud FHE input-admission proof attachment: provenance signs
  the proof statement, core derives the statement from the service, binding,
  key, operation, payload, BFV profile, RNS chain, key-switch decomposition
  chain, and governance transaction, validates the STARK/FRI
  `OpenVerifyEnvelope` against an active `soracloud`
  verifier key for the canonical V1 circuit id, rejects restored verifier
  records whose Goldilocks field label or inline key length drift from the
  stored key material, and persists the claimed residual bound only after the
  envelope, ciphertext shape, registered identifier slot cap, and residual
  capacity checks pass. The
  production circuit and governed key-material rollout for public noise
  admission remains open, so this is the ledger admission boundary rather than
  a complete BFV-RNS proof system.
  BFV evaluation-key metadata now caps rotation-key and Galois key bundles,
  rejects duplicate Galois automorphism powers, and requires portable bounded
  bootstrap key ids containing only ASCII alphanumeric, `.`, `_`, or `-` bytes.
  The crypto layer now also exposes and validates a
  registered RAM-LFE v1 BFV RNS coefficient-modulus chain with bounded,
  strictly increasing odd-prime, NTT-friendly, pairwise-coprime limbs and a
  checked product that covers the current ciphertext modulus, plus a stable
  domain-separated chain digest for governance and release-vector binding. The
  shared RNS validator now also validates the BFV parameter set itself, so
  direct exact-lift and exact `Z_q` coverage checks fail closed on malformed
  parameter profiles before inspecting chain arithmetic bounds. The same chain
  now supports checked limb-major polynomial decomposition and CRT
  reconstruction, rejecting malformed limb counts, limb lengths, unreduced
  residues, and source coefficients outside the ciphertext modulus; it also
  has deterministic scalar residue addition and per-limb NTT-backed
  negacyclic multiplication with a scalar fallback in `Z_Q[x] / (x^n + 1)`.
  The shared Soracloud operation fixture now binds the registered RNS
  descriptor/digest plus sample
  decomposition/reconstruction, residue addition, and negacyclic
  multiplication hashes across Rust and lightweight SDK shape checks. The RNS
  chain now also exposes guarded exact ciphertext-modulus polynomial addition
  and negacyclic multiplication for sufficiently wide chains, plus exact
  RNS-backed ciphertext addition, multiplication, relinearization, and Galois
  key-switch bridges that match the scalar evaluator on small wide-chain
  profiles. The registered RAM-LFE chain is now wide enough for that guarded
  exact `Z_q` bridge, so Rust exercises exact RNS ciphertext addition,
  multiplication/relinearization, and Galois key-switching against the
  production RAM-LFE parameters while still rejecting the narrower exact-lift
  compatibility corridor. The programmed RAM-LFE BFV runtime now uses that
  registered exact RNS bridge for ciphertext add, subtract,
  multiply/relinearization, and `SelectEqZero` exponentiation/selection
  arithmetic; plaintext-scalar operations remain scalar because they do not
  require RNS polynomial products. The public Soracloud BFV operation executor
  now uses the same registered exact RNS bridge for Add, Multiply, packed and
  outer `RotateLeft`, and bounded Bootstrap refresh rounds, so the shared
  operation vectors cover the production job path rather than scalar-only
  fallbacks. The deterministic BFV baseline now also has
  packed-polynomial Galois automorphism keys that switch `sigma_k(s)`
  ciphertexts back to the original secret key after applying `x -> x^k`, with
  regressions covering canonical odd powers, malformed key rejection,
  plaintext automorphism parity, and registered-chain exact-RNS parity. The
  shared Soracloud fixture now binds a canonical
  Galois key-switching bundle shape, SDK-visible component hashes, a scalar
  Galois switch output vector, and a packed Galois slot-permutation execution
  vector backed by deterministic packed plaintext CRT slot encoding/decoding,
  with scalar and exact-RNS key-switch primitives now validating
  decomposition-entry counts and operand shapes before zipping digits so
  malformed key material cannot silently truncate a switch,
  plus bounded one-/two-round bootstrap refresh vectors that consume distinct
  per-round public refresh ciphertexts. Rust crypto/core now
  also support arbitrary non-zero packed `RotateLeft` requests by deriving a
  deterministic public Galois-key mask schedule, applying each required
  automorphism, masking contributed slots, and summing the masked ciphertexts.
  The raw packed `RotateLeft` helpers now validate the complete supplied
  Galois-key slice for bounds, duplicates, and malformed entries before
  looking up scheduled powers, so extra bad key material cannot be silently
  ignored outside an evaluation-key bundle.
  Shared BFV key validators now also validate the parameter set before
  inspecting secret, public, rotation, relinearization, Galois, key-switch
  entry, or bootstrap key shapes, so direct validator use cannot bypass
  malformed parameter rejection or reach decomposition math first. Plaintext,
  ciphertext, polynomial, Galois-power, affine-circuit, and RNS-polynomial
  validators now apply the same parameter preflight before inspecting
  caller-controlled shapes. Bootstrap-key validation now also checks the
  declared round-refresh count before inspecting refresh ciphertext shapes, so
  malformed public refresh material cannot mask missing per-round bootstrap
  inventory.
  The outer ciphertext-slot `RotateLeft` helper now also rejects empty slot
  lists and full-cycle step counts before applying rotation-key refresh
  material, and the exact, registered RNS, bounded-noise, and bounded RNS
  execution helpers perform that public metadata preflight before inspecting
  refresh-key or slot ciphertext shapes. Packed `RotateLeft` execution helpers
  now likewise derive the public rotation schedule before inspecting
  caller-supplied ciphertexts or Galois-key sets across exact, RNS,
  bounded-noise, and bounded basis-extension paths. This keeps no-op rotations
  fail-closed before key material is parsed.
  The exact BFV bridge now exposes reusable first-release evaluation budget
  planners that reject zero-input plans, single-input nonzero-depth plans,
  single-input Add plans, multi-input RotateLeft plans, and zero-round
  or non-single-input Bootstrap plans; the planner now also rejects zero-round
  Bootstrap metadata before input-shape errors and over-budget depth/refresh
  metadata before secondary operation-shape checks. Soracloud Bootstrap
  job-spec validation and runtime planner admission now also reject zero
  `bootstrap_count` metadata before non-single-input shape errors, and Add,
  Multiply, RotateLeft, and Bootstrap operation metadata is rejected before
  secondary arity/input-shape errors across manifest validation and runtime
  planner admission. Soracloud multi-input Multiply executes as a deterministic balanced tree, rejects jobs whose declared
  multiplication depth underestimates that tree at job-spec validation and
  runtime admission through the same crypto planner, and parameter-set /
  execution-policy validation rejects advertised multiplication/bootstrap
  budgets above the exact evaluator budget before governance admission. The
  shared operation fixture pins each runtime vector's requested depth across
  Rust and SDK shape checks.
  These deterministic `t`-multiple error terms, refresh paths, modulus-chain
  descriptors, residue arithmetic helpers, and packed rotation schedules are
  still not a complete bounded-noise BFV-RNS evaluator or full bootstrap
  circuit.
- Broaden the cross-SDK deterministic BFV-RNS vector corridor: Kotlin, Java,
  Swift, and JavaScript now require `RamLfeOutputOpening` on identifier
  claim/resolve helpers, and a shared Soracloud BFV identifier-envelope fixture
  now covers the baseline encrypted identifier plus three-input Add and
  Multiply operand payloads with deterministic plaintext-modulus-multiple BFV
  error terms in the Rust, JavaScript, Swift, Kotlin/JVM, and Java Android
  envelope builders. The same fixture now pins Rust executor output
  lengths, SHA-256 digests, and plaintext slots for Soracloud Add, Multiply,
  RotateLeft, and Bootstrap operation vectors, as well as deterministic public
  key/public-parameter byte lengths and SHA-256 digests, evaluation-key bundle
  byte length, SHA-256 digest, domain-separated digest, decomposition metadata,
  relinearization entry count, per-relinearization-entry `b`/`a`
  coefficient-vector digests, Galois key count, Galois automorphism powers,
  per-Galois-entry `b`/`a` coefficient-vector digests, rotation key count,
  bootstrap key id, bootstrap key max refresh rounds, rotation encrypted-zero
  refresh digests, bootstrap zero-refresh and per-round encrypted-zero refresh
  digests, and refresh `c0`/`c1` coefficient-vector digests. The fixture now
  also pins a
  scalar Galois switch vector with deterministic input/output ciphertext and
  plaintext coefficient digests, plus a packed Galois switch vector with input
  slots, the induced slot permutation, output slots, packed plaintext
  coefficient digest, ciphertext digests, and output component digests, plus a
  runtime packed `RotateLeft` vector for the registered half-slot rotation
  bound to Galois automorphism power `65`, plus a one-step runtime packed
  `RotateLeft` vector that pins the full BFV Galois mask-and-sum schedule,
  expected packed slot rotation, ciphertext digests, plaintext coefficient
  digest, requested multiplication-depth metadata, and output component
  digests across the same SDK fixture-shape validators, plus bounded bootstrap
  refresh vectors with key-aware refresh-round admission, refresh rounds,
  deterministic input/output ciphertext digests, plaintext coefficient digests,
  and output component digests. The same shared operation fixture now pins the
  registered RNS chain descriptor/digest, deterministic sample coefficients,
  per-limb residue hashes, and reconstructed hashes for RNS decomposition,
  addition, and negacyclic multiplication; Rust recomputes those fields from
  the registered chain, while JavaScript, Swift, Kotlin/JVM, and Java Android
  validate the descriptor and residue-hash shape. JavaScript, Swift,
  Kotlin/JVM, and Java Android now also parse `norito_length_encoding =
  compact-v1` and reproduce the Rust-compatible compact operation-input
  encryption stream for the non-packed Soracloud Add, Multiply, outer
  `RotateLeft`, and Bootstrap input vectors. Packed-slot operation inputs still
  rely on Rust execution plus SDK fixture-shape and digest validators outside
  the browser/native identifier-envelope builders. JavaScript, Swift,
  Kotlin/JVM, and Java Android now validate those component-vector fields from
  the shared fixture and carry adversarial fixture mutations for missing,
  noncanonical-case, duplicate, zeroed, coefficient-count-drifted, and
  key-count-drifted component metadata; the JavaScript lane also carries
  adversarial RNS mutations for missing, duplicate, zeroed, count-drifted, and
  malformed metadata. A shared
  signed/proof-attestation identifier receipt fixture now pins canonical payload
  bytes, Iroha prehash, resolver signature, signed/proof attestation bytes, and
  adversarial receipt/policy mutations across the Rust data model, JavaScript,
  Swift, Kotlin/JVM, Java Android, and Torii runtime claim-receipt signing path.
  The Soracloud FHE governance fixtures now bind the canonical parameter set,
  execution policy, governance bundle, and job spec to the registered
  `bfv-default` RAM-LFE BFV runtime descriptor and reject descriptor drift in
  core admission. Parameter-set descriptors now also carry the canonical
  domain-separated registered BFV RNS modulus-chain digest, and core admission
  rejects RNS descriptor drift before FHE jobs can run. The execution policy now
  also carries the canonical evaluation-key bundle digest from the shared
  operation fixture, and `RunSoracloudFheJob` rejects structurally valid but
  ungoverned key material before output state is emitted. Shared release
  vectors still need to cover the full BFV-RNS evaluator and full
  bootstrapping circuit/key material beyond the current encrypted-zero
  round-refresh bundles.
- Broaden validation from the green focused crypto/data-model/core/Torii/daemon
  checks into the next full workspace and SDK corridor. The `iroha_cli
  --all-targets` strict clippy gate now covers the governance-instruction, IVM
  contract deploy, and Taikai helper targets after the previously failing
  length/time arithmetic paths were made warning-clean. The `iroha_crypto
  --all-targets` strict clippy gate is also green after the SoraNet
  token/handshake and RAM-LFE test-target warning blockers were cleared. The
  non-default GOST, SM, forced-NEON SM, SM OpenSSL provider, Rayon-backed
  Merkle, secp256k1 MSM-batch, BLS multi-pairing, FFI export, and crypto
  parity-test feature corridors now also pass strict `iroha_crypto
  --all-targets` clippy and focused library tests, with SM acceleration and
  OpenSSL preview tests serialized around their test-only runtime dispatch
  overrides. The combined `iroha_crypto --all-features` all-targets clippy,
  library, and integration-test corridors are also green after keeping the BFV
  adversarial evaluation-key metadata coverage below strict test-target line
  limits and serializing forced-NEON SM acceleration tests around their shared
  runtime override state; the all-features pass fixed SM dispatch precedence so
  `sm-neon-force` force-enables only the `Auto` policy and explicit
  `force-disable` still pins the scalar fallback. The
  `iroha_data_model --all-targets` strict clippy gate is green after clearing
  the Kagemusha/ZK-ACE test/bench lint surface, and the touched-package
  all-target gate for `iroha_data_model`, `connect_norito_bridge`,
  `iroha_js_host`, `iroha_kagami`, and `sorafs_orchestrator` now also passes
  with `--no-deps`. The full `soranet-relay` strict clippy gate now reaches and
  passes relay diagnostics without `--no-deps`. Focused
  adversarial tests now cover malformed/truncated ciphertext envelopes,
  hidden-program shape/overflow rejection,
  replayed/tampered/future/expired/wrong-verifier openings,
  receipt-signing/backend mismatch refusal, adversarial BFV public parameters
  and evaluation-key metadata, execution-policy evaluation-key digest
  mismatches, unregistered BFV parameter sets,
  impossible decrypted identifier envelopes, FHE governance lifecycle/linkage
  abuse, operation-shape and budget-smuggling jobs, encrypted-only Torii DTO
  rejection, duplicate JSON encrypted/opening-field and nested shadow-field
  rejection before DTO decoding,
  full receipt/opening security-binding mutation checks, proof-only receipt
  attestations passed to Rust/JavaScript/JVM SDK signature verifiers,
  wrong resolver keys, mismatched receipt policy ids, validly re-signed but
  execution-mismatched output openings on `ClaimIdentifier`, missing/malformed
  Soracloud evaluation keys, empty/malformed ciphertext slots, malformed
  relinearization keys, structurally valid wrong BFV key-bundle component
  material, malformed SDK ciphertext hex, plaintext-only policy misuse,
  slot-count/digest mismatches, shared signed receipt canonical payload
  drift, shared signed/proof attestation canonical byte drift, malformed
  signatures, wrong resolver keys, wrong policy ids, tampered output ciphertext
  hashes, proof-only attestations, ZK-ACE public-input version drift, and
  ZK-ACE prepared authorization proofs rebound to a different transfer digest,
  chain id, receiver, amount, or policy hash across the Rust data-model, JS,
  Swift, Kotlin/JVM, Java Android, and Torii runtime fixture corridor, plus
  core ZK-ACE rotated/revoked identity state, unsupported action classes,
  transaction digest/account substitution, and mutated ZK-ACE/STARK public
  inputs;
  RAM-LFE proof-verifier metadata now rejects noncanonical backend/circuit
  identifiers, zero schema hashes, empty/all-zero verifier keys, and oversized
  verifier keys before proof-carrying programmed policies are admitted;
  crypto identifier-envelope public-parameter validation now rejects
  structurally valid but unregistered BFV profiles before identifier
  encryption, decryption, or downstream Torii/core admission, caps
  `max_input_bytes` at the registered 63-byte/64-slot RAM-LFE identifier
  profile across Rust, JS, Swift, Kotlin/JVM, and Java Android clients, and identifier
  slot encoding now reports byte-length and slot-index conversion failures
  through `BfvError` instead of panic-only assumptions; always-built BFV scalar
  modular addition, multiplication, and coefficient reduction now avoid
  post-reduction `expect` conversions while preserving max-width `u64::MAX`
  modulus behavior, and the RAM-LFE default programmed BFV hidden program now
  uses profile-sized `u16` constants instead of runtime `usize`-to-`u16`
  conversion assumptions; programmed BFV memory RNG transcript derivation now
  binds `u64` step values directly instead of converting through a panic-only
  `expect`; BFV/RAM-LFE domain-separated digest, receipt, and RNG-seed
  transcripts now stream hash chunks directly while preserving the previous
  contiguous byte layout; BFV `RotateLeft` outer-slot step normalization now also uses `u64`
  modulo arithmetic before converting back to `usize`, avoiding
  target-width-dependent behavior for large public rotation-key step counts;
  programmed RAM-LFE BFV hidden-program admission now caps v1 instruction tapes
  at the canonical 64-slot, four-instruction shape before execution and rejects
  `LoadInput` indexes that exceed the encrypted envelope's advertised
  `max_input_bytes`; `LoadConst`, `AddPlain`, `SubPlain`, and `MulPlain`
  immediates must also be canonical `F_257` values before public-program
  digests or programmed parameters are admitted; the
  feature-gated BFV acceleration selector now falls back to deterministic scalar
  schoolbook multiplication for zero or overflowed derived
  convolution lengths, and the CRT-NTT helper path now rejects invalid operand
  lengths, unsupported NTT lengths, and CRT reconstruction overflow before
  using that same fallback instead of panicking on degree or NTT arithmetic;
  programmed RAM-LFE BFV bundle construction now keeps only fallible production
  constructors that reject unregistered identifier profiles and invalid proof
  metadata before public-parameter digests are emitted, while programmed BFV
  public-parameter decoding rejects encrypted-envelope capacities above the
  canonical profile slot count;
  programmed BFV public-parameter admission now rejects zero hidden-program
  digests and relinearization-only violations where unused rotation/bootstrap
  refresh keys are smuggled into identifier-program metadata;
  BFV evaluation-key metadata now rejects noncanonical, delimiter-shaped, or
  oversized bootstrap key ids and oversized rotation-key bundles before
  key-bundle digests are admitted;
  generic RAM-LFE and identifier receipt proof verifiers now have focused
  pre-parse regressions for public-input schema drift and non-zero mismatched
  verifier-key hashes;
  secp256k1 recoverable prehash signing now normalizes low-S output and the
  public-key recovery primitive rejects high-S malleable encodings before
  deriving EVM addresses;
  Ed25519 uncached batch verification now rejects noncanonical or small-order
  signature `R` encodings before entering the dalek batch backend, and direct
  byte-key/preparsed batch APIs now filter exact verify-cache hits before
  signature parsing and backend setup; the thread-local exact verify-ok cache
  now keeps two entries per exact slot to reduce collision churn for 32-byte
  transaction-hash verification tuples without returning to a process-wide
  cache;
  SoraNet relay handshake frame length-prefix writes now use a checked helper
  plus a compile-time `u16` maximum-frame assertion, so oversized relay hellos
  fail as `FrameTooLarge` instead of relying on a narrowing assertion;
  SoraNet constant-rate scheduler dequeue now handles unexpected empty queues
  explicitly and falls through to the dummy-cell path instead of using
  panic-only queue-pop assertions;
  ML-DSA public-key reconstruction from private-key material now has a
  fallible API, and `KeyPair::from_private_key` uses it so length-valid but
  internally inconsistent ML-DSA secrets return `KeyGen` instead of panicking;
  ML-DSA seeded-keygen HKDF expansion now propagates `Error::KeyGen` through
  the existing `Result` path instead of relying on a panic-only assertion, and
  its S2 nonce offset conversion now uses the same `Error::KeyGen` route
  instead of a const-conversion `expect`;
  GOST deterministic nonce generation now feeds the domain tag, private scalar,
  message scalar, and optional extra entropy into HMAC-Streebog as separate
  components and streams the HMAC inner hash directly while preserving the
  previous contiguous seed transcript; Ed25519 and secp256k1 now expose checked
  `try_keypair` paths, and top-level
  `KeyPair::try_random_with_algorithm` routes OS-backed Ed25519 seed bytes and
  secp256k1 candidate scalar bytes through `OsRng::try_fill_bytes` so
  entropy-source failures or bounded scalar-sampling exhaustion surface as
  `Error::KeyGen` instead of the infallible compatibility RNG adapter;
  standalone X25519 key exchange now exposes `KeyExchangeScheme::try_keypair`,
  draws OS-backed private-key bytes through `OsRng::try_fill_bytes`, and routes
  P2P, native Connect bridge, and Python Connect keypair generation through
  fallible error surfaces instead of the infallible compatibility adapter;
  Connect Norito bridge C/Java keypair-from-seed helpers and the Swift parity
  regeneration utility now use `KeyPair::try_from_seed`, returning existing
  bridge/key-derivation errors instead of panic-only seed expansion;
  GOST random scalar sampling and per-signature extra entropy now also use
  checked OS fills, while both BLS backends derive random keys from checked OS
  seed material and the default w3f backend seeds its key-splitting/signing RNGs
  only after checked OS fills, leaving the compatibility `os_rng()` adapter
  test-only; P2P SoraNet runtime handshakes now seed their local `StdRng`
  through `SeedableRng::try_from_os_rng` and surface entropy-source failures as
  `HandshakeSoranet` instead of panicking; Taikai ingest-edge drift jitter now
  keeps explicit seeds deterministic while routing unseeded `StdRng` setup
  through `SeedableRng::try_from_os_rng` and the CLI `Result` path, and CEK
  rotation receipt HKDF salts now use direct checked OS RNG fills when an
  explicit `--hkdf-salt` is not supplied; Kagami keypair, PoP, client-config,
  genesis-signing including NPoS bootstrap escrow, wizard, and localnet
  peer/genesis/gas/extra-account key generation now route
  random, seeded, and private-key-derived material through `KeyPair`'s fallible
  APIs and BLS PoP `Result`s instead of compatibility panic
  wrappers; irohad's ephemeral Torii receipt-signer fallback now uses checked
  secp256k1 key generation and surfaces entropy/keygen failures as `StartTorii`,
  while `iroha_swarm` peer/genesis key generation, seeded network material, and
  BLS PoP proving now return `Error::KeyGeneration` through `Swarm::new`
  instead of panicking; the CLI offline fallback config and governance council
  VRF candidate-account derivation now use `KeyPair::try_from_seed`, surfacing
  config/candidate derivation errors through existing `Result` paths, and
  Izanami workload, Nexus gas, NPoS validator, post-topology, and network-builder
  key material now uses `KeyPair::try_random` / `KeyPair::try_from_seed` with
  explicit `Result` propagation instead of panic-only `KeyPair` wrappers;
  `MultisigRegister::from_spec` now also returns `Result` and generates its
  temporary registration anchor account through checked default key generation;
  the transaction-gossip frame-cap probe now uses a fixed checked Ed25519 seed
  instead of drawing a runtime dummy key;
  Private Kaigi fee-spend execution now derives its synthetic fee-payer account
  through checked Ed25519 seed expansion from the action hash; SoraFS hybrid
  KEM derived material now binds the recipient public keys and encapsulated
  public transcript components through length-prefixed HKDF input with checked
  capacity accounting, and SoraNet session-key HKDF extraction now
  domain-separates and length-prefixes IKM components before expansion, with
  NK2/NK3 interop vectors refreshed under both checked-in fixture bundles;
  SoraNet deterministic SHAKE expansion now also frames its domain, label, part
  count, and every absorbed component before deriving deterministic KEM,
  simulated ML-DSA, dual-mix, or Noise-seed material, with checked-in fixture
  bundles regenerated from the framed outputs;
  `PublicKey::try_to_*` and `ExposedPrivateKey::try_to_*` now expose fallible
  public/private key formatting, public-key Norito serialization now routes
  full-to-compact conversion through a checked payload extractor, and
  `PublicKey::to_prefixed_string` now reuses the malformed compact-key marker
  instead of unwrapping invalid internal key state, while `ExposedPrivateKey`
  display and prefixed compatibility formatting now return a non-secret
  invalid-private-key marker instead of unwrapping checked private-key
  formatting; `Signature::try_new` now routes SM2 through checked private-key
  rebuild/signing helpers and SM2 key-pair/public-key derivation now routes
  through `try_public_key`, SM2 concrete public-key prefixed formatting now
  returns a deterministic invalid-key marker instead of unwrapping checked
  multihash encoding, SM2 private-key byte export now exposes
  `PrivateKey::try_to_bytes` and routes exposed private-key multihash formatting
  through checked payload extraction, secp256k1 message signing now exposes
  `try_sign` and routes `Signature::try_new` through the fallible helper, and
  secp256k1 recoverable prehash signing now checks the low-S recovery-id parity
  flip before emitting EVM-compatible signatures; SM2 embedded-distid payload
  decoding now returns `ParseError` for short length prefixes instead of relying
  on a panic-only fixed-slice assertion, SM2 PEM export now wraps the already
  encoded base64 `String` without a panic-only UTF-8 reconversion, SM2
  DER signature export now exposes `try_as_der` with checked short-form length
  encoding and routes the OpenSSL bridge through that fallible exporter before
  DER parsing, SM4-CCM now checks tag, nonce, AAD, payload, and counter-block
  length narrowing through its existing encrypt/decrypt `Result` paths, the SM
  signature shim's SM4 self-test block now uses the infallible fixed-key
  constructor instead of
  `new_from_slice(...).expect(...)`, and ML-DSA import plus `Signature::try_new`
  reject secrets whose recomputed public material or embedded `tr = H(pk)`
  public hash is inconsistent before signing; SoraNet PQ labeled-HKDF derivation
  now streams the namespace,
  separator, label,
  separator, and context components through `expand_multi_info`, preserving the
  previous contiguous info layout without manual capacity arithmetic;
  SoraNet PQ ML-DSA helpers now apply the same secret-key consistency check to
  direct validation and direct/OS-backed signing, and expose fallible public-key
  reconstruction from secret material;
  BLS same-message aggregate and preaggregated verification now reject
  duplicate public keys and public-key aggregates that cancel to the identity
  before verification, and the public PoP-gated same-message wrappers reject
  duplicate signer keys before PoP verification/cache work and no longer fall
  back to per-signature verification after aggregate rejection; distinct-message
  aggregate verification rejects duplicate messages and aggregate signatures
  that cancel to the identity before batch verification, and the blstrs feature
  backend compressed G1/G2 public-key decoders now use explicit `CtOption` to
  `Option` handling instead of panic-only unwrap assumptions. The blstrs feature
  backend also reuses the w3f signing/message semantics for normal, small,
  same-message, preaggregated, and distinct-message aggregate verification so
  backend choice does not change accepted signatures, and the feature-gated
  `iroha_crypto --all-targets` strict clippy corridor now covers the blstrs BLS
  test targets while the default w3f `bls` all-targets corridor is also green
  after removing an unused panic-only secret-key wrapper. The default w3f BLS
  backend now exposes fallible secret reload, signing, and public-key derivation
  helpers, both BLS backends expose checked keypair generation, the public
  backend helper names `keypair` and `sign` now return `Result`, and the w3f
  stored-secret `public_key` helper is fallible too. SM2 top-level random
  key generation now routes through `Sm2PrivateKey::try_random`, fallible
  `TryCryptoRng` byte draws, and bounded scalar validation before returning
  key material. Top-level BLS keygen, signing, proof-of-possession proving, and
  public-key derivation route through checked paths on `Result`-returning APIs;
  BLS VRF proof construction now returns `Result`, rejects invalid stored
  secret scalars before signing for both Normal and Small variants, and uses
  checked compressed-proof decoding so malformed G1/G2 proof encodings fail
  closed without `CtOption::unwrap`; governance VRF candidate generation
  handles those errors directly instead of relying on `catch_unwind`, and the
  governance council CLI plus core/Torii fixtures now propagate the fallible
  BLS keypair/signing API directly. The
  public `PublicKey::to_bytes` compatibility helper delegates to the checked
  compact-key parser so fallible public-key expansion remains live in
  BLS-enabled builds; Merkle leaf iteration now stops cleanly on an
  unexpected missing leaf slot instead of relying on panic-only internal layout
  assertions, and parent recomputation now stops if malformed in-memory state
  lacks a computed parent slot. Compact Merkle proof conversion and verification
  now share a fixed direction-bitset depth cap instead of converting
  `u32::BITS` through panic-only assertions, while decoded tree layout
  validation remains strict; the multihash `VarUint` codec now decodes through checked `u128`
  accumulation plus final bounded conversion, accepts valid max-width integer
  encodings, rejects oversized canonical varints including high final-chunk
  bits above `u128::MAX`, and constructs continuation bits without unchecked
  tail mutation; SoraNet SRCv2 certificate issue and
  verification now use checked CBOR serialization/digest helpers, with
  canonical integer emission and checked byte/text/array length conversion
  replacing panic-only encoder assumptions; core `Hash` and `HashWriter`
  hashing now use the fixed-output Blake2b-32 digest type, preserving the
  historical digest bytes while removing panic-only variable-output
  initialization/finalization assumptions; Ed25519 and default w3f BLS
  verify-ok cache keys now use the same fixed-output Blake2b-32 route while
  preserving their domain-separated transcripts; Ed25519 public-key parse,
  public-key-full fast-cache, and exact verify cache index helpers now use
  checked little-endian chunk
  extraction and invalid cache-size fallback to index `0`, eliminating
  panic-only cache-index assumptions while preserving the configured
  power-of-two masks, and `Signature::verify` now routes compact public-key
  expansion through checked parsing so malformed in-memory public keys return
  `Error::Parse` instead of reaching Ed25519 invariant panics; `KeyPair::new`
  now validates compact public-key payloads through the same checked parser
  before algorithm comparison or GOST pair validation, so malformed in-memory
  public keys return `Error::Parse` instead of panic-compatible full-key
  expansion; Norito streaming key-update verification now extracts remote
  Ed25519 identities through checked compact-key parsing, so malformed
  in-memory identity keys fail as `HandshakeError::BadSignature` before
  signature verification, suite negotiation, or transport-key state changes;
	  BLS PoP verification, PoP proving, and PoP-gated aggregate public-key
	  collection now use checked compact-key extraction, so malformed in-memory
	  BLS public keys surface through `Error::Parse` before proof verification,
	  duplicate-key caching, or aggregate backend work; public-key fallible string
	  encoders now validate compact payloads through full public-key parsing
	  before multihash formatting, so malformed in-memory keys return
	  `ParseError` instead of canonical-looking bare or prefixed strings;
	  `PublicKey` Norito serialization now reuses the cached full-key parser
	  before writing compact wire bytes, so malformed in-memory keys return a
	  Norito error and no exact encoded length instead of emitting invalid
	  archives; direct `PublicKeyCompact` Norito serialization now applies the
	  same full-key validation before writing tag+payload bytes, so malformed
	  compact state cannot bypass the checked `PublicKey` wrapper; the private
	  compact-to-full conversion is now `TryFrom<&PublicKeyCompact>` and uses
	  checked tag/payload accessors, so malformed compact state returns
	  `ParseError` instead of relying on panic-only invariant accessors;
	  `KeyPair::new` also reuses the checked public-key payload for ML-DSA
	  pair validation instead of re-entering the compatibility
	  `PublicKey::to_bytes()` helper after compact parsing has succeeded;
	  `PublicKey::try_to_bytes()` is now public, giving downstream
	  `Result`-returning paths a checked algorithm/payload accessor without
	  relying on the infallible compatibility wrapper; the legacy signer-backed
	  SCCP EVM submission helper now uses it when deriving Secp256k1 signer
	  public-key bytes, so malformed or non-Secp256k1 signer state fails closed
	  before address derivation; `PublicKey` hashing and ordering now also use
	  checked tag/payload extraction with a deterministic raw compact fallback
	  for malformed in-memory envelopes, so peer maps and sorted target sets no
	  longer reach the infallible compatibility accessor; `PublicKey::try_algorithm()`
	  now exposes checked tag access, while infallible `Display`, `Debug`, and
	  Norito JSON formatting emit a deterministic invalid-public-key marker for
	  malformed in-memory compact envelopes instead of panicking; the `iroha_core`
	  single-Ed25519 admission precheck, parsed-key cache, and allowed-signing
	  admission gate now use checked public-key accessors for fast-path
	  eligibility and signing algorithm checks, so malformed in-memory compact
	  public-key state misses the optimization or returns a structured
	  malformed-signature rejection instead of touching unchecked key invariant
	  accessors; Sumeragi vote-verifier workers now also prepare peer key
	  algorithms and aggregate-verification public-key bytes through the checked
	  accessor, so malformed in-memory consensus peer keys are reported through
	  `VoteSignatureError::SignatureInvalid` before BLS aggregate grouping or
	  raw key-byte collection; block commit/signature subset validation, native
	  AMX attestation signer checks, vNext aggregate-certificate signer
	  classification, lane-relay QC key collection, consensus peer
	  registration, consensus-key registration policy checks, active-roster
	  filtering, and admission-time signature batch prechecks now share checked
	  algorithm/payload extraction for consensus and transaction signer keys, so
	  malformed in-memory keys are rejected through existing signature and policy
	  error surfaces before BLS role checks, PoP lookup, or batch key-byte
	  collection; account/domain controller capability
	  gates now also pass multisig members through their checked public-key
	  accessors instead of the infallible member convenience methods; account
	  controller multisig policy construction, canonical member sorting, CTAP2
	  policy encoding/digesting, and account-address controller encoding now
	  extract compact public-key payloads through checked accessors, so malformed
	  in-memory controller keys return `MalformedPublicKey` or
	  `InvalidPublicKey` on result-returning paths instead of reaching
	  compatibility invariant accessors; trusted-peer PoP config parsing,
	  trusted-roster validation, daemon NPoS validator status counting, genesis
	  trusted-peer PoP verification, and Torii Sumeragi BLS-key operator views
	  now also classify BLS-normal keys through checked accessors, turning
	  malformed in-memory keys into config errors or non-BLS status entries
	  instead of compatibility accessor panics; SCCP Nexus BLS commit-QC
	  verification and fraud assessment attester preflights now also classify
	  public keys through checked accessors before PoP verification, aggregate
	  signature verification, or Ed25519 signature-shape checks; restricted
		  transaction-gossip target scoring and NPoS validator-election tie-break
		  scoring now also read peer public-key bytes through checked accessors,
		  falling back to the deterministic invalid-key marker for malformed
		  in-memory peer keys while preserving valid-peer score inputs; JDG
		  committee manifest validation, attestation signer membership checks, and
		  BLS aggregate PoP lookup now also canonicalize committee and signer keys
		  through checked accessors before duplicate detection, threshold
		  membership checks, or aggregate verification; SoraFS GAR verification now
		  also classifies registered gateway signer keys through checked accessors
			  before Ed25519 JWS signature verification; SoraDNS resolver-directory
			  signing payloads and Torii VPN quote response metering-key hex rendering
			  now also extract public-key payloads through checked accessors, returning
				  existing invalid-parameter/conversion-error surfaces for malformed
				  in-memory keys; SCCP EVM digest signing and Torii SCCP proof-build
				  diagnostics now also require checked Secp256k1 public-key
				  classification before EVM address/signature handling; SCCP canonical
				  Nexus message-bundle and source-chain proof-envelope packaging now uses
				  checked Merkle-proof, inclusion-branch, and dynamic-vector length writers
				  on production admission paths, returning `None` for oversized bundle,
				  source-proof, or transparent-statement transcript fields instead of
				  relying on panic-only `u32` conversions; SCCP source-adapter
				  verification statement, adapter-commitment, and FastPQ context
				  packaging now also fail closed on unbounded adapter-proof shapes and
				  checked proof-byte length prefixes, while the remaining SCCP
				  proof-body transcript helper audit stays open; config
				  parsing for streaming identity, Torii receipt signer, and Torii
				  offline issuer public keys now also uses checked algorithm access
				  before allow-list decisions; the Nexus app
				  facade now classifies selected signing keys through checked
					  accessors before transfer draft construction,
					  Connect approval resolution, or wallet-signature requests; SoraFS
					  gateway PoR proof construction now also extracts the embedded proof
					  signer payload through checked accessors and rejects non-Ed25519
					  gateway signing keys before emitting Ed25519-labelled proof
					  envelopes; native Connect/Norito bridge C ABI and Java/JNI
					  public-key export helpers now also copy public-key payloads only
					  after checked extraction from derived or seeded keypairs; the JS
					  host native binding now also exports generated/derived keypair and
					  alias-proof signer payloads only after checked public-key
						  extraction; reusable core/Torii/config/client/SoraFS Rust fixtures
						  now also use checked public-key payload/algorithm accessors, leaving
						  the targeted compatibility-accessor scan clean across those source
						  roots; operator tooling and daemon paths for SoraDNS resolver signing
						  payloads, SoraNet relay/puzzle identity derivation, Kagami PoP/genesis
						  helpers, Taira canaries, Soracloud release governance proofs, CLI
						  governance/account controller display, and ephemeral Torii receipt-signer
						  logging now also use checked public-key accessors and propagate their
						  existing error surfaces; Taira write-canary generated signers now also
						  use checked Ed25519 keypair generation and surface OS entropy failures
						  through the canary command result path; oracle default reward/slash
						  accounts now derive their fixed Ed25519 ids through checked
						  seed-expansion while preserving infallible config defaults; the
						  `iroha_genesis` manifest-normalize helper now generates its temporary
						  signing key through checked default key generation and reports entropy
						  failures with binary-specific context; the `iroha_crypto` SoraNet
						  handshake-check helper now derives its fixed client/relay Ed25519 keys
						  through checked seed expansion and reports failures through the
						  handshake harness error path; offline v1/v2 interop vector generators
						  now derive their fixed issuer, account, and note Ed25519 keys through
						  checked seed-expansion helpers with fixture-specific error context; the
							  `iroha` dev key-material example now generates its
							  Ed25519 keypair through checked randomness and propagates entropy
							  failures from `main`; the `iroha` Nexus app transfer and tutorial,
							  `iroha_data_model` signed-block/I105 vector, and
							  `iroha_torii_shared` permissions-preimage examples now also use checked
							  Ed25519 generation or seed derivation and surface entropy or fixture-key
							  failures through their example `main` result paths; the `iroha_kagami`
							  Taira Kaigi localnet example now also derives its optional seed-based
							  genesis signer through checked seed expansion and reports failures
							  through the example result path; `iroha_js_host`
							  N-API Ed25519/generic keypair exports and the relay envelope sample now
							  also use checked random generation or seed derivation, mapping failures
							  into N-API errors instead of panic-only keypair wrappers; Offline
							  deterministic escrow account derivation now also uses checked Ed25519
							  seed expansion while preserving the fixed-seed infallible API; account-address
							  vector and compliance-vector fixture public keys now also use checked
							  Ed25519 seed expansion while preserving their fixed seed bytes; Norito
							  fixture-export and trigger-print scripts now also derive their fixed
							  Ed25519 fixture authorities through checked seed expansion; `iroha_test_samples`
							  sample-account generation now exposes a fallible helper and routes seeded/random
							  test key material through checked key-generation APIs; `iroha_core` tx-size
							  and memory examples now also use checked random key generation, with `tx_size`
							  surfacing entropy/keygen failures through its example `main` result; the custom
							  data-model sample fault-injection smoke test now also uses checked random
							  key generation for its transaction signer; confidential keyset generation now
							  accepts fallible `rand_core` 0.9 crypto RNGs and maps spend-key entropy
							  failures to `ConfidentialKeyError::RandomBytes`; SoraNet client and relay
							  handshake construction now also uses fallible `TryCryptoRng` draws for nonce,
							  Noise secret, and client ML-KEM seed material, returning labelled
							  `HarnessError::RandomBytes` failures; SoraNet PoW and Argon2 puzzle
							  ticket minting now also uses fallible `TryCryptoRng` draws and preserves
							  labelled nonce-generation failures through `MintError::RandomBytes` and
							  the p2p challenge wrapper; SoraNet admission-token minting and SoraFS
							  proof-token minting now also use fallible `TryCryptoRng` draws and return
							  labelled `MintError::RandomBytes` failures for admission-token nonce and
							  proof-token id generation; SoraNet request blinding nonce generation now
							  also accepts fallible `TryCryptoRng` inputs and reports entropy failures
							  through `BlindingError::RandomBytes`; P2P handshake hello
							  construction now also extracts local peer key metadata through checked accessors and reports
						  malformed local keys through a dedicated handshake error, while multisig
						  members expose a fallible checked algorithm accessor for result-returning
							  callers; Python native bridge keypair export, account public-key hex,
							  transaction envelope public-key embedding, public-key multihash parsing,
							  public/private multihash formatting, SM2 fixture public-key formatting,
							  and SoraFS alias-proof fixture signer extraction now also use checked
							  public-key payload/formatting access and return Python errors on
							  malformed compact key state; SM2 typed formatter export, Connect C
							  SM2 prefixed formatting, JavaScript native generic/SM2 multihash
							  helpers, Kagami prefixed key JSON output, SoraFS manifest-sign key
							  formatting, and ADDR-2 fixture multihash/prefixed fields now also
							  use checked formatter APIs before emitting operator or SDK-facing
							  strings; xtask SoraNet drill bundles, FastPQ manifests, Taikai anchor
						  summaries, OpenAPI manifests, SoraNet rollout captures, SoraDNS release
						  signing payloads, SoraFS admission/pin fixture generators, and SoraFS
						  gateway token-signing key rotation now also extract embedded Ed25519
						  public-key payloads through checked accessors before writing operator
						  artifacts; offline note tests, ADDR-2 compliance
						  vectors, and Offline V1/V2 interop vector generators now also extract
						  fixture public-key payloads through checked accessors before embedding
						  certificate, address, or offline FI public-key fields; the remaining
						  SoraFS conformance/chunker/pin/discovery fixtures, gov draw fixtures,
						  bridge proof vectors, config/test-network assertions, dev key example,
						  Swift parity generator, and offline-note integration certificate helpers
						  now also use checked public-key accessors, leaving the compatibility-accessor
						  scan confined to `iroha_crypto` internals, tests, and benches; inside
						  `iroha_crypto`, BLS PoP fixtures, generated public-key roundtrips,
						  Ed25519 aggregate/batch fixtures, ML-DSA/PQC fixtures, and the Ed25519
						  hot-path benchmark setup now also use checked public-key payload extraction,
						  while ML-DSA public/private formatter roundtrips and SM2 public-key
						  formatter fixtures now use checked multihash/prefixed formatter APIs,
						  and `PublicKeyFull` normalization internals now use a fallible borrowed
						  canonical-payload path for formatter encoders, and the blstrs typed BLS
						  backend plus default w3f BLS `PublicKeyFull` variants now borrow stored
						  canonical public-key payloads, clearing the targeted BLS formatter
						  compatibility-accessor scan for both backends; bridge finality
						  commit-QC validator classification now also
						  uses checked public-key algorithm access before BLS aggregate
						  verification, returning a structured malformed-validator-key error
							  for malformed compact key state;
						  JDG SDN commitment validation,
								  registry registration/lookup, and attestation commitment dedup now
								  also build SDN public-key fingerprints through checked payload
								  extraction; VPN helper-ticket serialization now also exposes fallible
								  checked byte/hex builders and Torii helper-ticket issuance uses them
								  before embedding metering public-key payloads; embedded Soracloud
								  provider-advert fixture admission now also validates provider and
								  council Ed25519 public-key payloads through checked accessors before
								  embedding advert/admission bytes;
							  X25519 public-key decoders for hybrid KEM keys, hybrid ephemeral ciphertext
				  keys, and the standalone key-exchange surface now reject low-order encodings
  before ECDH while retaining all-zero shared-secret fallback checks, and
  X25519 session-key derivation now maps HKDF expansion failures through the
  shared-secret `Result` path instead of using a panic-only assertion; SoraNet
  PQ ML-KEM key generation now exposes checked direct and seeded constructors,
  routes OS-backed keygen through key-pair validation, and hybrid X25519/ML-KEM
  `try_generate` consumes that checked path before reconstructing the hybrid
  secret. The public `HybridKeyPair::generate` helper now returns `Result`
  instead of panicking after checked generation; hybrid key-generation,
  encapsulation, and SoraFS hybrid payload envelope paths now consume fallible
  `TryCryptoRng` draws and return labelled RNG errors before key, ciphertext,
  or AEAD nonce material is emitted; the public direct and seeded
  `generate_mlkem_keypair*` wrappers now
  return `Result` instead of panicking after validation; nonzero PQClean ML-KEM
  backend statuses now surface as
  `MlKemError::BackendFailure` through keygen, encapsulation, and decapsulation
  `Result` paths instead of panic-only assertions, and ML-KEM 12-bit
  coefficient validators now reject partial byte groups as `BadEncoding`
  instead of relying on debug-only divisibility assertions;
  Kotlin/Java Connect X25519 direction-key derivation now maps provider
  low-order agreement failures into `ConnectProtocolException`, while the
  native Connect bridge FFI rejects the same low-order peer key without touching
  output buffers;
  Kotlin/Java Connect nonce, frame/envelope codec, and queue journal paths now
  reject negative signed sequence values before nonce/AAD construction,
  encoding, decode handoff, or journal persistence, high-bit `uint64` frame
  and envelope sequences fail closed, and ciphertext-frame encoding requests the
  canonical zero-flag Connect Norito field layout explicitly;
  Kotlin Connect approval preimages now canonicalize `accountId` through the
  shared I105 account-literal helper before binding it into wallet
  authorization bytes, matching Java Android and rejecting domain-qualified
  aliases;
  Soracloud uploaded-model `X25519HkdfSha256` admission now requires exact
  32-byte recipient and ephemeral public keys and routes both through the same
  low-order decoder before bundle registration;
  confidential key hierarchy derivation now reports HKDF expansion failures via
  `Result`-returning helpers instead of panic-only assertions, and the CLI
  `create-keys` path now propagates those failures through normal command
  errors instead of a post-length-check `expect`;
  BFV identifier slot encoding and per-slot seed derivation now propagate
  conversion failures through `BfvError` instead of panic-only `usize` to `u64`
  assumptions, and BFV scalar modular helpers now avoid panic-only
  post-reduction integer conversions while preserving max-width modulus
  behavior;
  the RAM-LFE default programmed BFV hidden program now uses profile-sized
  `u16` constants instead of panic-only index conversion assumptions, and its
  memory RNG transcript binds `u64` step values directly; BFV/RAM-LFE
  domain-separated digest, receipt, and RNG-seed transcripts now stream hash
  chunks directly while preserving the previous contiguous byte layout; the feature-gated
  BFV acceleration selector now falls back to deterministic scalar schoolbook
  multiplication for zero or overflowed derived convolution lengths, and its
  CRT-NTT helper path now rejects invalid operand lengths, unsupported NTT
  lengths, and CRT reconstruction overflow before using that same fallback
  instead of relying on panic-only degree or NTT arithmetic;
  confidential encrypted shield payloads now require supported versions,
  non-empty ciphertext, and low-order-free X25519 ephemeral keys before
  `Shield` execution burns public balance or records note commitments, and the
  CLI plus Connect/Norito bridge shield payload builders now run that same
  preflight before instruction construction, raw payload emission, or signing,
  with Swift fallback serialization enforcing matching empty-ciphertext and
  X25519 low-order admission;
  standalone ML-KEM public-key validation, secret-key validation,
  encapsulation, and decapsulation now reject noncanonical 12-bit public-key
  coefficients plus noncanonical secret-key private coefficients, and
  secret-key validation plus decapsulation reject corrupted embedded `H(ek)`
  public-key hashes before implicit rejection can derive divergent transport
  keys;
  changing the streaming ML-KEM profile on key material or live sessions now
  clears configured Kyber public keys, fingerprints, and local decapsulation
  secrets before any later HPKE use, and direct local ephemeral-payload
  precomputation no longer commits Kyber transport keys, negotiated-suite, STS,
  or snapshot state before a signed key update is built or accepted;
  Norito streaming X25519 key updates now require prepared local ephemeral
  material and reject low-order remote ephemeral public keys before
  transport-key derivation or committing session state, X25519 ephemeral
  generation and outbound content-key nonce generation now propagate OS RNG
  failures as `HandshakeError::Randomness` instead of relying on the infallible
  RNG compatibility wrapper, signed
  remote key updates verify signatures and stage key-counter, suite, and
  ephemeral-shape admission on a local copy before X25519 shared-secret
  derivation, ML-KEM decapsulation, transport-key derivation, resetting, or
  committing session state, successful remote key updates now return the
  inserted transport keys directly instead of relying on a panic-only option
  readback, outbound key-update construction
  stages ephemeral generation, transcript signing, and Kyber transport
  derivation before committing session state and rejects zero or same-session
  non-increasing counters before ephemeral generation, direct Norito
  key-update state admission now rejects zero counters and suite/payload length
  mismatches before accepting counters by requiring 32-byte X25519 public keys or
  1088-byte Kyber768 ciphertexts, streaming snapshot restore also rejects zero
  key counters before replacing live session state, direct Norito key-update
  state restore/from-snapshot paths reject zero counters before replacing replay
  state, KeyUpdate and capability
  negotiation admission now rejects zero protocol versions before committing
  suite, counter, transport-key, or ACK state, capability reports must carry the
  viewer endpoint role before p2p or core ACK construction records negotiation
  state, viewer-side capability ACKs must echo the report stream id, protocol
  version, negotiated DATAGRAM size, and DPLPMTUD flag before transport state or
  callbacks are updated, direct Norito STS derivation now rejects non-32-byte
  handshake shared secrets before HKDF, and
  Norito streaming content-key updates now authenticate and unwrap the GCK
  before recording accepted rotation state so malformed wrapped keys cannot
  poison replay windows, while outbound content-key construction rejects
  regressed rotations before nonce generation or AEAD wrapping, inbound,
  outbound, and restored snapshot GCKs must now be exactly 32 bytes, including
  direct Norito GCK wrap/unwrap helpers, direct Norito content-key
  state restore/from-snapshot paths reject partial id/valid-from metadata before
  replacing replay state, and streaming snapshot restore stages
  KEM-suite id validation, transport-key derivation, and Kyber
  public-key/fingerprint validation before replacing live session state,
  rejects partial content-key or Kyber metadata, and binds Kyber768 suites to
  ML-KEM-768 snapshot metadata plus either the validated remote fingerprint for
  inbound state or the validated local fingerprint for outbound state, with
  local Kyber metadata requiring an installed decapsulation secret whose embedded
  public key and `H(ek)` public-key hash match before restore can replace state;
  transport
  capability recording and snapshot restore now reject
  DATAGRAM/fallback shape drift before updating live session state or
  capability hashes; streaming
  feedback admission now clamps inbound
  `parity_chunks`, receiver `parity_applied`, and `fec_budget` to the 6-chunk
  FEC ceiling, and caps inbound loss samples at Q16.16 100% before updating
  snapshot or outbound hint state; the first accepted feedback hint or receiver
  report now binds the feedback state to that stream id, and later feedback
  frames with a different stream id are rejected before counters, EWMA loss,
  parity, or snapshot-visible fields change;
  SoraNet NK2/NK3 handshake parsers now reject low-order Noise static and
  ephemeral public keys in decoded client and relay frames, reject malformed
  Dilithium3/Ed25519 handshake signature field lengths, require 1024-byte
  zero-padded frames, and reject selected KEM/signature ids that are absent
  from either peer's advertised capability TLVs, including the relay capability
  vector echoed in `RelayHello`; unsupported KEM ids fail at the KEM profile
  gate before downgrade telemetry is built;
  SoraNet signed-ticket signing now preflights ML-DSA-44 secret-key lengths,
  and signed-ticket decode/direct verification now reject ML-DSA-44 verifier
  public-key and signature vectors whose lengths disagree with the suite
  metadata before signing payloads, accepting tokens, or entering backend
  verification, while signed-ticket relay/transcript binding checks now run
  before signature work in the full verifier, and signed-ticket policy metadata
  now rejects unsupported versions, difficulty mismatches, expiry, and TTL
  window failures before signature work; signed-ticket ML-DSA payloads now use
  a fixed-size buffer with explicit used length for the optional transcript
  binding while preserving the previous contiguous signed payload layout;
  SoraNet PQ helpers now validate ML-KEM
  encapsulation public-key lengths and ML-DSA signing context/secret-key
  lengths before drawing direct or OS-backed randomness for malformed inputs;
  SoraNet runtime client-hello processing now preflights NK2/NK3 client ML-KEM
  public keys before capability telemetry, relay Noise key generation, OS-backed
  ML-KEM key generation, or encapsulation; runtime handshake descriptor
  commitments and resume hashes must now be 32-byte transcript-binding fields
  before client RNG, relay RNG, transcript hashing, KEM key generation, or
  encapsulation, client/relay capability vectors must now fit the
  length-prefixed handshake field before client RNG or frame construction,
  transcript hashing now rejects capability vectors that cannot fit its fixed
  `u32` length field before hashing, len-prefixed handshake message parsing
  now reads frame fields through checked cursor ranges, capability TLV parsing
  now reads headers and value spans through checked cursor helpers, and
  suite-list capability TLV re-encoding now rejects oversized values through
  `update_suite_list` before encoded capabilities are emitted; deterministic
  handshake fixture and telemetry signature rendering now uses checked base64
  output lengths and fallible slice encoding before returning `prefix:base64`
  witness strings; PoW
  ticket parsing now reads fixed fields through checked cursor helpers, ticket verification,
  signed-ticket verification, ticket
  minting, and Argon2 puzzle verification/minting now reject malformed
  descriptor, relay-id, or transcript binding field lengths before challenge
  derivation, solution search, Argon2 work, or public-key validation;
  PoW and Argon2 puzzle policy parameters now expose fallible constructors for
  runtime config loaders so zero minimum TTLs and inverted future-skew bounds
  fail closed without panicking, and their compatibility `new` constructors now
  return fail-closed policies instead of unwinding on invalid timing bounds; PoW
  ticket minting, Argon2 puzzle minting, and
  revocation-store insertion now reject unrepresentable expiry timestamps
  through checked `SystemTime` conversion; PoW challenge, solution-digest, and
  revocation fingerprints plus Argon2 puzzle challenges now feed BLAKE3
  incrementally while preserving the previous contiguous transcript layout, and
  Argon2 puzzle solution salts now use a fixed-size stack buffer. P2P SoraNet
  runtime construction now uses those fallible constructors for config-derived
  PoW/puzzle bounds;
  relay capability advertisement and runtime GREASE append now check TLV payload
  lengths before writing the two-byte length field, and relay config validation
  rejects configured GREASE payloads that cannot fit that wire field;
  relay
  replay-filter bit counts are now bounded before power-of-two rounding, and
  direct replay-filter construction plus `DoSControls::new` now propagate
  oversized filter shapes as `ConfigError::ReplayFilter` instead of reaching
  overflow-prone arithmetic; relay incentive uptime/scheduled-uptime and
  verified-bandwidth epoch accumulators now saturate on overflow instead of
  panicking on extreme telemetry or proof totals; relay adaptive PoW
  success/failure window counters and difficulty-step arithmetic now saturate
  before min/max clamping, avoiding panic-only overflow paths under extreme
  counters or oversized adaptive-step config; P2P
  QUIC/TCP happy-eyeballs dialing now records the first branch failure and
  returns the second branch failure directly when both dials fail, avoiding
  panic-only option readbacks in the fallback path; SoraNet CID
  blinding key derivation now rejects
  all-zero epoch salts or all-zero circuit secrets before HKDF, and
  request-scoped blinding nonce generation now reports RNG failures without
  panicking; SoraNet
  revocation-store reload now rejects duplicate persisted fingerprints, rejects
  overflowing expiry timestamps, and bounds loaded active records to the
  configured capacity;
  SoraNet guard-directory snapshot decode now rejects duplicate or
  key-mismatched issuer fingerprints and enforces ML-DSA-65 issuer public-key
  length/phase requirements before snapshots are admitted, with issuer key
  shape and the fingerprint `u32` key-length field now checked before
  fingerprint derivation; the public directory issuer-fingerprint helper now
  returns `Result`, and orchestrator guard-directory admission maps fingerprint
  recomputation errors before advertised fingerprint comparison; relay
  directory build and snapshot rotation now propagate fingerprint-computation
  errors with issuer context before signing or publishing a snapshot, and
  guard-pinning fixtures derive ML-KEM public-key lengths from the advertised
  suite instead of stale constants; snapshot
  decode also rejects empty issuer or relay sets before trust-map construction
  or relay certificate verification;
  SoraNet admission-token decode now reads fixed-width body fields and trailing
  signature spans through checked cursor helpers so malformed token prefixes
  return decode errors instead of relying on manual slice invariants; admission
  tokens now expose `try_encode`, and the compatibility encoder fails closed to
  a malformed frame when impossible direct token state cannot fit the v1
  signature-length prefix; admission-token ML-DSA signing bodies now use a
  fixed-size stack buffer for the domain-separated body bytes shared by minting,
  verification, and token-id derivation while preserving the previous
  contiguous transcript layout;
  SoraNet admission-token replay-store reload now rejects duplicate persisted
  token IDs and overflowing expiry timestamps, and admission-token verification
  rejects zero-length or inverted validity windows and preflights ML-DSA issuer
  public-key and detached-signature lengths before backend verification or
  replay-store mutation. Torii SoraFS stream-token issuance now generates token
  IDs through checked OS RNG fills and returns labelled issuance errors before
  signed token bodies are emitted; Torii internal operator-signature request
  headers now generate their base64url nonces through checked OS RNG fills and
  return labelled signing-header errors before canonical request signing, and
  ZK IVM prove job creation now generates public job ids through checked OS RNG
  fills before inserting async job state; Rust client account-signed multisig
  and operator-signed admin request headers now also generate their base64url
  request nonces through checked OS RNG fills and propagate entropy failures
  before request builders are emitted; SoraFS orchestrator guard-cache
  persistence now generates authentication-tag nonces through checked OS RNG
  fills and returns labelled persistence errors before tagged cache bytes are
  emitted, and Taikai cache-admission gossip bodies now generate replay nonces
  through checked OS RNG fills before signed gossip entries are emitted;
  SoraFS orchestrator fetch job IDs now use checked OS RNG fills and return
  `OrchestratorError::JobIdRandomness` before fetch telemetry or provider
  selection continues on entropy failure; local QUIC proxy browser-manifest
  session IDs and cache-tag salts now also use checked OS RNG fills and return
  `ProxyError::RandomBytes` before manifest previews or handshake
  acknowledgements are emitted; Torii MCP async job IDs and Connect session
  SID fallbacks now also use checked OS RNG fills and fail closed with
  JSON-RPC/tool errors before async job state or Connect requests are emitted;
  Torii operator-auth WebAuthn challenge bytes and session tokens now also use
  checked OS RNG fills and fail closed with operator-auth errors before
  challenge or session state is inserted; Torii Connect session app, wallet,
  management, and relay bearer tokens now also use checked OS RNG fills and
  fail closed with internal Connect-session errors before response tokens are
  emitted;
  embedded Soracloud uploaded-model X25519 upload-key persistence now generates
  the local static secret seed through checked OS RNG fills and returns a
  labelled `io::Error` before the key file is written; CLI SM2 keygen and
  confidential `create-keys` random seed paths now generate 32-byte seed
  material through checked OS RNG fills and return normal command errors on
  entropy failure; SoraFS CLI repair idempotency keys, storage-token nonces,
  GAR receipt IDs, and admission-token RNG seeding now use checked OS RNG paths
  and return command errors on entropy failure, while hybrid manifest envelope
  encryption uses the already-fallible `OsRng` path; Soracloud CLI
  mutation-auth signature nonces and staging temporary directory suffixes now
  use checked OS RNG fills and return command errors before request signing or
  staging on entropy failure; Rust client transaction nonces now use checked OS
  RNG reads through fallible `try_build_transaction*` APIs, and client
  submission plus CLI transaction creation paths propagate those entropy
  failures before submit; transaction gossiper public/restricted shuffle seeds
  now derive deterministically from chain/local-peer/max-peer identity material
  and plane domains instead of reading process RNG during actor construction;
  telemetry future ids now use a process-local atomic counter instead of random
  ids; unseeded persisted-RBC chunk sampling now seeds `StdRng` through checked
  OS entropy and reports `SamplingError::RandomSeed` on failure while explicit
  seeds remain deterministic; proactive block-sync gossip now derives
  target-selection seeds from local-peer, height, gossip round, gossip size,
  candidate, and world-peer material instead of reading thread RNG; P2P connect
  scheduling and reconnect backoff jitter now derive bounded delays from
  domain-separated local-peer, remote-peer, address, and attempt-context
  material instead of reading thread RNG; Iroha core queue/storage tests now
  use deterministic counters for synthetic domain names, transaction hashes,
  and stress-test delays instead of process or thread RNG; operator-signature
  integration and Torii fixture helpers now use monotonic deterministic nonces
  instead of thread RNG in test-only signed requests; `iroha_test_network`
  peer selection now uses deterministic round-robin order instead of thread
  RNG; Iroha core memory-example synthetic asset/NFT values now use
  deterministic counters instead of process RNG; Izanami chaos keeps explicit
  seeds deterministic while routing unseeded `StdRng` setup through checked OS
  entropy and returning setup errors on entropy failure; CLI multisig
  auto-account registration now uses checked key generation and returns command
  errors on entropy failure; JS-host SM2 keypair generation now uses checked OS
  entropy through `Sm2PrivateKey::try_random_from_os` and returns N-API errors
  on entropy or key-generation failure; Rust SDK SM2
  `Sm2KeyPair::generate_with_distid` now uses the same checked OS helper and
  returns `ParseError` on entropy or scalar-generation failure; SoraNet PQ
  hedged seed construction now also accepts caller-supplied `TryCryptoRng`
  seed entropy, and ML-DSA keypair/signing plus ML-KEM keypair/encapsulation
  OS helpers delegate through the same fail-closed required-seed boundary
  before deriving PQ material;
  admission-token verifier construction exposes a
  fallible path that rejects malformed issuer public keys before fingerprint
  derivation or runtime state admission, and the compatibility constructor now
  keeps malformed issuer keys as fail-closed verifier state that is rejected
  during ML-DSA preflight before backend signature work or replay-store mutation;
  admission-token decode now rejects unrepresentable `issued_at`/`expires_at`
  UNIX-second fields before downstream relay tools can attempt unchecked
  `SystemTime` conversion;
  admission-token minting now
  preflights issuer ML-DSA secret-key length before nonce generation, body
  construction, or backend signing, and reports nonce RNG failures as typed
  mint errors; SoraNet SRCv2 bundle
  verification re-runs canonical certificate-payload admission for in-memory
  bundles, rejects weak Ed25519 verifier keys, and preflights ML-DSA-65
  issuer public-key and detached-signature lengths before backend verification;
  local SRCv2 issuance reuses certificate-payload admission and ML-DSA-65
  issuer secret-key length preflight before signing bundles; Phase 2 SRCv2
  rollout accepts Ed25519-only relay certificates while Phase 3 remains the
  dual-signature gate;
  SoraNet SRCv2 certificate decode now rejects unknown ML-KEM suite ids and
  key-material length drift for ML-DSA-65 identity keys and advertised ML-KEM
  relay public keys, rejects malformed/noncanonical/weak Ed25519 identity
  public keys, rejects ML-DSA-65 detached signature length drift, and its
  canonical CBOR parser rejects trailing payload/bundle bytes plus non-shortest
  integer/length encodings and duplicate nested
  bundle/signature/endpoint/KEM-policy fields, with byte/text/exact payload
  reads routed through checked cursor helpers; SRCv2 validity-duration accessors
  now use checked signed timestamp subtraction, expose a checked route for
  callers, and fail closed to `Duration::ZERO` for directly constructed inverted
  or unrepresentable windows; guard-directory relay entries
  now parse as SRCv2 bundles and must bind to a known snapshot issuer, the
  snapshot directory hash, and a unique relay ID, with relay certificate
  signatures verified against embedded issuer keys under the snapshot
  validation phase; zero-length or inverted snapshot validity windows now fail
  closed, and relay certificate validity must cover the full snapshot window
  without being published after the snapshot; SRCv2 role/capability bitmask decode rejects unsupported
  bits instead of masking them away and validity windows fail closed when they
  are inverted or published after expiry; KEM rotation policies reject static
  fallback/rotation/grace metadata, staged policies without fallbacks, rolling
  policies without nonzero cadence, and preferred/fallback suite equality;
  handshake-suite preference lists and endpoint URL lists must be non-empty and
  duplicate-free, and endpoint URL strings reject empty,
  whitespace-bearing, or control-character values; endpoint tags, when present,
  reject empty, whitespace-bearing, control-character, or duplicate values;
  remaining breadth should emphasize full cross-SDK RNS vectors and broader
  release validation.

## Kotodama first-release follow-ups

- Completed 2026-06-01: static compiler-derived access descriptors now cover
  the formerly opaque peer, subscription, VRF epoch seed, AXT, and Soracloud
  host helper syscalls. Dynamic, malformed, and syscall/operation-mismatched
  payloads stay in the incomplete-hint path and are rejected by production
  compilation instead of being represented with wildcard fallbacks.

## Nexus independent lane consensus follow-ups

- Replace the current global proposal-path lane lookahead with the full
  per-lane proposal/vote scheduler so lane blocks are proposed, executed, and
  QC-sealed by their own lane committees instead of being emitted as relay
  metadata from the global block path.
- Wire lane-local DA/RBC payload ownership into that scheduler and persist lane
  block artifacts independently from global block sealing.
- Add a multi-peer integration corridor proving two active lanes can advance at
  different heights, produce lane-domain QCs, upgrade FastPQ relay proofs, and
  merge without waiting for an idle configured lane. Broaden the unit-level
  committed-record hydration coverage into restart/replay coverage for
  persisted verified relay records.

## Cross-dataspace AMX follow-ups

- Add a multi-peer integration corridor that proves native AMX receipts emitted
  by the universal coordinator survive block relay, Sumeragi status export, and
  downstream audit consumption.
- Extend SDK/OpenAPI convenience models for lane settlement commitments so
  native AMX receipt legs are first-class in client responses instead of only
  available through generic commitment decoding.
- If future coordinator execution supports partial prepare failure inside a
  batch, extend `NativeAmxReceipt` with explicit abort evidence. Finalized
  receipts currently represent only successfully committed native AMX batches.

## Transaction pipeline follow-ups

- Broaden fee/gas/Nexus detached postprocessing beyond the current simple
  transparent single-transfer case. Remaining work includes deterministic
  receipt/effect representation for multi-instruction and multi-asset deltas,
  plus data-trigger-aware fee event ordering. Those shapes intentionally remain
  visible as `fee_postprocessing` detached fallback reasons in Sumeragi status
  and pipeline telemetry.
- Broaden validation from the focused scheduler, dynamic IVM access, telemetry,
  fee-enabled transfer, query-continuation, and receipt-hash tests to the next
  long `cargo test --workspace` corridor once the repository-wide dirty
  worktree settles.

## Torii query API follow-ups

- Completed 2026-06-01: audited endpoint-specific OpenAPI schemas and SDK
  convenience parsers for app endpoints that expose concrete response models.
  Account, domain, account-asset, asset-definition, NFT, RWA, asset-holder, and
  repo-agreement list/query responses now share concrete page schemas that
  document required `has_more`, required `count_mode`, and optional `total`;
  JavaScript and Python convenience parsers preserve bounded page metadata and
  reject malformed count flags before treating a response as valid.
- Completed 2026-06-01: added sustained Torii query-load profiles to
  `torii_hot_paths` for signed iterable `/query` in stored-cursor bounded mode,
  primary account-alias projections, account-asset predicates, asset-holder
  scans, committed-history contract-activity predicates, and generic aggregate
  queries under concurrent in-process HTTP clients. The signed profile walks
  deep continuation chains over the Arc-backed snapshot replay path, the
  contract-activity profile builds real committed transactions with contract
  metadata, and `query_load_profiles` rejects malformed or adversarial
  benchmark shapes before fixture construction.
- Completed 2026-06-01: added a localhost socket transport group for the same
  sustained Torii query profiles. The socket group binds ephemeral Axum
  listeners and drives them with pooled `reqwest` clients so handler-only
  measurements can be compared with real HTTP transport and body IO overhead.
- Run the full signed/app socket profile suite under production-like datasets
  and longer measurement windows to decide whether the existing account-asset,
  asset-holder, and contract-activity predicates need additional indexes or
  materialized views.

## Offline V2 Torii follow-ups

- Completed 2026-06-06: Torii now mounts the versioned Offline V2 issuer
  routes under `/v1/offline/v2/*`, including readiness, key refill, note issue,
  note redeem, and audit. The redeem route submits `RedeemOfflineNoteV2` after
  binding the redemption to the authenticated account/asset, validating the
  chain-admissible key certificate, recomputing recursive public inputs, and
  rejecting malformed nullifier/amount shapes.
- Completed 2026-06-06: removed the stale legacy Offline policy/revocation HTTP
  route registrations from Torii and the source/generated OpenAPI surfaces; the
  Offline readiness smokes now assert `/v1/offline/revocations*` is absent.
- Completed 2026-06-06: removed the v1 Offline redeem/audit HTTP stubs that only
  returned issuer-unavailable errors. The smokes now assert
  `/v1/offline/notes/redeem` and `/v1/offline/audit` remain absent while the
  production redemption/audit surface lives under `/v1/offline/v2/*`.
- Completed 2026-06-06: removed the default governance council derive-vrf
  not-implemented fallback and aligned HTTP route registration, OpenAPI paths,
  and MCP tools behind `gov_vrf` for council persist/replace/derive-vrf
  mutation helpers.
- Completed 2026-06-06: refreshed `fixtures/offline/interop_contract_v2.json`
  and its generator so the published redeem vector uses
  `OFFLINE_NOTE_KEY_CERTIFICATE_VERSION` directly. Torii now consumes the
  committed fixture without normalization and keeps a separate stale-version
  rejection regression, while Swift, Kotlin/JVM, and Java Android SDK
  constructors mirror the same key-certificate version.

## SoraFS paid pin validation follow-ups

- Completed 2026-06-04: reran the SoraFS paid-pin validation corridor across
  the data-model SoraFS filter, DA pin intent query-response roundtrip, Core
  pin-registry suite, Torii storage-pin/discovery suite, and integration gateway
  policy/conformance filter. The pass is green after the paid-pin adversarial
  coverage and proof-token hardening work.
- Completed 2026-06-06: Torii DA commitment proof/verify routes are now pinned
  at handler level with a committed block-backed Merkle proof round trip and a
  tampered-root rejection. The OpenAPI and MCP descriptions now describe the
  Merkle proof contract instead of the stale placeholder wording.
- Completed 2026-06-06: Torii DA pin-intent proof/verify handlers are now
  pinned against the live indexed `DaPinStore`: handler coverage proves by
  lane/epoch/sequence, verifies the returned block location payload, rejects a
  tampered indexed location, and the OpenAPI/MCP descriptions now describe the
  indexed-location contract instead of placeholder proof language.
- Completed 2026-06-06: Torii SoraFS CAR range coverage now includes a
  non-full middle-of-manifest window spanning exactly two aligned chunks. The
  regression verifies the streamed CAR against the manifest-bound byte range
  and pins `Content-Range` plus `X-Sora-Chunk-Range` metadata for partial
  responses.
- SoraFS proof-token decode now uses checked cursor reads for fixed-width
  moderation-token fields with truncated-prefix regression coverage while
  rejecting unrepresentable issued/expiry UNIX-second fields before
  `SystemTime` conversion; proof-token body encoding now exposes `try_encode`,
  routes mint/signature/digest helpers through checked entry-count and
  entry-length narrowing, and makes the compatibility `encode` path fail closed
  to a malformed frame for impossible direct token states; proof-token minting
  now reports token-id RNG failures through labelled `MintError::RandomBytes`
  before blinded digest or signature material is produced; proof-token base64
  header encoding/decoding now uses the `base64` crate's checked no-alloc slice
  helpers instead of manual capacity arithmetic and panic-only buffer
  assertions.
- Remaining breadth should include SDK validation once Java is available and
  any wider admission/manifest-envelope/full-corridor reruns not covered by the
  current focused Torii SoraFS checks.

## Norito columnar and streaming validation follow-ups

- Fold the focused NCB row-count prefix regression into the next full Norito and
  workspace validation budget. Columnar `u64` combo views now read their `u32`
  row-count prefix through a shared checked helper, so truncated prefixes return
  `Error::LengthMismatch` on the normal decode path. Streaming baseline RLE
  block decode now reads DC differences and AC records through checked helpers,
  keeping truncated or overflowed cursor state on `CodecError::TruncatedBlock`
  before offset advancement, and baseline frame/chroma metadata uses checked
  fixed-width readers before chunk payload slicing. Bundled rANS SIMD stream
  lane lengths also use a checked prefix reader before cursor advancement or
  lane slicing.

## ZK audit validation follow-ups

- Completed 2026-06-06: Torii ZK prover report list/count/bulk-delete filters
  now reject malformed `has_tag` filters unless they are exactly four printable
  ASCII ZK1 TLV tag characters, with unit and router-level coverage for the
  fail-closed query contract.
- Completed 2026-06-06: Torii's prover-report success fixture now uses the
  public `halo2/ipa:tiny-add-public` envelope and matching registry schema
  hash, clearing the full `zk_prover_integration` target under `app_api`.
- Fold the now-green focused ZK cleanup and adversarial negative corridor into
  the next long `cargo test --workspace` / CI validation budget.

## TradFi ISO 20022 interop follow-ups

- Completed 2026-06-01: added inbound lifecycle endpoints for `pacs.002`,
  `pacs.004`, `camt.056`, `sese.023`, `sese.024`, and `sese.025`, with OpenAPI
  and MCP submission surfaces. The bridge records each lifecycle message in the
  durable ISO record model, rejects duplicate payload, business-message-id, and
  UETR replays, applies `pacs.002`/`pacs.004`/`camt.056` and
  `sese.024`/`sese.025` updates only when the referenced durable record is
  known, and records `sese.023` as a settlement instruction. The 2026-06-04
  ledger-crosswalk gate below now requires account, instrument, venue, CSD, and
  cash-leg mappings before live securities instructions are durably accepted.
- Completed 2026-06-01: added durable-record outbox helpers for `pacs.004`,
  `camt.029`, `sese.024`, and `sese.025`. Payment returns require recorded
  settlement amount/currency from the original payment message; securities
  confirmations require captured `sese.023` amount, currency, quantity, movement,
  payment, and execution-plan fields rather than fabricating missing data.
- Completed 2026-06-01: added a fail-closed Torii verification path for
  `require-verified` embedded-signature profiles. The bridge now accepts the
  supported P-256/SHA-256 enveloped XMLDSig/XAdES subset only after payload
  digest and signature verification, rejects tampered digests/signatures and
  unsupported algorithms, and keeps live `reject-unsupported` profiles rejecting
  embedded signature blocks.
- Completed 2026-06-01: added profile-specific XMLDSig trust pins for
  `require-verified` profiles. Torii now rejects otherwise valid signed
  payloads unless the verified raw public key or DER certificate SHA-256 digest
  matches the selected rail profile, rejects non-canonical/all-zero configured
  pins at startup, and covers the supported C14N 1.0, C14N 1.1, and exclusive
  C14N algorithm identifiers with deterministic fixtures.
- Completed 2026-06-01: added XMLDSig/XAdES certificate-chain verification for
  `KeyInfo/X509Data`. Torii now accepts at most eight unique DER certificates,
  derives the signing key from the leaf
  certificate, verifies each supplied leaf-to-issuer chain link by binding the
  child issuer distinguished name to the parent subject distinguished name and
  checking the child signature before exposing issuer/root DER SHA-256 digests
  to the selected profile, requires leaf critical `keyUsage` carrying
  `digitalSignature` while rejecting CA leaf certificates, requires issuer
  critical CA `basicConstraints` plus critical `keyUsage` carrying
  `keyCertSign`, requires every supplied certificate to use ECDSA-with-SHA256
  over id-ecPublicKey secp256r1 with uncompressed P-256 SEC1 subject public-key
  bytes, enforces issuer `pathLenConstraint` values for subordinate CA chains,
  rejects unknown, malformed, or unsupported parsed critical X.509 extensions
  on every supplied certificate, checks leaf and issuer certificate validity
  against deterministic verified signed `SigningTime` or BAH `CreDt`, and
  covers the pinned-issuer accept/reject corridor with generated P-256 fixtures.
- Completed 2026-06-02: added a deterministic supported XML canonicalization
  subset for XMLDSig verification. Torii now canonicalizes `SignedInfo` and the
  referenced enveloped payload before hashing or signature verification. The
  supported Reference URI scope is a single empty URI or a unique same-document
  `#id` target using exact `Id`, `ID`, `id`, or `xml:id` attributes; remote,
  empty-fragment, duplicate-ID, namespace-qualified non-`xml` ID attributes, and
  same-document payload targets that do not strictly enclose the verified signature carrier
  fail closed, while selected same-document targets carry ancestor namespace
  declarations into root canonicalization. Each supported payload Reference must
  declare an enveloped-signature transform first, may add at most one final
  supported C14N transform that controls digest canonicalization, and must use a
  SHA-256 digest method; missing, reordered, extra, or unsupported transforms
  fail closed. The verifier also accepts one optional XAdES `SignedProperties`
  Reference with the XAdES `SignedProperties` Type URI, a local `#id` target, one
  supported C14N transform, and a SHA-256 digest; its enclosing
  `QualifyingProperties` target must bind to the enclosing `Signature` `Id`, and
  certificate-backed XAdES signatures must present a non-empty, duplicate-free
  ordered prefix of the verified XMLDSig certificate-chain SHA-256 digests,
  starting with the leaf certificate. The supported signed-property subset
  requires direct
  `Signature/Object/QualifyingProperties/SignedProperties/SignedSignatureProperties`
  structure; `QualifyingProperties` accepts only `Target`,
  `SigningCertificateV2` accepts only attribute-free direct `Cert` children with
  attribute-free direct `CertDigest` children, a `DigestMethod` carrying only
  `Algorithm`, and text-only digest values. Signed `SigningTime` is a singleton
  attribute-free text leaf. Any `SignedProperties` element under the signature
  must be the verified referenced direct target; unreferenced, wrapped, or
  duplicate `SignedProperties` elements and unrelated additional References fail
  closed.
  Supported XMLDSig method and transform elements are parameter-free: `CanonicalizationMethod`,
  `SignatureMethod`, `DigestMethod`, payload Reference transforms, and
  `SignedProperties` transforms reject non-whitespace child content such as
  `InclusiveNamespaces`, XPath, HMAC, or digest parameters. Critical XMLDSig
  method elements must appear exactly once, Reference transforms must be
  enclosed in exactly one attribute-free `Transforms` wrapper, and only
  implemented ordinary attributes are accepted (`Algorithm`, payload Reference
  `URI`, and XAdES Reference `URI`/`Type`). Extra direct children under
  `Reference` or `Transforms` fail closed, and supported References must keep
  direct children ordered as `Transforms`, `DigestMethod`, then `DigestValue`.
  Top-level `Signature` and
  `SignedInfo` parsing now accepts only implemented direct children in supported
  XMLDSig order, so reordered or wrapped `SignedInfo`/method nodes, unsupported
  direct children, and duplicate singleton signature nodes fail closed. The
  payload may contain exactly one supported signature carrier: either a bare
  XMLDSig `Signature` or an ISO `Sgntr` wrapper with exactly one direct XMLDSig
  `Signature` child. Any additional `Signature`/`Sgntr` element outside the
  verified carrier fails closed. Required XMLDSig base64 fields
  such as `SignatureValue`, per-Reference `DigestValue`, and XAdES
  `CertDigest` values reject duplicates and must be attribute-free text leaves
  without nested markup or comments; `PublicKey` must be singular, and
  `PublicKey`/`X509Certificate` credential leaves follow the same no-markup
  rule. Public-key material must not be mixed with `X509Certificate` material
  in the same `KeyInfo`. Key material must be scoped to exactly one `KeyInfo`
  using either `KeyValue/ECKeyValue` with the P-256 `NamedCurve` URI whose
  `PublicKey` bytes parse as an uncompressed P-256 SEC1 point, or one bounded
  duplicate-free `X509Data` certificate-chain wrapper; those wrappers accept
  only implemented direct children, and unsupported children, unsupported
  ordinary attributes, non-whitespace wrapper text, duplicates, or out-of-scope
  `PublicKey`/`X509Certificate` elements fail closed. The canonical subset
  covers empty-element expansion, attribute quote normalization, namespace
  declarations, unprefixed attributes, declared prefixed attributes, and implicit
  `xml:` attributes while accepting and omitting the fixed legal `xmlns:xml`
  declaration. It
  also decodes predefined and numeric XML character references before
  re-emitting canonical text/attribute bytes. It applies root namespace
  declarations inherited from an enclosing XMLDSig `Signature` element according
  to the declared C14N mode: inclusive C14N carries all inherited root namespace
  declarations, while exclusive C14N carries only visibly used inherited root
  namespace declarations. No-comments C14N now omits
  valid XML comments from `SignedInfo` and referenced payload bytes while
  rejecting malformed comments. The verifier still rejects processing
  instructions, CDATA/CDEnd tokens, uppercase `#X` numeric character
  references, DTD/general/custom entity expansion, carriage returns, duplicate
  attributes, unbound prefixed attributes, explicit reserved namespace
  rebindings, malformed structural QNames such as double-colon local-name
  matches, inherited namespace context beyond root declarations, raw attribute
  whitespace rewrites, and malformed tag structure.
- Completed 2026-06-02: broadened XMLDSig ECDSA `SignatureValue`
  interoperability. Torii now accepts the fixed-width P-256 `r || s` signature
  encoding used by XMLDSig profiles while retaining DER fixture compatibility,
  requires canonical low-S for both encodings to remove ECDSA malleability, and
  the require-verified suite covers accepted low-S plus rejected high-S
  signatures.
- Completed 2026-06-02: hardened XMLDSig namespace binding for the supported
  signed ISO subset. Prefixed XMLDSig structural elements must now resolve to
  the XMLDSig namespace in their inherited scope across `Signature`,
  `SignedInfo`, `Reference`/`Transforms`/`Transform`/`DigestMethod`/
  `DigestValue`, and public-key or X.509 `KeyInfo` material, with regressions
  covering a correctly signed payload that binds `ds` to a non-XMLDSig URI.
  Unprefixed XMLDSig structural elements remain accepted for legacy fixtures
  only when they do not carry an explicit conflicting default namespace.
- Completed 2026-06-02: tightened supported XML element span matching so a
  selected opening tag must close with the exact same qualified name. This keeps
  local-name discovery for prefixed XMLDSig fixtures while rejecting malformed
  mismatched-prefix close tags before structure or cryptographic verification
  continues.
- Completed 2026-06-02: tightened XMLDSig attribute value extraction to exact
  XML attribute names. Namespace-qualified spoof attributes such as
  `ds:Algorithm` or `ds:URI` no longer have a local-name fallback in the
  accessor and remain rejected before method, transform, digest, or Reference
  policy is evaluated.
- Completed 2026-06-02: hardened XAdES namespace binding for the supported
  signed-property subset. Prefixed XAdES structural elements now must resolve to
  the ETSI XAdES v1.3.2 namespace (`http://uri.etsi.org/01903/v1.3.2#`) across
  `QualifyingProperties`, `SignedProperties`, `SignedSignatureProperties`,
  `SigningTime`, `SigningCertificateV2`, `Cert`, and `CertDigest`; referenced
  `SignedProperties` targets carry inherited namespace scope into verification,
  and wrong-namespace XAdES payloads fail closed even when re-signed. Unprefixed
  XAdES structural elements now also reject explicit conflicting default
  namespaces.
- Completed 2026-06-02: added profile-level XMLDSig certificate revocation
  pins. Operators can configure `revoked_certificate_sha256` alongside the
  trust pins; Torii validates the SHA-256 deny list at startup and rejects an
  otherwise trusted XMLDSig chain when any verified leaf/issuer DER digest is
  explicitly revoked.
- Completed 2026-06-02: tightened ISO XMLDSig X.509 production admission so
  Torii config and shared profile JSON SHA-256 trust/revocation pins must
  already be canonical lowercase hex, `x509_trust_anchor_sha256_pins` and
  legacy certificate pins require a linked issuer certificate beyond the leaf,
  and CRL/OCSP freshness plus delegated OCSP responder certificate validity are
  evaluated at verified XAdES `SigningTime` or BAH `CreDt` rather than local
  wall clock.
- Completed 2026-06-02: documented the XMLDSig trust-anchor rotation pattern
  for operators: overlap current and next certificate pins during upstream
  cutover, remove the retired pin after cutover, and use
  `revoked_certificate_sha256` only for compromised leaf/anchor digests that
  must override otherwise valid trust pins.
- Completed 2026-06-02: tightened the ISO OCSP DER parser used by
  `require-verified` XMLDSig/XAdES revocation checks. Torii now rejects
  non-shortest long-form DER lengths and non-minimal positive integer encodings
  before OCSP status, responder, or signature validation.
- Completed 2026-06-02: tightened the supported ISO OCSP subset so
  `ResponseData` and `SingleResponse` extensions fail closed instead of being
  ignored by the local parser. Full OCSP extension-policy processing remains
  outside the first-release subset.
- Completed 2026-06-02: extended ISO bridge idempotency to business message
  identifiers. Torii now indexes trimmed `BizMsgIdr`/BAH business-message IDs
  alongside payload hashes and normalized UETRs, rejects replay by business
  message id across distinct durable message records, and preserves the existing
  conflict guard when a rejected message is retried with another record's
  business message id.
- Completed 2026-06-02: tightened reference snapshot checksum coverage for
  profile validation. Torii now has focused coverage proving inbound admission
  metadata records the exact `ReferenceDataSnapshots::snapshot_id()` checksum
  after a BIC/LEI snapshot is loaded, and that the loaded-snapshot checksum
  differs from the all-missing default snapshot.
- Completed 2026-06-02: broadened Torii ISO profile/lifecycle transition
  coverage. Profile admission now returns `UnknownMessageType` when the selected
  rail profile has no inbound message profile for the submitted endpoint family,
  rejects BAH `MsgDefIdr` values outside the selected profile's version set, and
  covers known-original `pacs.004` return plus `camt.056` cancellation paths down
  to durable original-message and lifecycle-message status fields.
- Completed 2026-06-04: added a checked-in `sese.024` securities status-advice
  XML fixture and pinned it at both the IVM parser layer and the Torii
  lifecycle layer. The Torii regressions now cover known-original pending
  updates, unknown-original recording without synthetic record creation,
  wrong-family originals, and conflicting settlement references.
- Completed 2026-06-04: added checked-in `pacs.004` payment-return and
  `camt.056` cancellation-request XML fixtures. IVM parser tests pin the
  canonical return/cancellation fields, and Torii tests prove those fixtures
  drive known-original rejected/pending transitions without synthetic original
  creation.
- Completed 2026-06-04: added a checked-in `pacs.002` payment-status XML
  fixture. IVM parser tests pin the canonical status/original/additional-info
  fields, and Torii tests prove the fixture settles a known original payment.
- Completed 2026-06-04: added adversarial `pacs.002` lifecycle coverage proving
  `TxInfAndSts/StsId` cannot shadow `GrpHdr/MsgId` for durable lifecycle ids or
  audit business-message ids.
- Completed 2026-06-04: added Apache-2.0 mirrored Standards Editor XSD
  fixtures for `pacs.002.001.10`, `pacs.004.001.10`, `camt.056.001.08`, and
  `camt.056.001.09`. The MDR/XSD live-profile matrix now validates BAH status,
  return, and cancellation reports with rail-specific version and
  business-service controls wherever the default profiles allow those exact
  versions.
- Completed 2026-06-04: extended the XSD fixture preflight and production
  readiness rollup with default profile-catalog coverage. The XSD verifier can
  parse `DEFAULT_PROFILES_JSON`, record concrete profile-advertised message
  versions, and fail `--require-profile-schema-backed-versions` when a version
  lacks a schema-backed XML fixture; readiness rechecks those counts and
  missing-version entries, including canonical profile ids, ISO family message
  types, allowed directions, and message-definition family binding, before
  accepting an XSD summary. The summaries now also bind the manifest SHA-256,
  per-schema source repository/commit/path,
  SPDX license, source SHA-256, profile source-file SHA-256, and embedded
  catalog JSON SHA-256 values for release evidence provenance, cap source
  repository URLs at 2048 characters, require exactly one active Rust
  `DEFAULT_PROFILES_JSON` raw-string declaration while ignoring
  spoofed declarations in comments or unrelated strings, and fail closed
  on duplicated, malformed, or unknown-key profile/message/direction/version
	  catalog entries. Manifest schema and fixture paths now fail closed on
	  backslashes, leading-dash path segments, empty or dot segments, forbidden parent-segment forms, and
  DTD/entity declarations before an XSD/profile summary is emitted. Schema
  `Document` declarations must also be unambiguous: exactly one top-level
  `Document` element whose type is exactly the local `Document` type, one
  referenced `Document` complex type, one direct `Document` sequence, and one
  direct payload element with exact `name`/`type` attributes, no `ref`
  indirection, a local unprefixed type, and exactly one matching local payload
  complex type containing exactly one direct `xs:sequence`; XSD composition
  (`xs:import`, `xs:include`, `xs:redefine`, `xs:override`) and
  foreign-namespace direct children under schema, `Document`, or payload
  structures fail before evidence can depend on ignored or unpinned schema
  declarations. Schema roots must declare exactly `elementFormDefault` and
  `targetNamespace`, rejecting root-level `attributeFormDefault`,
  `xsi:schemaLocation`, or other schema-root hints before evidence is emitted.
  Checked XML fixture `Document` and immediate payload roots must be
  attribute-free, so fixture-local schema-location hints or root metadata cannot
  enter digest-bound summaries. Manifest, schema, and fixture files are parsed
  and hashed from the same checked byte buffer, with manifest JSON and profile
  catalog source capped at 4 MiB and schema/fixture XML capped at 8 MiB before
	  parsing, while optional `xmllint` stdout/stderr is drained through a 64 KiB
	  cap and validator runtime is bounded by positive finite
	  `--xmllint-timeout-secs`, preventing restricted-term,
  XML-parse, and emitted-digest evidence from drifting across separate reads.
  Catalog `versions` lists now only skip
  schema-backed checks for the exact message-family alias; unrelated or
  duplicated family aliases fail before an XSD/profile summary is emitted.
  Optional runtime catalog
  fields are also checked against the runtime parser contract: rails,
  embedded-signature policies, and structured-address modes are required;
  optional required reference datasets, trust/revocation pins and OIDs,
  trusted/revoked pin overlap, bounded canonical CRL/OCSP base64 DER-sequence
  material, revocation-flag material requirements, `require-verified`
  trust-pin presence, booleans, supplementary-data caps, business-service
  dependencies, and amount minor-unit currency rows are shape-checked when
  present.
  Candidate schema imports fail closed when source provenance is missing,
  malformed, digest-drifted, or when an XSD contains known restricted Standards
  Editor redistribution terms, preventing public mirrors with embedded
  no-redistribution notices from being treated as production fixture evidence.
- Broaden XMLDSig/XAdES fixture coverage beyond internal P-256 key and
  generated certificate-chain material, including complete canonical XML
  coverage for broader signed ISO envelopes, official
  rail/profile-specific trust-anchor packages, official CRL/OCSP or rail
  revocation-feed fixtures.
- Add official MDR/XSD fixture coverage per profile until both strict
  schema-backed fixture checks and profile-advertised version checks pass; do
  not import mirrored Standards Editor XSDs whose embedded terms prohibit
  redistribution.
- Completed 2026-06-01: tightened the deterministic XMLDSig/XAdES subset so
  `require-verified` profiles only accept the C14N 1.0 + single enveloped
  transform shape that the verifier actually checks. C14N 1.1, exclusive C14N,
  extra transforms, and duplicate `Sgntr` blocks now fail closed.
- Completed 2026-06-02: bound XAdES `QualifyingProperties` objects to their
  enclosing XMLDSig signature id. When a `QualifyingProperties` element is
  present, the supported subset now requires exactly one such element inside a
  single `Object`, requires a non-empty `Target="#..."`, and requires that
  target to match the `Signature`/`Sgntr` `Id`; copied, duplicate,
  mis-targeted, targetless, idless, or out-of-object XAdES properties fail
  closed before signature admission.
- Completed 2026-06-02: required XAdES `SignedProperties` to be
  cryptographically referenced from `SignedInfo`. XAdES-bearing signatures now
  need exactly one payload reference plus exactly one `Reference` whose URI
  targets the `SignedProperties` `Id`, whose `Type` is the XAdES
  SignedProperties reference type, and whose SHA-256 digest matches the
  `SignedProperties` XML in the target-bound `QualifyingProperties` object.
  Missing, wrong-URI, wrong-Type, digest-tampered, content-tampered, or
  missing-element XAdES property references fail closed.
- Completed 2026-06-02: bound XAdES `SigningCertificateV2` to X.509 signer
  material. X.509 `KeyInfo` signatures with XAdES signed properties now require
  a single `SigningCertificateV2` / `Cert` / `CertDigest` entry using the
  supported SHA-256 digest method, and that digest must match the exact signer
  leaf DER certificate admitted from `KeyInfo`. Missing, duplicate,
  wrong-algorithm, wrong-digest, or raw-public-key-with-certificate-property
  cases fail closed.
- Completed 2026-06-02: made known `SigningCertificateV2` issuer/serial
  metadata fail closed until the verifier binds it semantically. Digest-valid
  `IssuerSerial`, `IssuerSerialV2`, prefixed `xades:IssuerSerialV2`, and
  `X509IssuerSerial` material inside the XAdES certificate entry now fails
  before X.509 signer admission.
- Completed 2026-06-02: required the supported XAdES `SignedProperties`
  structure to carry exactly one `SignedSignatureProperties` block with one
  non-empty `SigningTime`. X.509 `SigningCertificateV2` signer evidence is now
  accepted only from inside that `SignedSignatureProperties` block; missing or
  duplicate signature-properties blocks, missing or duplicate signing times, and
  `SigningCertificateV2` material outside `SignedSignatureProperties` fail
  closed.
- Completed 2026-06-02: tightened XAdES `SigningTime` admission to the
  supported canonical UTC `YYYY-MM-DDTHH:MM:SSZ` subset with real calendar and
  clock bounds. Whitespace-spliced values, offsets, fractional seconds,
  non-ASCII digits, malformed widths, year zero, invalid leap days, invalid
  month lengths, and out-of-range hours/minutes/seconds now fail closed even
  when the `SignedProperties` digest and signature are otherwise internally
  consistent.
- Completed 2026-06-02: made the supported XAdES property subset fail closed
  for property classes the verifier does not semantically process. The bridge
  now rejects `SignedDataObjectProperties` and data-object transform metadata,
  signed signature policy/place/role properties, and unsigned timestamp,
  counter-signature, revocation, and archive property families even when the
  `SignedProperties` digest and XMLDSig signature are internally consistent.
- Completed 2026-06-02: added namespace-prefixed XMLDSig/XAdES fixture coverage.
  The `require-verified` verifier now has a positive `ds:`/`xades:` signed
  P-256 fixture whose prefixed `SignedInfo` is signed directly, plus a prefixed
  unsupported-property negative case to prove local-name matching does not let
  namespaced XAdES policy/place/role properties bypass the fail-closed subset.
- Completed 2026-06-01: added profile-level
  `signature_public_key_sha256_pins` for `require-verified` XMLDSig/XAdES
  profiles. The verifier now fails closed without configured pins, accepts raw
  XMLDSig public keys and X.509 certificate subject public keys only when their
  SHA-256 pin matches the profile, rejects malformed/all-zero pins, and rejects
  ambiguous or duplicate key material.
- Completed 2026-06-01: added profile-level
  `x509_trust_anchor_sha256_pins` for X.509 XMLDSig key-info chains in the
  supported P-256/SHA-256 subset. The verifier now validates leaf-to-anchor
  issuer links, ECDSA certificate signatures, certificate validity windows,
  CA/keyCertSign trust anchors, duplicate certificates, non-CA anchors, missing
  anchors, issuer mismatches, and trust-anchor DER SHA-256 pins before using an
  X.509 leaf key that is not directly pinned.
- Completed 2026-06-01: added profile-level
  `x509_required_certificate_policy_oids` for rail-specific X.509 XMLDSig
  signer policy gates. X.509 leaf certificates must now carry every configured
  certificate-policy OID before either direct leaf-key pins or validated
  trust-anchor chains can authorize the XMLDSig key; malformed configured OIDs,
  missing policy extensions, and wrong policy OIDs fail closed.
- Completed 2026-06-01: added profile-level CRL revocation enforcement for
  X.509 XMLDSig key-info chains. Profiles can require a fresh verified CRL via
  `x509_require_crl_revocation_check` and can supply pinned rail CRL material
  through `x509_crl_der_base64`; embedded `X509CRL` material is accepted only
  on the X.509 path. The verifier checks CRL DER parsing, issuer matching,
  issuer `cRLSign`, ECDSA/SHA-256 CRL signatures, freshness windows, duplicate
  CRL rejection, missing required CRLs, wrong issuers, expired CRLs, and revoked
  signer serials before using the X.509 leaf key.
- Completed 2026-06-01: added fail-closed X.509 name-constraint processing for
  trust-anchor-authorized XMLDSig key-info chains. The verifier now enforces
  permitted and excluded subtrees from constrained issuer certificates across
  subordinate signer certificates before using the leaf key, with local support
  for DNS, RFC822, URI-host, IP subnet, and directory-name forms and closed
  rejection for unsupported or invalid general names.
- Completed 2026-06-01: added profile-level OCSP revocation enforcement for
  X.509 XMLDSig key-info chains. Profiles can require fresh OCSP coverage via
  `x509_require_ocsp_revocation_check` and can supply pinned rail response
  material through `x509_ocsp_response_der_base64`; embedded `OCSPResponse` and
  `EncapsulatedOCSPValue` material is accepted only on the X.509 path. The
  verifier parses BasicOCSPResponse DER, binds SHA-256 CertID values to the
  signer and issuer, verifies issuer-signed and delegated ECDSA/SHA-256
  responders, enforces OCSPSigning EKU/digitalSignature key usage for
  delegated responders, checks producedAt/thisUpdate/nextUpdate freshness, and
  rejects missing, revoked, unknown, duplicate, stale, malformed, or unauthored
  responses before using the X.509 leaf key.
- Completed 2026-06-01: required delegated OCSP responder certificates in
  X.509 XMLDSig revocation paths to mark KeyUsage critical when authorizing
  `digitalSignature`. Otherwise-valid delegated responses whose embedded
  responder certificate carries non-critical digitalSignature KeyUsage now fail
  closed before OCSP coverage can satisfy the rail profile.
- Completed 2026-06-01: added X.509 path-length constraint enforcement for
  trust-anchor-authorized XMLDSig key-info chains. The verifier now evaluates
  BasicConstraints `pathLenConstraint` values across intermediate CAs and
  rejects a chain when a constrained root or intermediate authorizes more
  subordinate CA certificates than its policy allows.
- Completed 2026-06-01: required X.509 XMLDSig signer certificates to be
  end-entity certificates. The verifier now rejects signer leaves whose
  BasicConstraints extension is missing or CA:true before either direct public
  key pins or trust-anchor chains can authorize the key, with adversarial
  coverage for CA-capable signer certificates accepted by neither path.
- Completed 2026-06-01: added fail-closed unknown-critical X.509 extension
  handling for XMLDSig signer material. Critical extensions that the parser
  cannot decode or recognize now reject direct-pinned leaves, trust-anchor
  chains, and delegated OCSP responder certificates before any public key is
  accepted.
- Completed 2026-06-01: made X.509 signer certificate validity windows
  mandatory before direct public-key pins can authorize a key. Expired signer
  leaves now fail before direct-pin acceptance as well as on trust-anchor
  chains, with coverage for both paths.
- Completed 2026-06-01: added X.509 signer Extended Key Usage purpose binding
  for XMLDSig signer material. Signer leaves without EKU remain acceptable, but
  EKU-constrained leaves must allow `codeSigning`, `anyExtendedKeyUsage`, or
  the document-signing OID before either direct public-key pins or trust-anchor
  chains can authorize the XMLDSig key; incompatible server-auth-only signer
  leaves fail closed on both paths.
- Completed 2026-06-01: required X.509 XMLDSig signer certificates to mark
  KeyUsage critical when authorizing `digitalSignature`. Direct leaf public-key
  pins and trust-anchor chains now both reject signer leaves whose KeyUsage
  extension carries `digitalSignature` as a non-critical advisory extension.
- Completed 2026-06-01: added X.509 Authority Key Identifier / Subject Key
  Identifier binding for trust-anchor XMLDSig chains. When a subordinate
  certificate presents an AKI key identifier and the issuer presents an SKI, the
  identifiers must match before the trust-anchor path can authorize the leaf
  key; issuer-name/signature-valid chains with mismatched key identifiers fail
  closed.
- Completed 2026-06-02: added conservative required certificate-policy path
  continuity for X.509 trust-anchor XMLDSig chains. When a profile requires
  certificate policy OIDs, every intermediate CA below the pinned terminal
  anchor must carry all required OIDs or `anyPolicy`; generated chain tests
  cover matching, `anyPolicy`, missing, and unrelated intermediate policies.
- Completed 2026-06-02: fail closed on policy mappings, policy constraints,
  and inhibit-any-policy extensions in XMLDSig X.509 material until full RFC
  5280 policy-tree processing is implemented.
- Remaining ISO signature work is optional full RFC 5280 policy-tree processing
  if production profiles need to accept policy mappings, policy constraints, or
  inhibit-any-policy instead of rejecting those extensions in the supported
  subset.
- Completed 2026-06-01: tightened ISO idempotency so replayed Business
  Application Header `BizMsgIdr` values are rejected across different durable
  message identifiers, including after durable-store reload. Live-profile
  validation now also has regression coverage proving recorded metadata carries
  the exact reference-data snapshot checksum and that checksum changes when the
  loaded reference snapshot provenance changes.
- Completed 2026-06-02: tightened live-profile UETR admission and replay
  coverage. Present UETR values now need the canonical UUID hyphen layout and
  ASCII hex digits before profile metadata is produced; Swift CBPR+ coverage
  rejects missing and malformed UETRs, exercises padded/malformed direct
  validator inputs, and proves validated live-profile submissions still reject
  duplicate Business Application Header `BizMsgIdr` values and case-drifted
  duplicate UETRs across different durable message identifiers.
- Completed 2026-06-02: tightened inbound lifecycle reference handling for
  payment returns, cancellation requests, and securities settlement
  confirmations. `pacs.004`, `camt.056`, and `sese.025` payloads that carry
  conflicting original-message references now fail lifecycle id derivation and
  inbound application before any candidate original record is mutated.
- Completed 2026-06-02: tightened securities lifecycle durable identifiers and
  fixture coverage. BAH-wrapped `sese.023`, `sese.024`, and `sese.025`
  messages are now durably keyed by their transaction `TxId` with a message
  type prefix, while `BizMsgIdr` remains profile/idempotency metadata; this lets
  confirmations find the referenced `sese.023:<TxId>` record. Torii tests now
  wrap the checked-in `sese.023`/`sese.025` XML fixtures in AppHdrs, validate
  them through a securities CSD live profile with required reference datasets,
  apply lifecycle state, and reject unsupported version and document-root drift.
- Completed 2026-06-04: moved the default collateral substitution confirmation
  surface to the ISO `colr.012` family. Torii now exposes
  `/v1/iso20022/colr012`, the default generic and securities profiles advertise
  `colr.012.001.05`, the checked-in `colr.012` fixture validates through the
  generic profile and is durably keyed by `colr.012:<TxId>`, and the XSD
  manifest tracks the remaining official `colr.012.001.05` schema gap. The
  older `colr.007` parser/route remains as a legacy local compatibility path,
  not the production default; operator evidence must not rely on it.
- Completed 2026-06-01: broadened live-profile mismatch and lifecycle
  transition coverage. Swift CBPR+ validation now has negative tests for
  unsupported message-definition versions and business services, while
  `pacs.002`, `pacs.004`, `camt.056`, and `sese.025` lifecycle updates fail
  closed when the referenced durable record belongs to the wrong ISO family.
- Completed 2026-06-01: added XSD document-root admission for real ISO XML
  parsing. Each supported XML family now has a canonical `Document` child-root
  gate, and real XML with a missing or mismatched family root fails before
  field-level validation can materialize a message.
- Completed 2026-06-01: added live rail XSD/profile fixture coverage for the
  embedded Swift CBPR+, Fedwire Funds, SEPA SCT Inst, and securities CSD
  profiles. The fixture matrix now validates accepted `pacs.008`/`pacs.009`
  samples against required reference data, business services, message
  definition versions, reference snapshot metadata, and minor-unit policy, with
  adversarial wrong-service, wrong-version, and fractional-amount drift cases.
- Completed 2026-06-01: added offline Standards Editor generated MDR/XSD
  fixtures for `pacs.008.001.08` and `pacs.009.001.08` and bound them to the
  live rail profile matrix. Swift CBPR+, Fedwire Funds, SEPA SCT Inst, and the
  securities CSD profile now each validate at least one live profile payload
  whose namespace and `Document` child root are asserted against the checked-in
  XSD, with a root-drift negative case proving mismatched MDR roots fail before
  profile admission.
- Completed 2026-06-01: kept backward-compatible `trusted_public_key_sha256`
  and `trusted_certificate_sha256` profile aliases while normalizing them into
  the stricter `signature_public_key_sha256_pins` and
  `x509_trust_anchor_sha256_pins` verifier inputs.
- Completed 2026-06-04: bound durable ISO message JSON records to a
  deterministic `record_sha256` digest. Persisted records now carry a versioned
  digest over the record body, and reload rejects missing, malformed, or
  mismatched digests without rebuilding message status or replay indexes.
- Completed 2026-06-04: added a deterministic durable ISO audit index at
  `store_dir/audit/messages.index.json`. The index is sorted by message id,
  carries `index_sha256`, links each entry to the corresponding message file and
  `record_sha256`, and is regenerated from only valid records on reload so
  forged persisted files are excluded from the audit manifest.
- Completed 2026-06-04: exposed the durable ISO audit manifest through Torii at
  `GET /v1/iso20022/audit/messages`, backed by the same deterministic index
  builder used for `store_dir/audit/messages.index.json`, and added endpoint
  coverage for successful export plus disabled-bridge rejection.
- Completed 2026-06-04: added config-backed durable ISO store retention and
  compaction. Operators can set `store_retention_secs` or `store_max_records`;
  zero defaults retain all records. Compaction is independent from dedupe TTL,
  removes expired or oldest overflow records from memory and disk, clears replay
  indexes, and regenerates the audit manifest from survivors.
- Completed 2026-06-04: added the config-backed ISO external audit export spool.
  Operators can set `audit_export_dir`; each audit-index regeneration mirrors
  `messages.index.json` into that external directory and writes digest-addressed
  `.notary.json` preimages that bind `index_sha256`, source `store_dir`, record
  count, the embedded manifest, and `anchor_sha256`.
- Completed 2026-06-04: added `scripts/iso_audit_notary_adapter.py` for
  operator-side archival/notary publication. The adapter consumes
  `audit_export_dir`, verifies the anchor digest, embedded index digest,
  top-level `index_sha256`, digest-addressed filename, local
	  `messages.index.json` equality, duplicate-free audit records, and
	  record-count consistency before any network delivery. Non-empty anchors
	  must expose `store_dir/messages` record
  sources by default, and the adapter verifies every indexed persisted record
  body against its `record_sha256`, audit-index row metadata, and monotonic
  current status history before publication while anchor `store_dir` values
  reject whitespace, leading dashes, leading-dash path segments, backslashes,
  semicolon path parameters, empty path segments, and dot/parent path segments even when the local
  diagnostic `--allow-missing-record-sources` override is supplied. It rejects
  plaintext HTTP unless explicitly enabled for local
  tests, rejects endpoint URLs with credentials, params, query strings,
	  fragments, surrounding or embedded whitespace, or control characters, rejects
	  empty, zero, leading-zero, malformed, out-of-range, or explicit-default ports, non-canonical hosts,
	  invalid DNS labels, percent-escaped hosts, numeric-host/legacy-IPv4
	  spoofing, and IPv6 transition addresses embedding non-global IPv4 addresses,
	  rejects traversal, backslash, encoded-separator, encoded-semicolon,
	  encoded URL delimiters, encoded-percent,
	  percent-encoded control/space bytes, malformed percent escapes,
	  repeated URL path separators, or embedded-semicolon URL paths, rejects
	  duplicate publication endpoints before network delivery, treats remote
	  redirects as failed receipts without following them, and
	  requires bearer-token files to be regular non-symlink inputs capped at
	  8 KiB before decoding to exact UTF-8 values with no surrounding
	  whitespace, embedded whitespace, or control characters, and
	  requires the export directory, `latest.notary.json`, the digest-addressed
	  anchor peer, `messages.index.json`, and clean `store_dir/messages` record
	  sources to be non-symlink regular directories/files, caps each audit export
		  JSON input at 64 MiB, requires positive finite `--timeout-secs` and
		  positive integer `--response-limit-bytes`, and writes bounded
		  per-endpoint receipts without persisting token material, redacting
		  secret-looking remote response previews or transport errors before persistence. Receipt
		  output directories and receipt leaves are preflighted before publication,
		  reject control characters, whitespace, leading-dash segments,
		  backslashes, semicolon parameters, empty segments, dot/parent
		  traversal, symlinked existing ancestors, and hard-linked, symlink, or
		  non-regular targets, and are written via exclusive same-directory
		  owner-private temporary files with bounded digest-derived names that are
		  descriptor-rechecked, fsynced, and atomically replaced where available.
- Completed 2026-06-04: added `scripts/iso_rail_gateway_adapter.py` for
  operator-side live rail file-drop ingress. Each XML payload requires a JSON
  sidecar with `message_type`, explicit `profile` by default, and
  `payload_sha256`; the adapter verifies the sidecar before posting to the
	  matching Torii ISO endpoint, rejects plaintext HTTP unless explicitly enabled
	  for local tests, rejects Torii base URLs with credentials, params, query
	  strings, fragments, surrounding or embedded whitespace, or control
	  characters, overlong URLs or DNS hosts, localhost/local-private IP
	  literals, known local/private rebinding hostnames, or IPv6 transition
	  addresses embedding non-global IPv4 addresses, rejects malformed, out-of-range,
	  empty, zero, leading-zero, or explicit-default ports and non-canonical hosts, invalid DNS labels, percent-escaped hosts, and
	  numeric-host/legacy-IPv4 spoofing, rejects traversal, backslash, encoded-separator,
	  encoded-semicolon, encoded URL delimiters, encoded-percent, percent-encoded control/space bytes, malformed percent
	  escapes, or embedded-semicolon URL paths, keeps explicit `--message` paths
	  inside the declared inbox, rejects explicit `--message` path and discovered
		  XML leaf whitespace, leading-dash segment, backslash, semicolon,
		  empty-segment, or dot/parent segment smuggling before reads, rejects duplicate
		  payload digests or duplicate `rail_message_id` values within one gateway run before network delivery, rejects sidecar `profile` and `rail_message_id`
			  values that are explicitly `null` or carry surrounding whitespace,
			  embedded whitespace, or control characters, rejects non-canonical sidecar
			  profile IDs, rejects sidecar `rail_message_id` values that are longer
			  than 128 characters or are not canonical ASCII rail-message identifiers,
			  rejects unknown sidecar fields, bounds sidecar JSON before parsing,
			  rejects legacy `colr.007`
	  drops unless `--allow-legacy-colr007`
	  is set for local diagnostics, requires bearer-token files to be regular
	  non-symlink inputs capped at 8 KiB before decoding to exact UTF-8 values
	  with no surrounding whitespace, embedded whitespace, or control
	  characters, rejects symlinked XML payload or sidecar files, rejects
		  symlinked inbox roots, requires positive finite `--timeout-secs`,
		  requires positive integer `--max-payload-bytes` and
		  `--response-limit-bytes`, treats remote redirects as failed receipts
		  without following them, preserves explicit
		  `--message` leaves for regular-file checks, and writes bounded
		  submission receipts without persisting token material, redacting
		  secret-looking remote response previews or transport errors before persistence. Receipt output
		  directories and receipt leaves are preflighted before Torii submission,
		  reject control characters, whitespace, leading-dash segments,
		  backslashes, semicolon parameters, empty segments, dot/parent
		  traversal, symlinked existing ancestors, and hard-linked, symlink, or
		  non-regular targets, and are written via exclusive same-directory
		  owner-private temporary files with bounded digest-derived names that are
		  descriptor-rechecked, fsynced, and atomically replaced where available.
- Completed 2026-06-04: added `scripts/iso_operator_receipt_verify.py` as a
  read-only canary gate for rail/notary adapter receipts. It recomputes receipt
  digests, requires successful 2xx receipts by default, rejects plaintext HTTP
  evidence unless explicitly enabled for local tests, rejects leaked
  authorization/token material and receipt endpoint URLs with credentials,
  params, query strings, fragments, malformed hosts, surrounding or embedded
	  whitespace, empty/zero/leading-zero/malformed/default ports, non-canonical hosts, or control
	  characters, localhost/local-private IP literals, known local/private
	  rebinding hostnames, IPv6 transition addresses embedding non-global IPv4
	  addresses, invalid DNS labels, percent-escaped hosts, numeric-host
	  or legacy-IPv4 spoofing, plus traversal, backslash, encoded-separator,
	  encoded-semicolon, encoded URL delimiters, encoded-percent,
	  percent-encoded control/space bytes, malformed percent escapes, or
	  embedded/encoded-semicolon/encoded-delimiter/repeated-separator URL paths, can cross-check referenced XML or notary anchor
	  source files, closes the raw receipt schemas per receipt kind plus notary
	  anchor/audit-index source schemas, including duplicate-free nested audit records, binds
  audit record filenames to `sha256(message_id).json`, binds each indexed
  `record_sha256` to the persisted `store_dir/messages` body when source files
	  are required or locally available, rejects row/source metadata drift and
	  persisted-state-derived `pacs002_code` or status-history timestamp drift,
	  binds endpoint digests to recorded endpoint URLs, requires timezone-aware adapter timestamps that do not
  require trimming, enforces `ok`/`status_code` consistency,
	  validates bounded response metadata, requires rail `xml_path` values to
	  point at `.xml` leaves, cross-checks rail sidecars against the
	  adapter's `xml_path + .json` convention and receipt metadata, requires notary
	  `anchor_path` values to keep the `latest.notary.json` or digest-addressed
	  `anchors/<index_sha256>.notary.json` shape even when source files are not
	  required, rejects raw notary `anchor_path` and `store_dir` values, raw rail
	  receipt `message_type`, `xml_path`, and
	  `sidecar_path` values that carry whitespace, control characters,
		  leading dashes, leading-dash path segments, backslashes, semicolon path
		  parameters, empty path segments, or dot/parent path segments, plus receipt and source-sidecar rail
		  `profile`/`rail_message_id` values when they carry surrounding whitespace or
		  embedded whitespace or control characters, rejects non-canonical receipt or
		  source-sidecar profile IDs, rejects overlong or non-canonical ASCII
		  `rail_message_id` values, caps receipt JSON at 4 MiB, notary source
		  JSON at 64 MiB, rail source XML at 4 MiB, and source-sidecar JSON at
		  16 KiB before parsing or hashing, replays digest-addressed notary-anchor and
  `messages.index.json` checks while rejecting symlinked or non-regular notary
  anchor/index peers and rail XML/sidecar files, rejects legacy `colr.007` rail
  source files unless `--allow-legacy-colr007` is set for local diagnostics,
  rejects symlinked receipt archive directories before discovery, rejects
  repeated receipt paths or copied receipts with duplicate `receipt_sha256` values, and
  emits a digest-bound verifier summary with per-receipt file paths,
  `receipt_sha256` values, and policy flags.
- Completed 2026-06-04: added `scripts/iso_operator_canary.py` as the generic
  provider canary runner. The runner consumes a strict JSON runbook capped at
  64 KiB with explicit provider/environment labels, executes the rail file-drop adapter,
	  audit notary adapter, and receipt verifier as subprocesses, rejects unknown
	  runbook keys, rejects surrounding whitespace and control characters in
	  runbook strings, rejects present `null` optional path and numeric limit
		  fields instead of silently applying defaults, rejects embedded whitespace,
		  leading-dash path segments, backslashes, semicolon path parameters, empty
		  path segments, and dot/parent segments in runbook paths before expansion, keeps relative paths inside
	  the runbook directory while preserving final path leaves for child script
	  symlink/file-boundary checks, rejects
	  endpoint URLs with credentials, params, query strings, fragments, embedded
	  whitespace, malformed bracketed hosts, overlong URL strings, or DNS hosts
	  longer than 253 characters, localhost/local-private IP literals, or known
	  local/private rebinding hostnames, legacy IPv4 numeric notation, or IPv6
	  transition addresses embedding non-global IPv4 addresses,
	  rejects empty, zero, leading-zero, malformed, out-of-range, or explicit-default ports,
	  rejects non-canonical hosts, invalid DNS labels, percent-escaped hosts,
	  numeric-host spoofing, percent-escape smuggling, and smuggled URL paths
	  including encoded semicolon parameters and encoded URL delimiters,
	  rejects duplicate endpoint lists, duplicate explicit receipt paths or receipt
  directories, and shared stage receipt directories, verifies generated
  receipts by default with source-file cross-checks, redacts bearer-token file
  arguments in the summary, bounds each child stage with positive finite
  `--stage-timeout-secs`, records `timed_out` for killed children, drains child
  stdout/stderr through the configured preview cap instead of retaining
  unbounded output, supports
  `--require-explicit-policy` so production runbooks must spell out every
  policy boolean and the summary records that proof, with regression coverage
  over the rail, notary, and verifier policy-boolean surface, and writes a single
  bounded JSON summary suitable for CI or operator evidence archives. Summary
	  output paths are preflighted before subprocess stages, reject control
	  characters, whitespace, leading-dash segments, backslashes, semicolon
	  parameters, empty segments, dot/parent traversal, symlinked existing ancestors, and
	  hard-linked, symlink, or non-regular targets, and are written via
	  exclusive same-directory owner-private temporary files with
	  bounded digest-derived names that are descriptor-rechecked, fsynced, and
	  atomically replaced where available.
  `--plan-only` validates runbooks and prints redacted child commands without
  contacting Torii or notary endpoints.
- Completed 2026-06-04: added checked-in ISO operator canary runbook templates
  under `fixtures/iso20022/operator_canary/` for Swift CBPR+, Fedwire Funds,
  SEPA SCT Inst, and securities CSD profile families. The script tests validate
  that each template plans successfully without network access.
- Completed 2026-06-04: added `scripts/iso_trust_bundle_verify.py` as an
  offline XMLDSig/XAdES trust-bundle preflight for operator rail PKI packages.
  It caps bundle JSON at 64 MiB before parsing, verifies canonical lowercase
  profile IDs, known ISO rail IDs, canonical
  lowercase nonzero SHA-256 pins, digest-bound base64 DER envelopes with a
  pre-decode 1 MiB DER-size cap and lightweight semantic shape checks for X.509 certificates,
  X.509 CRLs, and OCSPResponse wrappers, duplicate material, contradictory
  trust/revocation pins, explicit CRL/OCSP revocation policy booleans, required
  CRL/OCSP material, HTTPS provenance without credentials, params, query
	  strings, fragments, malformed bracket syntax, control characters, surrounding
	  or embedded whitespace, empty/zero/leading-zero/malformed/out-of-range/default ports, localhost, or
	  local/private IP literals, non-canonical hosts, invalid DNS labels,
		  percent-escaped hosts, numeric-host/legacy-IPv4 spoofing, IPv6
		  transition embedded-IPv4 smuggling, percent-escape smuggling,
		  smuggled URL paths including encoded semicolon parameters, encoded URL delimiters, and repeated separators, required provenance URL,
		  required source authority/version values, and timezone-aware
	  non-future retrieval timestamp fields,
  repeated-path/copied-bundle/duplicate
  profile ID rejection, duplicate `bundle_sha256` rejection, unique DER labels
  per material class, DER-object `sha256` values that fail when present as
  `null` or another non-string value, omitted absent labels in trust summaries,
  archived-summary `label: null` rejection in the evidence gate, and
	  secret-looking fields before emitting Torii profile trust override JSON.
	  Profile override emission now also rejects local-audit `--allow-record-only`
	  or `--allow-insecure-source-url` modes and placeholder source provenance
	  (`placeholder`, `replace-before-production`, or `example.invalid`), leaving
	  those bundles summary-only until real rail source metadata is supplied.
		  It also requires an explicit `--max-source-age-days` freshness budget and
		  leaves stale source packages summary-only instead of writing profile
		  overrides. The digest-bound trust summary records that budget so evidence
		  and readiness can reject omitted, malformed, or weaker source-freshness
		  policy, and recompute whether `profile_json_emittable` still matches the
		  archived source evidence.
- Completed 2026-06-04: added checked-in trust-bundle templates under
  `fixtures/iso20022/trust_bundles/` for Swift CBPR+, Fedwire Funds, SEPA SCT
  Inst, and securities CSD profile families. The templates use synthetic DER
  envelopes for CI/schema validation only, require `--allow-synthetic-der`,
  cannot emit profile override JSON, and must be replaced with current rail PKI
  material before production.
- Completed 2026-06-04: added `scripts/iso_operator_evidence_verify.py` as an
	  offline production evidence gate for ISO operator archives. The verifier
	  recomputes canary and trust summary digests, requires successful
	  rail/notary/verify canary stages plus digest-bound receipt-verifier JSON with
	  positive rail/notary receipt evidence, duplicate-free receipt-kind lists,
	  unique canonical `*.receipt.json` receipt paths, unique per-receipt digests,
	  per-receipt `ok=true` plus 2xx `status_code` success metadata, and
	  kind-specific notary anchor/index/count or rail message/profile/payload
	  metadata,
		  complete child-process stdout/stderr previews for every executed canary
		  stage, rejects timed-out stages,
		  and timeout-bounded direct receipt archive verification covering canary
		  receipt digests, receipt filenames, receipt kinds, successful status
		  metadata, kind-specific compact receipt metadata, and explicit rejection
		  of rail default-profile fallback unless the local override is recorded by
		  the receipt verifier,
	  requires exact expected `--provider` and `--environment` CLI context and
	  records that context in the digest-bound evidence policy for readiness
	  rechecking, requires explicit freshness budgets for canary, trust-summary,
	  and trust-source evidence while recording them in the evidence policy,
	  preserves compact trust profile JSON emission booleans and a digest
	  recomputed from archived profile overrides, rejects profile-emittable drift
	  and emitted-but-not-emittable contradictions against the archived trust
	  source policy, `bundle_sha256`, required
	  source authority/version plus source URL/retrieval provenance, the trust
	  verifier's `max_source_age_days` emission budget,
	  revoked-certificate pin count, certificate-policy
	  OID count, and compact trust-anchor/revoked/CRL/OCSP DER proof digests and
	  byte lengths for the final rollup,
		  rejects compact canary config paths, receipt paths, and child
		  receipt-directory arguments with embedded whitespace, leading-dash path
		  segments, semicolon path parameters, empty segments, raw backslashes, or traversal segments,
	  rejects stale digest-correct archive inputs, rejects repeated or copied
  canary/trust summaries by path and `summary_sha256`,
  requires canary summaries to prove they were generated with
  `--require-explicit-policy`, rejects duplicate compact receipt paths/digests,
  duplicate archived trust profile IDs, and bundle digests across summaries with
  label-only diagnostics, rejects non-canonical archived trust profile IDs or unknown
  rail IDs, requires each canary rail receipt profile to have matching compact
  trust material for the same profile ID and environment, with same-rail binding
  for built-in rail-named profiles, rejects forged trust profile overrides whose id/rail/policy,
  pin/OID/CRL/OCSP counts, canonical OIDs, DER summary digests, DER byte
  lengths, bounded canonical base64 DER SEQUENCE blobs, or trusted/revoked pin
  overlap no longer match the trust-bundle verifier output,
	  rejects duplicate JSON object keys, non-standard `NaN`/`Infinity` JSON
	  constants, and lone UTF-16 surrogate escapes across raw canary, trust,
	  receipt, XSD, evidence, readiness, embedded receipt-verifier stdout, and direct archive
		  receipt-verifier stdout inputs before semantic validation, and rejects
	  symlinked existing ancestors plus symlink or non-regular leaves for canary
	  runbooks, trust bundles, evidence/readiness summaries, XSD manifests,
	  profile catalogs, schema files, XML fixtures, and receipt archive
	  directories before digest, provenance, discovery, or policy checks run,
	  opens those checked file inputs through no-follow file descriptors where
	  available, rejects raw CLI artifact path smuggling for live rail inbox
	  roots, live notary export roots, rail/notary bearer-token files, canary
	  configs, trust bundles, XSD manifests/profile catalogs, receipt
		  files/directories, canary/trust summaries, and XSD/evidence summaries
		  before argparse `Path` normalization or file discovery, rejects
		  non-positive or non-finite live rail/notary timeout values and
		  non-positive live adapter byte caps before local reads or network
		  delivery, caps archived
  canary/trust and XSD/evidence summary JSON inputs at 4 MiB before parsing,
  caps direct receipt-verifier stdout/stderr at 4 MiB before JSON parsing,
	  rejects receipt,
	  summary, and emitted profile-override output paths when they contain
	  control characters, whitespace, leading-dash segments, backslashes,
	  semicolon parameters, empty segments, dot/parent traversal, symlinked existing ancestors,
	  or are hard-linked, symlink, or non-regular targets, then atomically replaces targets from owner-private
	  descriptor-checked temporary files with bounded digest-derived names,
  rejects plan-only or dry-run canaries, insecure HTTP evidence,
  default-profile fallbacks, legacy `colr.007` local overrides, unredacted
		  bearer-token paths, secret-looking child output, smuggled, whitespace-bearing,
		  empty-port, malformed-port, non-canonical-host, invalid-host-label, overlong-url,
		  overlong-host, percent-escape,
		  numeric-host/legacy-IPv4-spoofed, IPv6-transition embedded-IPv4,
		  repeated-separator, or traversal-bearing trust-source URLs,
		  placeholder trust-source authority/version metadata and `example.invalid`
		  source provenance,
  missing/malformed/future trust-source retrieval timestamps,
  missing/malformed/future or padded trust-summary `verified_at` timestamps, smuggled
  child command endpoint URLs including localhost/local-private IP literals,
  known local/private rebinding hostnames, legacy IPv4 numeric notation, or IPv6
  transition addresses embedding non-global IPv4 addresses,
  local-only child command flags in either `--flag` or `--flag=value` form, including the notary adapter's
  `--allow-missing-record-sources` diagnostic override, unsupported child
  command flags outside the expected rail/notary/receipt-verifier CLI surfaces,
  duplicate singleton child command flags, boolean child command flags using
  attached or separate values, non-positive or non-finite numeric child command
  flag values, non-canonical child command path values, control-bearing or
  whitespace-padded child command entries, missing required child command
  inputs, whitespace-padded strings or paths, non-canonical canary
  rail/notary `receipt_dir` values,
	  rail/notary `receipt_dir` values that do not match the child command's single
	  `--receipt-dir`, verify-stage commands that omit generated rail/notary
	  receipt directories, control-bearing or whitespace-padded
	  provider/stage/receipt-kind/trust-profile identity strings, non-canonical
	  canary runbook `config_path` values, unknown upstream canary/receipt/trust
	  summary fields, synthetic trust DER, record-only trust policy, and trust
	  summaries that did not emit profile override JSON before an archive is
	  accepted as production evidence. Canary command redaction also handles
  `--bearer-token-file=<path>` in addition to the separated argument form.
- Completed 2026-06-04: added `fixtures/iso20022/xsd/fixture_manifest.json`
  and `scripts/iso_xsd_fixture_verify.py` as an offline structural preflight for
  checked-in ISO XSD/XML fixtures. The verifier checks schema target
  namespaces, `Document` payload roots, XML fixture namespaces and payload
  roots, canonical lowercase ISO message definition ids, schema path
  containment under the manifest tree, fixture path containment under the ISO
  fixture tree, manifest duplicates/path escapes, manifest schema/fixture path
  and fixture schema-reference whitespace or semicolon smuggling, duplicate XML
  fixture SHA-256 values, optional `xmllint --nonet` XML schema validation for
  schema-backed fixtures, and digest-bound summaries while making reviewed
  missing-schema fixture exceptions explicit. All
  checked-in payment XSDs now have standalone XML fixtures and validate against
  their checked-in XSDs, so the `--require-fixture-for-schema` strict flag
  passes; the schema-backed strict flag still rejects the current
  official-package gaps until the remaining securities/collateral/legacy-return
  XSDs are checked in. `--require-profile-schema-backed-versions` now uses
  the default `DEFAULT_PROFILES_JSON` catalog when no `--profile-catalog`
  override is supplied, so the release gate fails directly on the current
  profile-advertised schema gaps.
  Optional manifest/profile fields are optional only when
  omitted; present `null` reviewed reasons, trust/revocation material lists,
  booleans, numeric caps, business-service arrays, or amount minor-unit arrays
  fail before a digest-bound XSD summary can be emitted. Required and optional
  manifest/profile-catalog strings now reject ASCII control characters before
	  summary emission, including reviewed gap reasons. Readiness also rejects
	  archived reviewed gap reasons that are present but empty or non-string
	  instead of treating them as absent, blocks schema-backed archived fixtures
		  that still carry a missing-schema reason, and checked-in XSD source
		  provenance, manifest schema, fixture, fixture schema-reference, and
		  archived profile-catalog paths reject embedded whitespace, leading-dash
		  path segments, or semicolon path parameters before summary emission and during readiness rechecks.
- Completed 2026-06-04: added `scripts/iso_production_readiness.py` as the
  aggregate offline ISO release gate. It verifies digest-bound XSD fixture and
  operator evidence summaries, requires strict schema-backed/fixture-backed XSD
  proof by default, rejects non-production evidence policies, provider or
  environment drift, missing rail/notary/verify canary stages, missing
  rail/notary receipt kinds, missing or weak direct receipt-archive
  verification, direct archive receipts unrelated to any canary receipt summary,
  unsupported compact receipt entry kinds, copied compact receipt paths or
  digests reused across canary summaries, failed or status-mismatched compact
  canary/archive receipt entries, stripped or cross-kind compact receipt
  metadata, archive/canary compact receipt status or metadata drift for the
  same receipt digest, legacy `colr.007` local overrides,
  canary/trust/receipt/profile material replayed across evidence summaries,
  omitted XSD strict flags,
  XSD summaries produced without XML schema validation,
  inconsistent digest-bound XSD schema/fixture arrays, duplicate XSD schema or
  fixture evidence digests, XSD schema/fixture material replayed across compact
  summaries, non-canonical or message-id-mismatched schema
	  paths, leading-dash path tokens or segments, non-XML, absolute, empty-segment, dot-segment, or non-leading-parent
  fixture paths, schema-reference drift, unknown XSD summary fields, forged or
	  non-canonical missing-schema/schema-only reviewed gap lists and reason
	  strings, forged schema-only flags/reasons, stale missing-schema reasons on
	  schema-backed fixtures, forged
  profile-catalog missing-version lists,
  omitted evidence or nested receipt-summary policy flags, archived trust
  summaries with omitted policy/profile revocation flags, omitted planned-stage
  `dry_run` flags, omitted evidence status booleans, omitted whole XSD or
  evidence input summaries, omitted explicit release `--provider` or
  `--environment` context, omitted explicit freshness budgets, stale
	  digest-correct XSD/evidence/canary/trust summaries, omitted or drifted
			  evidence policy context, omitted or weaker evidence freshness policy fields,
			  omitted, malformed, or release-weaker compact trust source freshness budgets,
		  omitted or malformed compact trust source authority/version provenance,
		  omitted, malformed, placeholder, overlong-url, or overlong-host compact trust source provenance, stale compact trust
	  source retrieval timestamps, profile-emittable drift or emitted-but-not-emittable
	  contradictions against compact trust source policy, omitted canary
	  explicit-policy proof, repeated or
	  copied XSD/evidence summaries, missing or non-canonical compact canary
	  runbook `config_path` values, compact canary/trust summary paths, canary
		  config paths, and receipt paths with embedded whitespace, leading-dash
		  path segments, semicolon path parameters, empty segments, raw backslashes, or traversal segments,
	  whitespace-padded compact strings or paths, unknown compact evidence fields,
  repeated or copied compact canary/trust summaries, nested receipt-summary
  tampering, non-canonical compact receipt paths, duplicate receipt paths or
  receipt digests, weak trust profiles, duplicate compact trust profile IDs or
  bundle digests across trust summaries, non-canonical compact trust profile IDs or unknown rail IDs,
  missing or malformed compact trust `bundle_sha256`,
  record-only trust policy,
  disabled CRL/OCSP revocation checks, and missing required revocation
  material, omitted revoked-certificate or certificate-policy compact trust
  counts, omitted or count-drifted compact DER proof fields, mismatched trust
  `verified_bundles`/profile counts, missing compact
  canary/trust source paths, malformed compact canary/trust source paths,
  non-canonical compact canary/trust summary digests, and missing,
  control-bearing or whitespace-padded compact identity strings, timezone-less,
  or future
  XSD/evidence/trust `verified_at` timestamps, malformed or reversed canary
  `started_at`/`finished_at` windows,
  missing or out-of-window compact `stage_windows`, overlapping stage
  timelines, name-mismatched or reordered compact stage windows, and emits a
  digest-bound blocker report for valid but not-yet-production summaries.
  Compact canary stage names must also be unique, limited to the production
  stages, and ordered as rail/notary/verify.
- Completed 2026-06-04: hardened live securities lifecycle profile admission
  against local reference snapshots. `sese.023`/`sese.025` profile validation now
  rejects syntactically valid but unmapped settlement instrument ISIN/CUSIP
  values, inactive or unknown place-of-settlement MICs, and unmapped delivering
  or receiving party BICs before a durable settlement lifecycle record can be
  accepted.
- Completed 2026-06-04: gated live `securities-csd` `sese.023` ledger
  instruction admission on configured CSD venue, securities settlement-account,
  and cash-leg crosswalk snapshots. The gate now rejects missing snapshots,
  incomplete rows, party/account mismatches, and unknown cash-leg currencies
  before durable lifecycle recording, with checked-in sample snapshot schemas
  under `fixtures/iso_bridge/`.
- Broaden XMLDSig/XAdES fixture coverage beyond the current local fixture set,
  including full certificate-chain fixtures and official rail/profile-specific
  trust-anchor packages that replace the synthetic trust-bundle templates and
  emit production profile override JSON with digest-bound `profile_json_sha256`
  evidence.
- Run provider-specific production canaries for the selected archival/notary
  vendors using `scripts/iso_operator_canary.py`, pass the archived summaries
  and receipt files through `scripts/iso_operator_evidence_verify.py`, retain
  the accepted evidence summary and receipts, include the accepted evidence in
  `scripts/iso_production_readiness.py`, and document any vendor-specific
  authentication, SLA, or response evidence required by the production runbook.
- Run provider-specific live gateway canaries for selected
  SWIFT/Fedwire/SEPA/CSD operator integrations using
  `scripts/iso_operator_canary.py`, pass the archived summaries and receipt
  files through `scripts/iso_operator_evidence_verify.py`, retain the accepted
  evidence summary and receipts, include the accepted evidence in
  `scripts/iso_production_readiness.py`, and document rail-specific file-drop,
  retry, and acknowledgement handling.
- Add redistributable official MDR/XSD fixture coverage for the remaining
  profile-advertised gaps beyond the current schema-backed payment and
  cancellation corridor. `pacs.004.001.09` is now checked in and validated;
  remaining blockers include `pacs.002.001.12`, `pacs.008.001.10`, and
  `pacs.009.001.10` (available public candidates inspected so far carry
  restricted redistribution terms and are now recorded as blocked sources in
  the fixture manifest) plus the securities and collateral lifecycle packages.
  Make the strict
  `scripts/iso_xsd_fixture_verify.py` schema-backed release flag
  (`--require-schema-backed-fixtures`) pass,
  make the aggregate `scripts/iso_production_readiness.py` gate pass without
  diagnostic overrides, and keep broadening Torii tests for
  additional live-rail profile edge cases beyond the current family-mismatch,
  conflicting-reference, BAH securities-linking, and collateral-substitution
  guards.

## Soracles follow-ups

- Add the off-chain/runtime leader scheduler and pacemaker automation for
  provider fetches and manual aggregate replacement. The MVP keeps deterministic
  committee/leader derivation and quorum checks, but does not yet schedule
  leaders at runtime.
- Add provider rating weights and governance-driven reputation adjustments once
  enough provider stats are available from live feed history. Current
  aggregation remains equal-weight median/percentile with deterministic
  provider counters.

## FASTPQ GPU acceleration follow-ups

- Evaluate whether to promote the low-level Poseidon fused column+parent kernel
  from parity-only coverage to the production hot path. CUDA and Metal parity
  evidence for the current high-level column + Merkle-pair GPU path is now
  recorded in `status.md`; acceptance for a low-level hot-path promotion still
  requires a fresh Izanami gate/profile showing a real throughput improvement
  over that high-level path, with scalar CPU remaining the authoritative
  fallback for every mismatch or dispatch error. No CUDA-specific FASTPQ proof,
  parity, benchmark, or release-comparison task remains open here.

## Sumeragi vNext consensus replacement

- Optimize from the cap `1096` / pipeline `250ms` 20k liveness baseline toward
  higher applied throughput while preserving the hard 2-3s consensus cadence
  gate. The current confirmed 300s stable point is
  `dist/izanami-liveness-matrix-20k-cap1096-p250-pi5-soak-300s-20260511-074409`:
  scan multiplier `32`, collectors/redundant-send `3/3`, backup RBC on, all
  `6,000,000` submissions accepted, strict height `126`, zero view changes,
  runner p95 `2523ms`, parsed peer p95 `2.899s`, max peer gap `3.833s`, full
  detached merge (`1096/1096`, fallback `0`), and `453.13` committed TPS.
  Higher rows are rejected for now: cap `1100`/`250ms` with `3/3` collectors
  failed the parsed peer p95 gate at `3.071s`, cap `1100`/`250ms` with `4/4`
  collectors still failed at `3.022s` with lower committed throughput, cap
  `1104`/`250ms` failed the parsed peer p95 gate at `3.054s` under the finer 5s
  progress monitor, cap `1120`/`250ms` failed both runner and parsed p95 gates,
  cap `1120`/`300ms` was only a runner-gate near miss, and the older cap
  `1312` 120s pass failed the 300s soak. Next, target DA/precommit and
  peer-gap tail reduction before trying to raise cap again. Accept a result
  only if the runner gate, parsed peer p95 gate, zero-view-change requirement,
  and detached-merge counters remain green. Keep backup-on as the default
  recovery posture and use backup-off only as an explicit experiment row. The
  4,096, 8,192, and 16,384 cap experiments already proved that much larger
  blocks are not the next fix without reducing DA/RBC/QC/application tail
  latency and queue-drain cost. Keep the simple-transfer batch path guarded by
  exact trigger-filter matching so per-transaction transcript, event, trigger,
  and rejection semantics remain intact.
- Treat 20k committed TPS as a separate throughput goal from 20k ingress
  liveness. At 2-3s blocks, the current safe cap can only commit hundreds of
  transactions per second; reaching 20k committed TPS requires safe payloads in
  the tens of thousands of transactions per block, equivalent deterministic
  parallel execution, or both. Use the matrix runner for every optimization
  step and require the consensus liveness gate to stay green before accepting a
  higher-throughput result.
- Keep hardening the actor-owned vNext round state now that the standalone
  runtime reactor boundary is gone. vNext control frames, body-backed proposal
  acceptance, DA/RBC availability handoffs, timeout ticks, validation worker
  dispatch/start/result, proposal-backed validation gates, validation
  accept/reject/defer handling, re-chain/view-change aggregation, sidecar
  replay, and commit-persistence completion now run directly through `Actor`.
  Block-sync BlockCreated recovery now also uses named payload-only,
  requested-payload, signed-quorum, and commit-evidence recovery modes instead
  of broad stale/authoritative/revival bypass booleans. The remaining work is
  to delete any legacy cooperative commit sweep paths that become redundant
  once the actor-owned vNext state has equivalent model and integration
  coverage.
- Finish auditing chain-order hash and `rechain_seq` binding in deferred
  vote/QC caches, signer-tally/cache keys, and evidence replay paths used by
  the replacement shell. Vote/QC preimages, precommit signer history,
  block-sync-derived QCs, validator-checkpoint sidecars, raw/deferred vote
  caches, and vote/QC verifier cache keys now carry the selected binding.
- Reconstruct vNext chain order from committed/replayed re-chain and
  view-change certificates during catch-up. The live actor now keeps a bounded
  in-memory certificate journal, persists matching certificates into committed
  Kura roster sidecars, reloads those durable sidecars into outgoing
  `BlockSyncUpdate` payloads, and replays inbound sidecars before vote/QC
  processing. Vote/QC chain-order binding checks also hydrate matching durable
  sidecars from Kura before rejecting a `chain_order_hash`/`rechain_seq`
  mismatch. The remaining open work is to broaden catch-up model and
  integration coverage around restarted-peer durable sidecar replay.
- Add model and integration coverage for slow validation, queue saturation,
  malicious accusers, head failure during re-chain, NPoS stake-quorum
  quarantine edges, and DA/RBC loss during re-chain.

## Validation corridor

- Carry the Sumeragi NPoS/permissioned QC and VRF hardening through the next
  full workspace corridor.
  - A 2026-05-05 workspace rerun exposed three remaining
    `consensus_and_da` cases after the UAID replay/checkpoint fixes:
    stale evidence persistence, NPoS baseline timing, and late VRF reveal
    penalty recovery. The stale-evidence and NPoS performance focused reruns
    are green after the Torii horizon filter and baseline budget update. The
    late-reveal path now has code-level fixes for VRF vote-queue routing and
    deferring committed-block catch-up until after VRF metadata handling,
    epoch-record hydration before reveal validation, stale pending-seal
    retention, and external Torii VRF metadata gossip. The focused core units
    for those paths are green. The remaining integration blocker is now a
    separate four-peer NPoS/DA liveness stall: the late reveal is accepted in
    Sumeragi status, but the network repeatedly stalls at height 4 with RBC
    READY/DELIVER data waiting on missing INIT/chunk state before the pending
    VRF seal can be committed. Fix that h4 DA/RBC stall, then rerun
    `sumeragi_randomness::npos_late_vrf_reveal_clears_penalty_and_preserves_seed`
    as the final persistence gate.
  - Focused commit, block-sync, VRF, QC-validation, roster-selection, Torii VRF
    OpenAPI/parser, and data-model consensus roundtrip tests are green as of
    2026-05-02 with `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-verify`.
  - Additional Sumeragi/DA adversarial coverage is green as of 2026-05-02 with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor` for the debug
    witness-root unit, witness-corruption recovery, chunk-drop recovery,
    Kura-eviction DA rehydration, and block-body DA rehydration focused
    reruns. The remaining broad-run Sumeragi DA payload-loss case is also green
    as of 2026-05-03 with the same target dir.
  - NewView QC `highest_qc` binding, exact local-vote `highest_qc` and
    parent/post-root matching, non-NewView `highest_qc` rejection, and
    same-highest aggregate formation are green as of 2026-05-03 with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest` for the NPoS
    aggregate-only substitution regression, the `new_view_highest` focused
    slice, and the stale/future NewView QC formation regressions. The same
    target now also covers commit/checkpoint missing-PoP rejection, block-sync
    QC validation with commit-phase enforcement, commit-certificate roster
    validation, checkpoint roster validation, validation telemetry reason
    labels, and the permissioned/NPoS aggregate-fallback quorum checks.
    Embedded commit-QC roster anchoring is green as of 2026-05-04 in the same
    target for both the malicious shrink-roster rejection and the valid
    stale-cache bootstrap path; the embedded-roster missing-PoP rejection is
    green in that same filter. NPoS block-sync roster selection now also has
    focused coverage for carrying a locally resolved stake snapshot when the
    incoming QC/checkpoint hint omits one.
  - The ZK-confidential localnet submit helper has been hardened for startup
    transport jitter and wrapped policy rejections. The classifier/retry-budget
    tests plus disabled shield/unshield localnet regressions are green as of
    2026-05-03 with `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target`. The full
    serial `consensus_and_da` target is also green in the same target dir:
    `250` passed, `0` failed, `6` ignored. Focused strict clippy over
    `iroha_core`, `iroha_torii`, `iroha_test_network`, and the
    `consensus_and_da` test target is also green in that target dir.
  - Focused `cargo clippy -p iroha_core -p iroha_data_model -p iroha_torii -p
    irohad --all-targets -- -D warnings` is green as of 2026-05-02 with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-clippy`.
  - Full workspace all-target clippy is green as of 2026-05-03 with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target`.
  - The broad workspace test rerun reached `events_and_triggers` after passing
    `consensus_and_da` and `core_api`; the exposed by-call trigger fixture and
    subscription time-trigger billing failures are repaired as of 2026-05-03.
    Focused `events_and_triggers` reruns for the two by-call trigger cases and
    `subscriptions::subscription_scenarios` are green with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target`.
    The full `events_and_triggers` target, full `queries_and_proofs` target,
    `network_functional::extra_functional::unstable_network`, full
    `nexus_and_streaming` target, and reduced-sample ignored
    `torii_load_profile` are also green as of 2026-05-04 in the same target
    dir. The stale IVM/Kotodama, Space Directory, lane commitment, Norito
    instruction, and streaming RANS fixtures uncovered by those targets have
    been regenerated.
    The full `core_api` target is green again as of 2026-05-04 after repairing
    private-entrypoint hash handling and widening the slow asset/sealed-reveal
    liveness paths (`171` passed, `4` ignored).
    A broad `cargo test --workspace` reached `integration_tests --lib` after
    compiling the workspace and passing the preceding crate/test targets; the
    first integration-library pass failed on a stale spawned daemon artifact,
    then the exact startup/drop regressions and the full integration library
    passed after rebuild (`41` passed). The core signature slice, crypto
    Ed25519 tests, and strict clippy for core/crypto/integration are also green
    after the deterministic single-Ed25519 verifier cleanup and heartbeat
    execution-context fixture repair. The replay/checkpoint follow-up is green
    as of 2026-05-05 for the focused replay units, Halo2 restart-marker
    verifier, strict core/crypto/consensus integration clippy, and the
    previously failing `consensus_and_da` restart/localnet cases:
    `sumeragi_restart_retains_lock_convergence`,
    `npos_pacemaker_resumes_after_downtime`,
    `confidential_combined_peer_downtime_and_timeout_pressure_localnet`, and
    `confidential_dual_restart_stress_mid_flow_localnet`.
    The 2026-05-07 follow-up also has focused green reruns for the staged
    consensus failures exposed in the latest broad workspace attempt:
    selective-drop recovery, conflicting-ready invalidation, Kura eviction DA
    rehydration, NPoS baseline metrics, pacemaker latency, pacemaker restart
    liveness, stale-evidence rejection, and the VRF randomness module. The
    focused `integration_tests --test consensus_and_da` compile check is green
    in `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check`.
  - Remaining validation: rerun `cargo test --workspace` from a clean start to
    completion in an uncontended multi-hour window.
  - Broad workspace all-target compile validation is green as of 2026-05-07
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check` after
    repairing the default Linux monitor synth gate and stale
    `LaneBlockCommitment` fixture initializers in Python/`xtask`.
- Carry the RAM-LFE API/proof hardening through the remaining signing and clean
  full-workspace Cargo corridor.
  - Focused OpenAPI detached-envelope tests, crypto RAM-LFE tests, the new
    state-deserialization policy regression, Torii RAM-LFE handler tests,
    JavaScript RAM-LFE tests, Swift execute-response parsing, the focused
    `iroha_core` RAM-LFE gate, the workspace all-target compile corridor,
    focused strict clippy over the repaired tool/SDK/Mochi/CoreHost/proof
    targets, JavaScript/Kotlin/JVM/Java Android identifier BFV parity,
    JavaScript Connect Norito schema-hash parity, Android Norito schema-manifest
    verification, C# SDK tests on macOS with a temporary .NET 8 SDK, full Swift
    package tests, full workspace all-target clippy, `scripts/check_no_scale.sh`,
    formatting, and diff whitespace checks are green as of 2026-05-02.
  - Remaining validation: run `cargo test --workspace` in an uncontended
    validation window.
  - Windows C# follow-up: on a Windows box with .NET 8, run
    `dotnet restore csharp/Hyperledger.Iroha.Sdk.sln`, then
    `dotnet test csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj`.
    Confirm the canonical Norito schema-hash test, transaction-builder goldens,
    faucet PoW vectors, and URL escaping expectations pass unchanged; record the
    Windows result in `status.md`.
    Repo-local Linux validation and the read-only public Taira live smoke are
    green as of 2026-05-19; this item is now specifically the external
    Windows-host rerun.
    Also cover the new multisig propose helper work on Windows: the focused
    tests should include
    `ToriiClientTests.ProposeMultisigAsyncPostsNativeNoritoInstructionFrames`
    plus malformed-response cases for invalid or empty `signing_message_b64`,
    false `ok`, negative `creation_time_ms`, and malformed hash metadata,
    and `NoritoCodecTests.EncodeWithSchemaHashUsesProvidedSchemaHash`, and the
    review should confirm `TransactionInstruction.EncodeInstructionBoxBase64`
    emits `InstructionBox` frames suitable for `/v1/multisig/propose`.
  - Focused Kotlin/JVM and Java Android RAM-LFE parser/transport tests are
    green as of 2026-05-02 with Homebrew OpenJDK 21 pinned via `JAVA_HOME`; the
    same harnesses also cover the canonical BFV identifier schema-hash vector.
  - The current static OpenAPI manifest now verifies in explicit unsigned
    first-release mode; before publishing a signed OpenAPI release, rerun the
    same manifest flow with the operator signing key or detached Ed25519
    signature envelope.
- Carry the FastPQ V1 release hardening through the remaining broad validation
  corridor.
  - The 2026-05-17 implementation removes prover-scale CPU replay from
    verification, validates proof-carried roots/transcript challenges/Merkle
    openings/AIR rows/lookup-product binding/FRI query chains from proof
    content, defaults production runtime config to explicit `cpu`, fails
    explicit `gpu` startup closed when preflight is unavailable, bounds Kura
    FASTPQ proof sidecar persistence, adds sidecar telemetry, exposes
    `/v1/pipeline/recovery/{height}/fastpq-proofs`, and adds AXT packaging
    helpers for already-bound batches.
  - Focused `fastpq_prover`, `iroha_config`, `iroha_core fastpq`, Torii recovery
    endpoint, confidential localnet restart/recovery, and explicit
    `fastpq-gpu` release checks are green as of 2026-05-17 with
    `CARGO_TARGET_DIR=target/codex-fastpq-release` and
    `CARGO_TARGET_DIR=target/codex-fastpq-gpu`. The full workspace all-target
    clippy corridor is also green; the remaining open work is only the next
    multi-hour `cargo test --workspace` corridor.
  - AXT proof envelopes now require FastPQ V1 verifier labels at both the
    production FastPQ binding layer and the standalone IVM host envelope-shape
    layer. DefaultHost, CoreHost, and WSVHost reject raw proof bytes and
    synthetic/non-V1 proof labels during diagnostic preflight. Because
    standalone IVM does not link a real FastPQ verifier, proof-consuming AXT
    calls fail closed after preflight; the production AXT verifier rejects oversized
    encoded proof payloads before Norito decode. The descriptor-derived
    synthetic AXT batch builder and CLI fallback have been removed; proof
    generation and measurement require an execution-captured `batch_base64`
    request field. Core state no longer synthesizes FastPQ batch hashes for
    ad-hoc transcript/RWA contexts; those paths require transaction call-hash
    context or a trigger-specific call hash. The shared preflight checks also
    require concrete binding fields,
    supported FastPQ claim types, 32-byte hex digests, and nonempty
    duplicate-free target dataspace sets. DefaultHost also binds handles to the
    manifest root carried by inline, recorded, or late proof envelopes before
    failing closed without a verifier. The focused `fastpq_prover` AXT binding slice,
    `iroha_data_model` `proof_matches_manifest` slice, `ivm_abi`
    `preflight_fastpq_v1_proof_envelope` test, `ivm` `axt_host_flow` target, and
    `ivm` `host_unknown_syscall`/`core_host_policy` targets are green as of
    2026-05-02.
  - CoreHost raw-root rejection and real FastPQ proof-envelope validation is
    covered by the focused `ivm_corehost_axt` proof-binding test with
    `iroha-core-tests,app_api`; the correctly-featured target is green as of
    2026-05-03 with `28` tests. The `app_api`-only command lists zero tests and
    should not be treated as coverage for this target.
  - Block-level app-API AXT validation and host proof-cache success fixtures now
    use reusable FastPQ-backed proof envelopes. The full
    `axt_validation_tests` module and focused `axt_verify_ds_proof` host sweep
    are green as of 2026-05-02.
  - Shared `ProofBlob` matching, standalone `ivm` CoreHost/WSV tests, state
    replay-ledger fixtures, ISI lane-relay registration, and data-model AXT
    fixtures now reject raw manifest roots and binding-less success envelopes;
    only malformed-negative tests keep those payloads.
  - Lane relay proof metadata has no legacy deterministic digest helper and
    carries a required `verified_at_height` field. Verified lane-relay
    registration binds the envelope digest to the submitted proof blob payload
    hash; the data-model proof-material tests and core `lane_relay` slice are
    green as of 2026-05-02.
  - Replace the current prover-scale canonical replay verifier with a succinct
    quotient-only verifier once the V1 quotient commitment/opening API lands;
    this is a performance follow-up, not permission to accept synthetic AXT or
    placeholder proofs.
- Carry the SoraNet VPN escrow hardening through the remaining ledger and
  deployment corridor.
  - The Torii/relay/helper control plane now requires XOR quote payments,
    non-operator escrow custody, client usage vouchers, one-use helper tickets,
    relay TLS pinning, helper-ticket-bound metering keys, and tariff-derived
    relay settlement.
  - Native lease escrow ISIs, WSV lease records, verified tariff settlement,
    relay/helper streaming voucher debt-window enforcement, Torii native
    `OpenVpnLeaseEscrow` quote skeletons, and Torii native `SettleVpnLease`
    receipt skeleton responses through the generic `tx_instructions`
    tooling convention are implemented. Torii active-session lookup and receipt
    settlement now reload authoritative lease state from WSV instead of relying
    on process-local VPN session caches.
  - Relay/backend deployment now uses `vpn.backend_endpoint`; Unix sockets are
    the default privileged path, while TCP requires a shared bootstrap secret
    and Norito MAC envelopes with timestamp/nonce replay checks.
  - Hidden helper workers now receive magic-prefixed Norito connect-payload
    frames over stdin and batch magic-prefixed Norito traffic-state persistence.
  - Relay operators can set `vpn.receipt_spool_dir` to persist the exact
    `/v1/vpn/receipts` request body for voucher-backed sessions, so settlement
    no longer depends on reconstructing receipt bytes from logs.
  - `soranet-vpn-settlement` consumes those artifacts and signs deterministic
    Torii receipt headers/body, or renders curl, using runtime-only operator seed
    material.
  - The JavaScript, C#, Swift, Python, Kotlin/JVM, and Java Android Torii
    clients now expose the quote-first open flow and operator receipt
    submission helpers with native instruction skeletons.
  - Next, finish the focused Cargo validation once the current shared target
    locks clear, then run a public relay/helper/Torii canary that opens a native
    XOR VPN lease from the wallet flow, submits a spooled operator receipt, and
    signs the returned `SettleVpnLease` transaction.
- Carry the IVM/Kotodama vector and syscall hardening through the next clean
  validation corridor.
  - `cargo test -p ivm_abi`,
    `cargo test -p ivm --test vector_execution_regression`, and
    `cargo test -p kotodama_lang vector_length` are green as of 2026-05-02.
  - The updated IVM gas/metadata/pointer window is also green as of
    2026-05-02:
    `cargo test -p ivm --test gas_conformance --test gas_golden --test metadata --test metadata_roundtrip --test pointer_tlv_neg`.
  - The focused analyzer regression
    `cargo test -p ivm analysis_treats_setvl_operand_as_immediate --lib` is
    green as of 2026-05-02.
  - The SCALLX ABI expansion is green as of 2026-05-02 for
    `cargo test -p ivm --lib ivm_is_send_sync_for_state_sharing`,
    `cargo test -p ivm --lib scallx_dispatches_extended_syscall_id`,
    `cargo test -p ivm --test abi_hash_versions --test gas_schedule_hash --test syscalls_doc_sync --test ivm_abi_doc_sync`, and
    `cargo test -p ivm_abi --lib syscallx_roundtrips_24_bit_number`, all with
    `CARGO_TARGET_DIR=target/codex-ivm-scallx`; the core admission regression
    `cargo test -p iroha_core validate_ivm_unknown_scallx_rejected_at_admission --lib`
    is green with `CARGO_TARGET_DIR=target/codex-core-scallx`.
    Follow-up host-bound coverage
    `cargo test -p ivm --lib ivm_is_send_sync_for_state_sharing`,
    `cargo test -p ivm --lib run_with_host_accepts_non_sync_host`, and
    `cargo test -p ivm --lib block_height_syscall_uses_configured_deterministic_value`
    is green with `CARGO_TARGET_DIR=target/codex-ivm-scallx`. Core host
    coverage for `dedicated_query_syscalls_return_norito_payloads`,
    `block_height_sysvar_uses_attached_transaction_context`, and scoped
    durable-state `STATE_KEYS`/`STATE_HAS`/`STATE_LEN`/`STATE_COUNT`
    tombstone resolution is
    green with `CARGO_TARGET_DIR=target/codex-core-scallx`.
    Broader IVM validation is also green with
    `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib` plus
    targeted integration batches for gas/opcode/vector, metadata/pointer ABI,
    predecoder/doc sync, syscalls, WSV-host flows, VRF, and ZK verifier gates.
  - Dedicated `QUERY_GET_ACCOUNT`, `QUERY_GET_ASSET`,
    `QUERY_GET_ASSET_DEFINITION`, `QUERY_GET_DOMAIN`,
    `QUERY_GET_CONTRACT_MANIFEST`, `QUERY_GET_NFT`, `QUERY_GET_PARAMETER`,
    and `QUERY_GET_CONTRACT_INSTANCE` are implemented. The helpers either use
    the validated query engine or deterministic attached-state snapshots, and
    all charge the singular query gas model in code and generated docs.
    `SYSVAR_BLOCK_HEIGHT` is threaded through default hosts and attached core
    query-state contexts. `STATE_KEYS` now provides deterministic durable-state
    prefix enumeration with pagination and contract-scope prefix stripping.
    `STATE_HAS`/`STATE_LEN` provide cheap presence and payload-length probes,
    and `STATE_COUNT` counts matching durable-state keys without returning the
    key list over the same scoped durable-state resolution. Classic `STATE_GET`,
    `STATE_SET`, and `STATE_DEL` now charge documented deterministic state gas
    instead of returning zero. `GET_ACCOUNT_BALANCE` and
    `RESOLVE_ACCOUNT_ALIAS` also return deterministic nonzero query-style gas.
    `TLV_EQ` and `TLV_LEN` now charge deterministic byte-counted codec-helper
    gas costs instead of inspecting potentially large payloads for free.
    Numeric helpers now charge the fixed `G_numeric` cost across default, WSV,
    standalone codec, and real-host forwarding paths. `POINTER_TO_NORITO` and
    `POINTER_FROM_NORITO` now charge `G_pointer + bytes` across the default,
    WSV, and standalone codec hosts, with the byte component tied to the
    canonical TLV envelope copied or validated. Schema helpers and the
    remaining classic codec helpers now charge deterministic byte-counted gas:
    `SCHEMA_*`, `JSON_*`, `DECODE_INT`/`ENCODE_INT`, `NAME_DECODE`, and the
    path builders no longer return zero for payload work. `SM2_VERIFY` now
    charges `G_verify + bytes`; `SM4_GCM_*` and `SM4_CCM_*` now charge
    `G_sm4 + bytes` through the shared default-host implementation, preserving
    deterministic vector output while charging AAD and plaintext/ciphertext
    bytes. Deterministic sysvar reads now charge
    `G_sysvar` or `G_sysvar + bytes`, and authority reads charge
    `G_get_auth + bytes`, across default, WSV, standalone codec, and real-host
	    paths. VRF verification now charges `G_verify + bytes` on decoded
	    status-returning paths, and standalone/WSV ZK verification status exits now
	    charge payload-size verification gas instead of returning zero. ZK
	    roots/tally reads and VRF epoch-seed reads now charge request + response
	    byte gas across standalone, WSV, and real CoreHost paths.
	    `VERIFY_DS_PROOF` now charges `G_verify + bytes` in the real
	    smart-contract host and `G_verify` for proof-clear paths across real,
	    default, standalone CoreHost, and WSV mock hosts while standalone
	    proof-consuming AXT calls remain fail-closed without the real FastPQ
	    verifier. Runtime helper syscalls now also avoid documented zero-gas
	    gaps: `INPUT_PUBLISH_TLV`
    charges envelope bytes across default, WSV, and standalone CoreHost paths;
    `VERIFY_SIGNATURE` charges message/signature/key bytes; and private input,
    nullifier, output commit, heap growth, allocation shim, debug/exit/abort,
    validation-only ISI mutation stubs, FastPQ batch-entry/apply validation,
    and Merkle proof helpers return fixed, page, per-entry, or depth costs
    instead of zero. The WSV mock host direct mutation ABI surface, FastPQ
    transfer batch apply path, and development `SMARTCONTRACT_EXECUTE_QUERY` /
    `SMARTCONTRACT_EXECUTE_INSTRUCTION` JSON shims now also return deterministic
    query or mutation gas instead of treating mock-host state changes as free.
    The real smart-contract host now charges the declared `G_sc_depth` floor for
    `SET_SMARTCONTRACT_EXECUTION_DEPTH`, including the zero-depth no-op path,
    and the declared `G_create_nfts_all` floor for empty
    `CREATE_NFTS_FOR_ALL_USERS` snapshots.
    The classic hash helper surface now includes gas-charged SHA-256,
    SHA3-256, raw Blake2b-256, Keccak-256, and Iroha `Hash::new` syscalls
    routed through the real smart-contract host with byte-identical CPU or
    byte-equivalent accelerated output requirements.
  - `VERIFY_PROOF` now has a CoreHost implementation for
    `NoritoBytes(OpenVerifyEnvelope)` payloads backed by on-chain verifying-key
    registry prechecks and deterministic status-code returns; standalone IVM
    hosts continue to reject it without registry context. Acceleration status
    reporting now only marks CUDA parity as OK when the backend is usable after
    policy, hardware detection, and self-tests.
  - `PROVE_EXECUTION` now returns `NoritoBytes(ExecutionProof)` instead of a
    reserved stub. The proof summary commits to deterministic trace/log/root
    material with SHA-256 and is stable across repeated identical runs, while
    leaving room for later cryptographic prover backends to bind to the same
    public material. Focused unit, syscall, doc-sync, gas-doc, and `ivm_abi`
    regression checks are green with `CARGO_TARGET_DIR=target/codex-ivm-scallx`.
  - The broader `cargo test -p ivm` corridor is green as of 2026-05-02 after
    repairing the data-model compile blocker, refreshing the AXT fixture
    headers, and moving Kotodama test helpers to host-private SCALLX numbers.
    The optimized `cargo test -p ivm --test shifts_prop` focused rerun is also
    green.
  - The `ivm_contract_deploy` staged copy/register fixture tests are green as
    of 2026-05-07 after the literal-table padding repair:
    `cargo test -p iroha_cli --bin ivm_contract_deploy staged_ -- --nocapture`
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check`.
  - Follow-up widened checks are green as of 2026-05-02:
    `cargo test -p ivm_abi`, `cargo test -p kotodama_lang`,
    `cargo clippy -p ivm_abi -p kotodama_lang --all-targets -- -D warnings`,
    and `cargo clippy -p ivm --all-targets -- -D warnings`.
  - Fold the 2026-05-03 Kotodama access-hint, contract artifact registry, and
    literal-padding hardening through the next clean full workspace test and
    clippy corridor after the focused validation recorded in `status.md`.
  - Fold the 2026-05-03 IVM ABI v1 gas/error hardening through the next full
    workspace test and strict clippy corridor after the focused syscall-doc,
    host-policy, AXT, and Soracloud validation recorded in `status.md`.
- Carry the UAID onboarding hardening through the next workspace validation
  corridor.
  - Focused formatting, Python syntax checks, Torii UAID parser tests, Torii
    MCP shortcut/raw-body tests, Torii HTTP onboarding negative-contract tests,
    the full Torii onboarding integration target, Torii onboarding
    error-metadata tests, Swift register-account tests, focused IVM host
    thread-safety tests, the OpenAPI sync/version/signature script tests, the
    focused core UAID portfolio grouping test, the DA manifest fixture sweep,
    `cargo test -p iroha_torii --lib --features app_api`, and
    `cargo check --workspace --all-targets` are green as of 2026-05-02. The
    full workspace all-target clippy corridor is also green as of 2026-05-02
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target`.
  - The Rust implementation is in place for explicit UAID-only onboarding,
    digest-only identity commitments, MCP/OpenAPI request contracts, Swift
    request canonicalization, asset-scope-aware UAID portfolio grouping, and
    stale OpenAPI manifest-signature suppression in generated version indexes;
    `versions.json` has been refreshed in explicit unsigned mode pending the
    operator signature.
  - Keep the broader `cargo test --workspace` corridor open. The repaired
    `events_and_triggers`, `queries_and_proofs`, `nexus_and_streaming`,
    unstable-network, and `core_api` targets are green individually as of
    2026-05-04. The Sora governance runtime-upgrade path now hashes prepared
    transaction entrypoints from the actual canonical signed payload bytes and
    confirms Torii status with explicit auto scope, but the full workspace
    command still needs an uncontended end-to-end pass. The static OpenAPI JSON,
    version index, and unsigned latest/current manifests are refreshed and
    verify under the explicit first-release unsigned corridor.
- Carry the Torii exposure-hardening slice through the remaining workspace
  validation corridor.
  - `cargo fmt --all` and `cargo check -p iroha_config -p iroha_torii` are
    green as of 2026-05-02 for the CORS/pre-auth, MCP tool-effect,
    protected-namespace, route-catchall, mixed-content extractor, and router
    composition changes.
  - `cargo test -p iroha_config torii_cors_parse --lib` and
    `cargo test -p iroha_torii tool_effects --lib` are green as of
    2026-05-02. The follow-up MCP effect audit is also green for
    `cargo test -p iroha_torii get_tools_are_declared_read_effect --lib`,
    `cargo test -p iroha_torii manual_sumeragi_snapshot_tools_remain_read_only --lib`,
    and `cargo test -p iroha_torii tool_effects --lib` with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-continue`. Fold the slice into the
    next workspace clippy/test corridor when validation budget allows.
  - Completed 2026-06-06: app-facing caller-scoped account reads no longer
    accept bare `X-Iroha-Account` as caller identity. Torii now requires
    canonical request signatures or witnesses for private caller visibility,
    while unsigned reads stay limited to public dataspace routes.
  - Completed 2026-06-06: repaired the stale SCCP, SoraFS, ISO20022, ZK IVM,
    and ZK prover fixtures that no longer matched production admission rules;
    `cargo test -p iroha_torii --lib -- --nocapture` is green with `2275`
    passed and `2` ignored.
  - Completed 2026-06-06: Torii's API-token-gated Sumeragi/SCCP/bridge
    telemetry hook now records bounded endpoint/token-state counters without
    exporting raw token material; the feature-enabled Torii clippy corridor is
    green after the SCCP route-manifest alias resolver lint cleanup.
- Carry the Torii first-release API cleanup through the remaining release
  corridor.
  - The route/API/error-envelope implementation, focused Rust sidecar/client
    tests, Swift/Python/Kotlin/JVM/Java Android/JavaScript client regressions,
    JS native/dist rebuild, formatting, and whitespace checks are green as of
    2026-05-17. Static OpenAPI JSON snapshots and latest/current unsigned
    manifests are refreshed and verified; the remaining broad release work is
    the next full workspace test/clippy corridor.
  - Completed 2026-06-06: default Torii builds no longer mount placeholder
    `501 Not Implemented` handlers for `/status`, `/metrics`,
    `/v1/debug/axt/cache`, `/v1/debug/witness`, `/v1/schema`,
    `/debug/pprof/profile`, or `/v1/zk/verify-batch`; these paths are
    feature-owned and absent unless `telemetry`, `schema`, `profiling`, or
    `zk-verify-batch` is compiled. The default OpenAPI snapshots now omit the
    disabled telemetry, schema, and profiling paths as well.
  - Completed 2026-06-06: account-alias resolver service fallbacks no longer
    return `501 Not Implemented` for non-account `AliasTarget` records.
    `/v1/aliases/resolve` and `/v1/aliases/resolve_index` now return a
    documented `409 Conflict` when a stored alias-service record targets an
    asset, peer, or custom payload instead of an account.
  - Completed 2026-06-06: routed-query `query_unsupported` responses now use
    `409 Conflict`, and inbound Torii proxy `Read`, `ReadFanout`, and
    `HostedHttp` requests compiled without `app_api` use `503 route_unavailable`
    instead of `501 Not Implemented`.
  - Completed 2026-06-06: SoraFS proof streaming now rejects
    `proof_kind=pdp` as `400 Bad Request` because the live endpoint accepts only
    PoR/PoTR until the SF-13 PDP provider protocol ships.
  - Completed 2026-06-06: code-only placeholder/TODO sweep removed stale
    governance deploy-proposal and ZK1 validator wording; remaining matches are
    intentional negative tests, placeholder-material fail-closed guards,
    OpenAPI fallback skeleton naming, manifest-derived contract source
    rendering, and telemetry peer compatibility handling.
  - Completed 2026-06-06: Torii's configured SCCP all-lanes launch diagnostic
    now uses the shared supported launch-domain set (ETH, BSC, Solana, TON,
    TRON) instead of the full core diagnostic-domain list. Substrate/SORA2
    configured material remains explicitly tested as out of launch scope, and
    `cargo test -p iroha_torii --lib --features app_api -- --nocapture` is
    green with `2309` passed and `2` ignored.
  - Completed 2026-06-06: the same Torii cleanup slice is now green under
    `cargo test -p iroha_torii --tests --features app_api -- --nocapture`.
    The broad run covers the updated governance council stake-asset fallback
    fixture, feature-gated MCP governance tool dispatch, valid ZK roots
    confidential payload fixtures, and the current Norito error-envelope
    contract for signed ZK attachment failures.
  - Completed 2026-06-06: the feature-minimal Torii connect corridor is now
    green under `cargo check -p iroha_torii --no-default-features --features connect`,
    `cargo test -p iroha_torii --no-default-features --features connect --lib -- --nocapture`,
    and `cargo clippy -p iroha_torii --no-default-features --features connect --all-targets -- -D warnings`.
    App-only route helpers, proof-record reads, hosted HTTP proxy fallbacks,
    integration tests, the attachment sanitizer binary, and hot-path bench now
    sit behind `app_api`/required-feature gates, while core ZK roots, verify,
    submit-proof, and vote-tally DTOs and handlers remain exported without
    `app_api`.
- Carry the Iroha Connect hardening through the remaining SDK and workspace
  validation corridor.
  - P2P session claims, hashed token storage, focused Rust checks, JavaScript
    checks, JS `dist`, Python syntax checks, and shared relay-auth vectors are
    green as of 2026-05-01.
  - Python pytest, Kotlin/JVM, Java Android, and Swift package tests remain
    blocked by missing local tools/artifacts.
  - When the validation shell has `pytest`, a Java runtime, and
    `dist/NoritoBridge.xcframework`, rerun the focused Python Connect tests,
    `./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.connect.ConnectWalletRequestTest --console=plain`,
    the matching Java Android Connect wallet tests, and the focused Swift
    Connect/Torii tests.
  - Fold the Connect session/relay changes into the next broader
    `cargo test -p iroha_torii`, `cargo test --workspace`, and workspace clippy
    corridor when validation budget allows.
- Carry Offline real-proof support through the remaining release corridor.
  - The native bridge prover FFI focused corridor is green as of 2026-04-30. Fold it into a broader `cargo test -p iroha_core --lib`, SDK test, and workspace clippy corridor when validation budget allows.
  - Offline-to-offline SDK local-final semantics, trusted Ed25519 issuer
    certificate verification, and Android rollback fail-closed storage checks
    are green as of 2026-05-17 across Swift, Kotlin/JVM, Java Android, iOS
    simulator XCTest, and Android emulator instrumentation. Fold this into the
    next full workspace test/clippy corridor when validation budget allows.
  - The pure Swift Offline prover hot path is green as of 2026-05-01 with
    subsecond median native audit/redeem proofs on macOS arm64. Keep that
    benchmark in the next iOS-device corridor and broaden Swift package
    validation when budget allows.
  - Kotlin/JVM and Java Android now have the native Offline instance-value
    groundwork and pure Java Halo2/IPA prover path, including focused JVM and
    Android harness coverage plus env-gated benchmark hooks. Keep the native
    prover tests, Swift/JVM cross-verification payload, and larger benchmark
    iteration counts in the next device and full-SDK corridor.
  - The Torii Offline issuer hardening focused corridor is green as of
    2026-05-01. Fold it into the next broader `cargo test -p iroha_torii`,
    SDK, workspace test, and workspace clippy corridor when validation budget
    allows.
- Carry native asset escrow through the remaining Aitai application corridor.
  - Wire the Sora Aitai application UI/backend onto the native numeric escrow ISIs and proof-carrying anonymous escrow helper surfaces, then subscribe through the numeric and anonymous escrow query/event APIs.
  - Add app-facing lifecycle events for transparent and shielded offer state changes, and keep any remaining Kotodama wrapper work scoped to app calls that still need contract compatibility.
  - Add end-to-end UI/client smoke coverage once the Sora Aitai application replaces the old contract escrow account path for both transparent XOR and shielded anonymous-asset offers.
  - Rerun the full Kotlin, Java Android, and Swift SDK suites after the Aitai app wiring lands and a Java 21 runtime is available in the validation shell.
  - Keep NFT/RWA escrow and court fee/payout generalization as separate follow-ups; the v1 primitive intentionally resolves only between the escrow seller and accepted buyer.
- Carry the Soracloud production posture hardening through the operator-host rollout corridor.
  - Local focused, portable QEMU, and prior multi-peer load gates are green as of 2026-04-25; the readiness runner now reports missing operator inventory and missing observability evidence as production blockers. Before public rollout, run the mixed-host Inrou smoke with the real operator inventory, attach the real metrics/status/alert/dashboard evidence, and archive a blocker-free readiness report.
  - The full `irohad` Soracloud binary filter is green as of 2026-05-05 under
    `--features embedded-soracloud-runtime`. The full readiness profile still
    requires operator mixed-host inventory and observability evidence before it
    can produce a blocker-free rollout report.
  - The affected live deployment is intentionally running the 2026-05-08
    no-embedded-runtime `irohad` binary after the Inrou advert incident. Before
    any future live Soracloud runtime rollout, add an explicit operator config
    gate for Inrou enablement and prove that zero-backend hosts do not emit host
    adverts.
- Carry the new Taira devex CLI through the opt-in live rollout corridor.
  - The local CLI/Torii/mock-script validation for `iroha taira doctor` and `iroha taira write-canary` is green as of 2026-04-25, but no live Taira write was run from this tree.
  - Before publishing a live receipt, run `iroha taira doctor --public-root https://taira.sora.org` and an operator-approved `iroha taira write-canary --public-root https://taira.sora.org`, preserving only the redacted receipt and any stable failure codes.
  - Fold the Taira CLI/Torii changes into the next broader `cargo test -p iroha_cli`, `cargo test -p iroha_torii`, workspace test, and clippy corridor when validation budget allows.
- Carry the verified lane relay JSON-state/key change through the next UC6 integration corridor.
  - The focused crate checks are green as of 2026-04-24, but no live UC6 settlement-smoke run or topology reset has been performed from this tree.
  - Before any live deployment, confirm the deploy/Core API smoke path still uses `relay_state_key`, JSON relay state, and the simulation gate against the exact finalization payload.
  - If a topology plan selects reset mode while validating this change, stop before approval and reassess the rollout scope.
- Carry the Torii routed-read and telemetry fixes through the next workspace validation corridor.
  - The crate-local sweep is green as of 2026-04-24 with `cargo test -p iroha_torii --lib --features app_api,telemetry -- --nocapture`.
  - When validation budget allows, carry the alias-routing and Torii telemetry slices through the next `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` corridor and record the result in `status.md`.
- Broaden validation for the new canonical account-alias lease flow beyond the focused onboarding and executor checks.
  - The onboarding auto-renew path now grants the subscriber `CanModifyNftMetadata` for the subscription NFT before trigger registration; rerun a wider `cargo test -p iroha_torii` window with the new `/v1/accounts/{account_id}/aliases`, `/renew`, and `/auto-renew` handlers enabled.
  - Add or rerun focused coverage for user-signed enable/disable mutation flows and the SNS subscription auto-renew billing path in `crates/iroha_core/src/smartcontracts/ivm/host.rs`, not just the onboarding enqueue path.
  - Once the alias lease slice is stable under those focused reruns, fold it into the next broader `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` corridor.
- Keep the Sumeragi main-loop broad corridor attached to future consensus
  changes.
  - The 2026-05-08 idle timing-cache change is covered by focused cached
    commit-quorum timeout, NPoS commit-floor, rebroadcast-cooldown,
    commit-pipeline cooldown, effective-timing snapshot, mode-flip tick
    deadline, commit-evidence replay cooldown, and proposal-backpressure timing
    tests. Rerun the full `cargo test -p iroha_core --lib` corridor before the
    next consensus sweep.
  - The 2026-05-08 known-block commit-QC recovery dampening is covered by
    focused cert-only fetch, duplicate-QC cleanup, committed-tip reacquisition,
    stale same-height view pruning, bounded missing-QC view rotation,
    stall-reset fallback handoff, local-payload recovery, retry-loop, and
    same-height `BlockBodyResponse` repair tests. Rerun the full
    `cargo test -p iroha_core --lib` corridor before the next consensus sweep.
  - The realistic 30 TPS, 20-minute transfer soak passes as of the
    2026-05-08 block-body response ingress fix. Remaining open work is
    throughput margin, not liveness: the passing release-daemon run submitted
    at 30.00 TPS but committed 21.61 TPS during load, peaked at 9,973 queued
    transactions, and needed 722 seconds of drain time. Use
    `integration_tests/artifacts/realistic-30tps-transfer-20min-640-release-daemon-block-body-response-block-lane/throughput-1778229477740/`
    for the next worker/proposal throughput tuning pass.
  - The matching realistic 30 TPS, 20-minute RAM-LFE email-claim soak also
    passes as of 2026-05-08. The release-daemon run submitted 36,000
    `ClaimIdentifier` email transactions, reached the 36,008 approved target
    with zero rejects, and finished with all peers at 723 non-empty blocks.
    Margin remains the open item: load committed at 21.27 TPS, final committed
    TPS was 19.17 including drain, peak queue was 10,377, and drain took 677
    seconds. Use
    `integration_tests/artifacts/realistic-30tps-ram-lfe-email-20min-release-daemon/throughput-1778232961671/`
    alongside the transfer artifact for the next worker/proposal throughput
    tuning pass.
  - The 2026-05-08 DA/RBC large RAM-LFE proposal fallback is covered by focused
    DA payload-budget tests, a RAM-LFE oversized-frame fallback regression, and
    the adjacent unservable-payload deferral check. Rerun the full
    `cargo test -p iroha_core --lib` corridor before the next consensus sweep.
  - The 2026-05-06 canonical proposal/block entrypoint-ordering fix is covered
    by focused ordering, mixed-entrypoint builder, rejection mapping,
    noncanonical static/unchecked-validation, and PrivateKaigi entrypoint
    execution regressions. Rerun the full `cargo test -p iroha_core --lib`
    corridor before the next consensus sweep.
  - The 2026-05-03 `cargo test -p iroha_core --lib` rerun is green
    (`5129` passed, `22` ignored) after fixing execution-witness recorder
    isolation and hardening the RBC sidecar cooldown fixture.
  - The later 2026-05-03 restarted-peer commit-QC recovery fix is covered by
    focused block-body response regressions and the confidential downtime plus
    timeout localnet scenario, now passing without the restarted-peer catch-up
    waiver warning. Rerun the full `cargo test -p iroha_core --lib` corridor
    after the next main-loop edit or before opening the next full workspace
    sweep.
  - For the next consensus change, rerun the same broad window so the collector
    fallback, exact-frontier repair, cached-target, vote replay, roster
    recovery, future-new-view, and model-backed reschedule fixtures continue to
    execute together rather than only as isolated filters.
- Broaden Sumeragi verification when new fatal hang classes are identified
  outside the current two-slot frontier abstraction.
  - The 2026-05-03 frontier formal process hardening is green and covers active
    pending progress touch, local-vote and commit-QC progress, stale recovery
    subject-view scope, vote-queue drain, payload recovery, quorum retransmit,
    retransmit follow-through, and future-slot promotion.
  - For any additional fatal hang shape, first add a focused Rust regression,
    then add the corresponding finite formal dimension or mutation so the
    expected-failure suite proves the model would have caught it.
  - If another restarted-peer catch-up issue appears in message admission or
    deduplication, add a small finite admission-order bridge or mutation before
    broadening the frontier model itself; the current model intentionally
    abstracts network-message dedup away.
  - Keep this scoped to the observed hang surface; do not generalize the model
    into an arbitrary pipeline unless a new bug requires more than the active
    plus one-future-slot abstraction.
- Reopen the wider validation corridor after the recent focused `iroha_core`, `iroha_torii`, and `iroha_data_model` test additions.
  - `cargo test -p iroha_core --lib` is green as of 2026-05-03; rerun it only
    after the next core/consensus change or before opening the full workspace
    corridor.
  - `cargo test -p iroha_torii` is green as of 2026-05-03 after fixing the
    macOS attachment-sanitizer subprocess wrapper path; rerun it after the next
    Torii/API change or before opening the full workspace corridor.
  - Rerun `cargo test -p integration_tests -- --nocapture` once the current
    tree is stable enough for network suites.
  - When validation budget allows, rerun `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`, then capture failures or green status in `status.md`.
## Consensus and Izanami

- Maintain Izanami communication vulnerability publication evidence.
  - The exact-injector 75% packet-loss 2026-04-26 paper-shaped run at `dist/izanami-exact-packet-paper-20260426` is green for both permissioned and NPoS Sumeragi and is recorded in `status.md`; keep this as the current full-matrix resilience baseline.
  - Native in-process P2P packet-drop injection is wired into `packet-loss` and leader-targeted `leader-isolation`; the matrix runner now supports the paper's 133s-266s timed fault window plus configurable packet-loss sweeps (`75%` quick, `25%/50%/75%` paper). The explicit 25%/50%/75% paper packet-loss sweep at `dist/izanami-packet-sweep-paper-20260427-loss-only` is green for both permissioned and NPoS Sumeragi and is recorded in `status.md`.
  - The 2026-04-27 quick matrix at `dist/izanami-quick-both-20260427` is green for all ten permissioned/NPoS rows, and the post-ingress-hardening leader-isolation rerun at `dist/izanami-quick-leader-retry-20260427` keeps both modes resilient with zero acceptance markers.
  - The result-strengthened matrix and sweep tooling is implemented as of 2026-04-28, including bounded shutdown-drain accounting, latency/recovery evidence, NPoS repair-coverage telemetry, generated `paper-style-final-report.md`, and separate `stress-400` / `stress-800` profiles.
  - Seed-7 real stress evidence at `dist/izanami-stress-400-seed7-20260428` and `dist/izanami-stress-800-seed7-20260428` is refreshed and green as of 2026-04-29: both `stress-400` and `stress-800` report 14/14 resilient rows across permissioned and NPoS Sumeragi, with no real `confirmation_queue_dropped` pressure in the fresh artifacts. This is recorded in `status.md`.
  - Run the full paper/stress seed sweep with fresh binaries when validation budget allows: `scripts/run_izanami_communication_vulnerability_sweep.sh --profiles paper,stress-400,stress-800 --sumeragi-mode both --seed-list 7,11,13,17,19,23,29,31,37,41`. Paper rows must remain `resilient`; stress rows should stay reported separately as margin evidence across broader seeds.
  - Keep any future publication reruns split with `--sumeragi-mode both` so permissioned and NPoS Sumeragi classifications are not collapsed, and preserve per-loss packet-loss subrows when comparing against the paper's Algorand/Aptos/Avalanche/Redbelly/Solana baseline.
- Recalibrate the Izanami stable-profile acceptance envelope for sustained workload targets.
  - The fresh 4-peer permissioned `1 TPS` / `300s` / `100 blocks` gate at `dist/izanami-stable-gate-20260427-target100` is green and recorded in `status.md`.
  - The matching `200`-block diagnostic at `dist/izanami-stable-gate-20260427-rerun` crossed the prior stall region and reached strict/quorum height `107` with zero submission or confirmation failures, but missed the target because the stable workload drained before `200` blocks.
  - Before the longer `3600s` / `2000+` block acceptance pass, choose a sustained-workload gate or lower short-run target so the KPI measures liveness instead of exhaustion of submitted work.
- Root-cause the remaining NPoS soak/localnet collapse instead of keeping it as a log-only symptom.
  - Reproduce with preserved peer dirs and `iroha_futures::supervisor=debug`.
  - Identify the first exiting supervised child before investigating downstream connection refusals.
  - Cross-check peer logs with `/v1/sumeragi/status` counters so the fix targets the actual failing layer.

## Throughput and query performance

- Re-establish current throughput knees for the de-amplified harness and shared-host localnet.
  - Rerun the stepped single-host sweep.
  - Repeat permissioned and NPoS passes on the same hardware envelope and compare against the archived `25-50 TPS` / `75-100 TPS` baselines.
  - Record the new knee points and any regressions in `status.md`.
- Carry the 2026-05-02 Norito/Crypto scalar hot-path slice through the remaining
  release validation corridor.
  - The Ed25519 admission follow-up now caches deterministic 32-byte invalid
    public-key parse outcomes, routes compact/full conversion through the
    cached parse path, widens the hot thread-local parse/verify caches for the
    20k stable workload window, skips signature parsing and dalek batch setup
    for all-cached exact verify tuples, and preserves lowest-original-index
    failure reporting for mixed batches. Focused crypto/Torii checks and the
    `ed25519_hotpaths` Criterion bench are recorded in `status.md`.
  - Remaining local benchmark baselines: `cargo bench -p iroha_data_model
    --bench chain_wire`, `cargo bench -p iroha_data_model --bench
    decode_registry`, and `cargo bench -p iroha_core --bench crypto_hotpaths`.
  - The latest 120s release gate rerun exists at
    `dist/izanami-prebuilt-20k-rerun-release-ed25519-cache-120s-20260502-180614`
    and is recorded in `status.md`; the wrapper exited `0`, but it is not a
    clean all-accepted ingress gate. It offered all `2,400,000` planned
    submissions, accepted `2,364,756`, reported `35,244` failures, and reached
    strict approved transactions `20,582` at strict height `7`, with the queue
    still saturated. Active build/gate process lines were captured before and
    after the run, so it remains diagnostic evidence only.
  - The latest contended 30s sampled profile exists at
    `dist/izanami-profile-20k-ed25519-cache-sampled3-30s-20260502-182524`
    and is recorded in `status.md`. It submitted and accepted all `600,000`
    planned ingress attempts but only reached strict approved transactions
    `4,113` at strict height `3`, with the queue still saturated. The next
    bottleneck focus remains peer CPU: FASTPQ transcript finalization over
    Norito account/numeric/array serialization into Poseidon byte hashing;
    Ed25519/Curve25519 batch-verifier miss work and public-key parse/decode
    misses; Norito transaction/signature decode and compact-length work; and
    smaller allocation/copy/CRC64 costs. It is not a clean comparable baseline
    because workspace `cargo test`/rustc and another debug test network were
    active before and after the run.
  - The FASTPQ GPU follow-up is now recorded in `status.md`: Metal toolchain
    preflight is green, `bn254_poseidon_words` uses the Metal backend,
    transcript digest finalization overlaps Metal dispatch with CPU work,
    execution-witness digest propagation avoids a duplicate witness-side
    finalization, the final `fastpq-gpu` 120s release gate accepted all
    `2,400,000` offered submissions and reached `36,986` strict-approved
    transactions, and the delayed load-window sampled peer stacks have no
    scalar `poseidon3_permute` or CPU FASTPQ fallback. CUDA hardware closure
    evidence was captured later on 2026-05-19 and is recorded in `status.md`.
  - The 2026-05-05 hardware-backed FASTPQ Metal parity rerun on macOS is green
    after repairing Goldilocks FFT/LDE, BN254 LDE, and Poseidon Metal/CPU
    mismatches. CUDA hardware closure evidence was captured later on
    2026-05-19 and is recorded in `status.md`.
  - The next throughput slice should target the post-GPU peer CPU stack:
    Ed25519/Curve25519 public-key parse and verification, Norito
    transaction/transfer serialization and decode, transaction metadata
    hashing, allocation/copy traffic, and CRC64/SHA-256 helpers. The first
    bookkeeping slice already removed per-transaction `DashMap::len()` from
    `PipelineStatusCache::prune_if_needed`, and the Ed25519 thread-local slice
    now includes a direct-mapped full-key cache before the generic linear
    verifier cache. The current allocation slice streams typed Norito hashes
    directly into Blake2b, finalizes direct Blake2b hashes into fixed buffers
    without boxed digest allocation, absorbs Merkle parent/commitment chunks
    without staging concatenation buffers, and hashes external transaction
    entrypoints through a borrowed encoder instead of cloning the signed
    transaction into an enum wrapper. The release Izanami/iroha3d binaries now
    rebuild with the allocation slice, and the clean return gate at
    `dist/izanami-prebuilt-20k-fastpq-gpu-return-120s-20260504-012106`
    restored ingress (`2,400,000` accepted and succeeded, `0` failures) but
    still reached only `12,413` strict-approved transactions at height `5`.
    The matching sampled profile at
    `dist/izanami-profile-20k-fastpq-gpu-return-sampled-30s-20260504-012521`
    was intrusive, but its peer stacks confirm the next work remains
    Ed25519/Curve25519 parse and verification, Norito transaction/transfer
    encode/decode, metadata hashing, allocation/copy traffic, and SHA-256/CRC64
    helpers. A first queue-lock slice now releases `push_remove_lock` before
    post-enqueue backpressure/gossip/event/wake side effects. The follow-up
    bottleneck fix repairs the post-queue-lock execution-context mismatch,
    moves RBC READY/DELIVER traffic onto the consensus-chunk lane, gives chunk
    traffic a turn after each high-priority payload frame, caches prepared
    metadata JSON depth, and keeps prepared metadata depth checks on the
    static-validation hot path. The clean rebuilt
    `20k TPS` / `120s` `fastpq-gpu` gate at
    `dist/izanami-prebuilt-20k-fastpq-gpu-bottleneckfix-120s-20260504-183724`
    accepted and succeeded all `2,400,000` submissions with no safety failures
    and reached `37,000` strict-approved transactions at height `11`, but queue
    saturation remained (`854,344 / 2,400,000`). The matching sampled profile at
    `dist/izanami-profile-20k-fastpq-gpu-bottleneckfix-peer-sampled-30s-20260504-184154`
    shows no scalar FASTPQ/Poseidon fallback; the next bottlenecks are block
    validation and serialization costs: Ed25519/Curve25519 verification math,
    Norito compact-length and transaction/transfer encode/decode,
    allocator/reallocation and copy traffic, SHA-256/Blake2/CRC64 helpers,
    `resolve_streaming_metadata`, and pipeline access/overlay preparation.
    A final prepared-hash cleanup after that profile avoids temporary
    signed-transaction byte vectors while preparing hashes/lengths and reuses
    prepared payload/signed hashes in validation cache and signature-batch
    paths. The current-code `20k TPS` / `120s` rerun at
    `dist/izanami-prebuilt-20k-fastpq-gpu-return-current-120s-20260504-194602`
    covered that cleanup: Izanami exited `0`, accepted and succeeded all
    `2,400,000` submissions, recorded no safety failures, and had submit
    latency `p50=6ms`, `p95=21ms`, `p99=99ms`, `max=269ms`. Strict progress
    was lower than the previous gate at `32,956` approved transactions at
    height `10`, with queue saturation still high (`883,791 / 2,400,000`) and
    commit-pipeline EMA `592ms`. Treat the 20k ingress path as restored; the
    committed-throughput target still needs the next validation/serialization
    hotspot pass. The fresh current-code profiles refine that target: the
    immediate `30s` sample at
    `dist/izanami-profile-20k-fastpq-gpu-current-peer-sampled-30s-20260504-195325`
    shows FASTPQ Metal pipeline creation still happens on the first proof hot
    path, while the delayed post-warm `60s` sample at
    `dist/izanami-profile-20k-fastpq-gpu-current-peer-postwarm-sampled-60s-20260504-195720`
    moves the steady-state bottleneck back to validation and serialization.
    The 2026-05-05 FASTPQ lane preflight follow-up moves backend construction
    off the startup/submission path, keeps digest acceleration disabled until
    the lane observes successful GPU preflights, and falls back to CPU prover
    modes after a failed Poseidon GPU preflight. The current May 6 return gate
    at
    `dist/izanami-prebuilt-20k-fastpq-gpu-return-current-120s-20260506-124641`
    accepted and succeeded all `2,400,000` submissions with no safety failures
    and reached `49,428` strict-approved transactions at height `14`, above the
    previous `45,191` preflight gate. Treat first-proof FASTPQ GPU preflight
    and the latest single-transfer digest deferral path as addressed for now;
    the next open work is Ed25519/public-key parse and verify work, Norito
    transaction and transfer encode/decode/length accounting, allocation/copy
    churn, queue-admission/world-view preparation, and queue drain under
    saturated 20k ingress. That older profile avoided scalar FASTPQ/Poseidon
    fallback work until new evidence; the May 7 load-window sample below
    reintroduces scalar cost specifically in the BN254 runtime digest path,
    while general FASTPQ prover parity remains fixed.
    The 2026-05-07 Metal final return gate fixes general FASTPQ Poseidon
    preflight parity and removes normal commit-QC inline validation supersedes;
    keep the next Izanami pass on queue drain/block-validation cost and BN254
    runtime Metal batch stability, not on prover Poseidon preflight parity.
    The corrected load-window profile at
    `dist/izanami-profile-20k-fastpq-gpu-final-loadsample-90s-20260507-225637`
    sharpens that order: scalar Halo2 BN254 Poseidon is again the top sampled
    application leaf after runtime Metal batch failures, while consensus
    progress is limited by payload availability and exact-frontier recovery
    signals under a saturated queue. Fix BN254 runtime batch stability first,
    then reduce local READY/DELIVER deferrals and block-body reacquisition
    latency before revisiting the secondary Norito, Ed25519/Curve25519, SHA-2,
    Blake2, CRC64, and allocation hot paths.
  - Avoid repeating the rejected process-wide Ed25519 public-key parse cache
    approach without new evidence: the 2026-05-03 sharded shared-cache
    experiment regressed short-gate commit progress and was backed out. Keep
    near-term Ed25519 work thread-local, allocation-focused, or validation-path
    specific unless a clean before/after gate proves otherwise. The accepted
    thread-local slice pre-sizes only the public-key parse map, keeps parsed key
    entries boxed to satisfy `variant-size-differences`, and keeps the generic
    verify-ok map lazy so 32-byte transaction hashes do not allocate unused
    generic cache state.
  - Keep broader trait-wide parallel decode, deeper GPU decode materialization,
    deeper dalek backend experimentation, and deterministic hardware-specific
    Ed25519/Curve25519 acceleration as follow-up work until the current
    bottleneck slice has clean before/after evidence.
- Continue the 20k post-cache throughput tuning corridor.
  - The first post-cache 4-peer no-fault prebuilt `20k TPS` / `120s` release
    gate at `dist/izanami-prebuilt-20k-hotpath-120s-20260501-142015` improved
    strict approved transactions to `28,713` but still failed the committed
    20k target.
  - A same-shape repeat at
    `dist/izanami-prebuilt-20k-cachepass-120s-20260501-142429` accepted
    `52,167` ingress transactions but only reached `24,623` strict approved
    transactions, confirming material run-to-run variance and the same
    queue-drain/block-validation bottleneck.
  - The fresh post-cache sampled 20k profile at
    `dist/izanami-profile-20k-cachepass-sampled-30s-20260501-152126` confirms
    the next target has moved from queue gossip encoding to
    `validate_block_for_voting` / `validate_and_record_transactions` /
    `TxOverlay::apply_with_chunk`, incoming transaction-gossip Norito decode,
    and the remaining `AcceptedTransaction::signed_encoded_len` serialization
    fallback.
  - The targeted post-cache tuning pass at
    `dist/izanami-prebuilt-20k-postcache-tuned-120s-20260501-165947` improved
    strict approved transactions over the cachepass repeat to `28,790`, but
    still failed the committed 20k target and accepted fewer ingress
    submissions. The matching sampled profile at
    `dist/izanami-profile-20k-postcache-tuned-sampled-30s-20260501-165811`
    confirms `Queue::encode_gossip_payload`, `TxOverlay::byte_size`, and
    `external_entrypoints_cloned` are absent from current peer samples.
  - The further conservative cache pass is focused-validation green as of
    2026-05-01: prepared transaction metadata is reused through block
    validation/execution recording, all-external block validation keeps
    borrowing the entrypoint slice, signed/external entrypoint encoded-length
    coverage avoids the residual Norito fallback for representative shapes, and
    gossip transaction decode now uses the shared cached payload helper.
  - Rechecked 2026-06-04: the focused `AcceptedTransaction` signed-length,
    decoded-versioned signed transaction, and gossip signed-metadata regressions
    remain green. No additional no-wire-change edit is obvious without a fresh
    sampled profile showing `signed_encoded_len` as a material current bottleneck.
  - The clean release 4-peer no-fault prebuilt `20k TPS` / `120s` rerun at
    `dist/izanami-prebuilt-20k-conservative-cache-rerun-120s-20260501-175213`
    exited `0`, accepted `54,574` ingress transactions, and reached `28,710`
    strict approved transactions at strict height `9`. This is consistent with
    the prior tuned gates and still misses the committed 20k target, with no
    validation rejects, view changes, or RBC pressure.
  - A later requested same-shape rerun at
    `dist/izanami-prebuilt-20k-conservative-cache-rerun2-120s-20260501-144548`
    exited `0` but ran under active debug `cargo test`/`rustc` contention. It
    accepted `52,070` ingress transactions and reached only `12,329` strict
    approved transactions at strict height `5`, with safety intact but `4`
    view-change installs and missing-block recovery activity. Treat this as
    contended evidence only, not a replacement for the clean baseline.
  - The matching requested contended sampled profile at
    `dist/izanami-profile-20k-conservative-cache-rerun2-sampled-30s-20260501-145104`
    exited `0` with valid samples for the load driver and all four peers;
    `sample_status=1` only because the sampler also targeted the bash wrapper
    and one transient process. It accepted `52,817` ingress transactions and
    reached `4,137` strict approved transactions at strict height `3`. The
    bottleneck shape matches the previous conservative-cache profiles: Torii
    admission crypto/public-key parsing, canonical signed-byte construction,
    residual dynamic `InstructionBox` framing, gossip materialization/decode,
    and overlay execution/cloning. Treat it as contended bottleneck evidence,
    not a clean latency baseline.
  - The earlier conservative-cache sampled 20k profile at
    `dist/izanami-profile-20k-conservative-cache-parallel-sampled-30s-20260501-181025`
    confirms the previous removals are still absent
    (`Queue::encode_gossip_payload=0`, `TxOverlay::byte_size=0`,
    `external_entrypoints_cloned=0`) and moves the next bottleneck set to
    Torii ingress signature/public-key work, canonical signed-byte construction
    in `AcceptedTransaction::from_external_with_hot_cache`, exact-length
    `InstructionBox` payload framing, gossip materialization during admission,
    and remaining overlay instruction clones.
  - The broader 20k bottleneck pass is focused-validation green as of
    2026-05-01. Lazy transaction-gossip materialization now preserves cached
    framed entrypoint bytes and skips semantic decode before route, plane, and
    known-duplicate filters; route-valid single-key Ed25519 gossip candidates
    use deterministic batch precheck through the existing signature-batch
    setting; overlay apply goes through the crate-private borrowed adapter while
    custom executors keep the owned path. The profile at
    `dist/izanami-profile-20k-postcache-tuned-bottleneck-30s-20260501-171955`
    is pre-broader-pass evidence; the fresh reruns are
    `dist/izanami-profile-20k-broader-pass-sampled-30s-20260501-194734` and
    `dist/izanami-prebuilt-20k-broader-pass-120s-20260501-194908`.
    The 120s gate kept final approved transactions flat against the previous
    gate (`28740` vs `28710`) but accepted fewer ingress submissions
    (`52291` vs `54574`), so treat the pass as bottleneck reshaping rather than
    a confirmed throughput win.
  - The fixed-runner follow-up sampled profile at
    `dist/izanami-profile-20k-broader-pass-rerun-sampled-30s-20260501-200527`
    completed with `sample_status=0` and sampled the actual Izanami runner plus
    all observed peer processes. It classifies the next bottlenecks as
    direct-peer Ed25519/curve25519 verification math first, then allocation /
    `memmove` and Norito compact/decode work, with ZK/BLS math and hashing as
    secondary costs. Queue mechanics and borrowed overlay apply are not primary
    CPU bottlenecks in that sample.
  - The latest clean rebuilt release 4-peer no-fault prebuilt `20k TPS` /
    `120s` gate is
    `dist/izanami-prebuilt-20k-direct-ingress-precheck-final-120s-20260501-212850`;
    it exited `0`, accepted `47,566` ingress transactions, and reached
    `20,499` strict approved transactions at strict height `7`. The contention
    snapshots only contain timestamps. Safety signals stayed clean, but the
    run still saturated the queue and ended with height skew `1` /
    approved-transaction skew `8,192`, so the 20k target remains open.
  - The latest fixed-runner sampled profile at
    `dist/izanami-profile-20k-rerun-release-sampled2-30s-20260501-211211`
    completed with `sample_status=0`, accepted `46,709` ingress transactions,
    and reached `4,125` strict approved transactions at strict height `3`.
    The current peer CPU stack is led by `iroha_zkp_halo2::poseidon` /
    `fastpq_isi::poseidon`, `memmove` and allocator paths, `sha2`/`blake2`
    hashing, Norito compact-length/decode/encode routines, and then
    `curve25519_dalek` / `ed25519_dalek` verification math. Direct ingress
    batch precheck remains visible but is not the dominant leaf in this sample;
    overlay clone and exact-length helpers are low-count residue.
  - The direct-ingress conservative cache and precheck slice is code-complete
    as of 2026-05-01: Torii signed transaction and batch submission now decode
    versioned signed payloads into a prepared core admission token and run
    deterministic single-Ed25519 batch precheck for eligible batch entries,
    reusing signed/entrypoint hashes, payload hash, exact signed length, and
    parsed single-Ed25519 key metadata without changing transaction wire/hash
    semantics, config knobs, dependencies, or `Cargo.lock`.
  - The exact-length `InstructionBox` cost is reduced without changing Norito
    wire: `encoded_len_exact` now counts the existing `(wire_id,
    framed_payload)` representation without re-framing the dynamic ISI payload.
  - The FASTPQ/Poseidon foreground pass is implemented: single-delta transfer
    transcript digests are finalized at block/witness drain instead of inside
    `Transfer::execute`, FASTPQ digest hashing streams bytes without a full
    preimage buffer, and decoded external entrypoint hashes now reuse the
    inbound versioned signed payload bytes.
  - The first FASTPQ BN254 Metal Poseidon batch path is implemented behind the
    existing `fastpq-gpu` feature and existing FASTPQ execution/poseidon modes.
    Later Metal and CUDA parity/performance closure evidence is recorded in
    `status.md`; this historical slice no longer carries open GPU validation
    work.
  - Carry the Norito sequence span planner through the remaining acceleration
    corridor: replace the length-prefixed helper's serial device parser with a
    tuned prefix-scan/chunked planner if profiling shows it is on the hot path,
    expand typed parallel sequence decode beyond the current hidden
    `parallel-decode` `Vec<T: Send>` path if profiling proves narrower
    transaction/admission/block-validation call sites need it, then rerun the
    30s sampled 20k profile and 120s gate with the target host's acceleration
    features.
  - The latest scalar release 4-peer no-fault prebuilt `20k TPS` / `120s` gate
    after the Norito span-planner pass is
    `dist/izanami-prebuilt-20k-rerun-release-norito-span-120s-20260502-015557`;
    it exited `0`, accepted `47,503` ingress transactions, reached
    strict/quorum height `10`, and approved `32,786` transactions. The latest
    matching scalar sampled profile is
    `dist/izanami-profile-20k-norito-span-sampled-30s-20260502-020217`; it
    shows Norito transaction/instruction codec as the current top active peer
    path, followed by Poseidon/Ed25519/Curve/hash work, Rayon proof/hash
    scheduling, allocation/copy churn, TLS/context lookup, and Torii admission
    queue routing. Use this artifact as the baseline before the next
    optimization pass.
  - Continue reducing Norito decode/allocation overhead on the direct and
    gossip admission corridors without changing wire bytes or canonical hashes.
    `InstructionBox::DecodeFromSlice` now uses the borrowed tuple parser
    directly and `ExecutionStep::DecodeFromSlice` now delegates its inner
    instruction list to `ConstVec<InstructionBox>`. `ConstVec<T>` slice decode
    now tries the scalar Norito sequence planner directly for non-`u8` elements
    before falling back to the canonical `Vec<T>` field path, removing the
    top-level archive/canonical-length pass from the hot instruction-vector
    route. `AcceptedTransaction` also now derives the cached signed frame and
    external entrypoint hash from one canonical signed payload in the hot-cache
    path, avoiding a second signed-transaction serialization. `SignedTransaction`
    and `TransactionPayload` slice decoders now walk AoS fields directly, and
    `Executable::Instructions` routes the instruction vector into the planned
    `ConstVec<InstructionBox>` decoder before falling back for other executable
    variants. A fresh WSL2 no-profiler validation run after this
    admission-decode pass is recorded in `status.md`:
    `dist/izanami-prebuilt-20k-admission-decode-unsampled-30s-20260506-020112`
    accepted/succeeded all `600,000` offered submissions, and
    `dist/izanami-prebuilt-20k-admission-decode-120s-20260506-020335`
    accepted/succeeded `2,379,055` submissions with no safety failures but only
    `20,553` strict-approved transactions. Treat these as fresh ingress/safety
    evidence, not a bottleneck profile: the host had neither `sample` nor
    `perf`, and the 2.4M prebuilt-buffer run consumed nearly all WSL2 memory.
    Individual instruction payload slice paths are now in place for `Log`,
    `RecordSccpMessage`, the canonical grouped instruction boxes
    (`RegisterBox`, `UnregisterBox`, `MintBox`, `BurnBox`, `TransferBox`,
    `SetKeyValueBox`, `RemoveKeyValueBox`, `GrantBox`, `RevokeBox`,
    `RwaInstructionBox`, `RepoInstructionBox`, and `SettlementInstructionBox`),
    transfer batches, account signatory/quorum changes, the stable core
    SetParameter/trigger/upgrade/custom ISIs, asset-definition
    alias/balance-policy instructions, asset transfer-control instructions,
    account alias binding/lease instructions, contract-alias instructions,
    account-recovery instructions, RAM-LFE program-policy instructions,
    hidden-identifier instructions, consensus-key lifecycle instructions,
    domain-endorsement instructions, verifying-key instructions, Offline
    note instructions, verified Nexus lane-relay/fee-budget instructions,
    native and anonymous asset escrow lifecycle instructions, Musubi
    package-registry instructions, smart-contract-code
    manifest/instance/bytecode instructions, the Space Directory manifest
    lifecycle instructions, SoraFS pin/capacity/replication/provider-owner plus
    pricing/credit instructions, oracle feed/observation/dispute/governance/
    Twitter binding instructions, bridge proof/receipt/SCCP instructions,
    Ministry citizen-agenda proposal submission, social Twitter reward/escrow
    instructions, registered public-lane staking instructions, invalid-
    instruction placeholders, SoraNet VPN lease open/settle/refund instructions,
    runtime-upgrade ISIs, SNS name ISIs, ZK proof/confidential/election ISIs,
    Kaigi session/relay ISIs, governance proposal/ballot/citizen ISIs,
    Soracloud service lifecycle, host/placement, agent, model/training,
    rollout, runtime-state, mailbox, and receipt ISIs, and Nexus
    emergency-validator override ISIs via an opt-in registry constructor. The
    default registry no longer exposes direct grouped generic wire forms such
    as `Register<Domain>`, concrete mint/burn/transfer variants,
    `Grant<Permission, Account>`, `RepoIsi`, or `DvpIsi`; canonical clients use
    the grouped boxes. Remaining targets are the standalone entries that still
    use the generic constructor, broader allocation/memmove churn around
    transaction admission material, and a sampled 30s profile plus clean 120s
    gate on a profiler-equipped host after the next scalar admission-decode
    pass.
  - The FASTPQ BN254 Metal validation was completed in later accelerator
    closure passes recorded in `status.md`; keep new profiling here focused on
    the remaining scalar admission/decode and Ed25519 authority costs.
  - Keep an Ed25519 parsed-public-key/signature verification cache or a
    deterministic batch corridor for the Torii/direct-ingress single-key
    Ed25519 authority path as the next crypto follow-up after the
    Poseidon/source-attribution and Norito allocation work. Gossip-side
    deterministic Ed25519 batch precheck is already implemented, and the
    crypto-layer direct/preparsed Ed25519 batch APIs now filter exact
    verify-cache hits before signature parsing; the thread-local exact
    verify-ok cache also keeps two colliding entries per slot. The ML-DSA key
    path now rejects inconsistent imported secrets and exposes
    `KeyPair::try_from_seed`, `KeyPair::try_random`,
    `KeyPair::try_random_with_algorithm`, `PublicKey::try_to_*`,
    `ExposedPrivateKey::try_to_*`, `Signature::try_new`, plus typed
    `SignatureOf::try_*` constructors, so remaining crypto follow-ups should
    focus on hot verification boundaries rather than ML-DSA panic replacement.
  - Rerun 4-peer no-fault prebuilt `5k` and `10k TPS` rows as needed to locate
    the new knee after the conservative cache pass.
  - The targeted built-in overlay path now avoids the full `InstructionBox`
    clone before `Executor::Initial` dispatch; user-provided executors still
    use the owned fallback. Keep the broader borrowed-instruction execution
    rewrite separate unless a later post-crypto/decode profile again shows
    `Transfer::clone`, `WorldTransaction::apply`, or the concrete instruction
    handler clones as active costs.
  - Treat RBC authoritative-payload delays as symptoms of slow validation and
    materialization unless a later profile shows DA/RBC storage pressure,
    missing `BlockCreated`, or QC payload-missing counters.
  - Move FASTPQ worker budgeting and deterministic hardware-accelerated crypto
    investigation into the next tuning branch if the post-deferral profile
    still shows background prover Poseidon work competing with consensus. Keep
    the full borrowed-`Execute` executor API rewrite separate unless a later
    profile makes overlay execution dominant again.
- Turn the proposal-gap / queue-pressure investigation into a reproducible measurement pass.
  - Rerun the 7-peer load that previously advanced slowly or stalled under backlog.
  - Sample `/v1/sumeragi/status`, pending-block / commit-inflight metrics, and queue depths throughout the run.
  - Use a load generator that can actually sustain the target rate before changing worker/backlog tuning again.
- Rebaseline sorted asset-definition query performance.
  - Rerun `snapshot_ephemeral_sorted_asset_defs_first_batch` and `snapshot_stored_sorted_asset_defs_first_batch` on an isolated host.
  - If stored-mode still regresses, tune `stored_sorted_fast_start_params` / first-batch thresholds and keep the matching query tests aligned.
  - Restore a green `cargo test -p iroha_core` baseline for the query-performance branch after any tuning.

## Targeted follow-ups

- Migrate the remaining operator VPN workflows to submit the Torii-returned
  native `OpenVpnLeaseEscrow` and `SettleVpnLease` transactions, then retire
  the legacy in-memory receipt endpoint after a public relay/helper/Torii
  canary.
- Broaden Kura replay determinism beyond the current unit and consensus
  integration corridor.
  - Sidecar recovery semantics are now aligned with the memory-only WSV model:
    commit manifests and WSV checkpoints are optional verification metadata,
    while intact Kura blocks remain the recovery source of truth. Remaining
    work should prove replay equivalence from blocks, not make sidecars
    mandatory.
  - Commit-worker coverage now proves that injected WSV checkpoint and commit
    manifest write failures after state commit are reported as sidecar
    warnings, not ledger data loss or commit rollback.
  - The broad `integration_tests --test consensus_and_da` target is green after
    the memory-only WSV sidecar changes, including DA restart/rehydration and
    the mode-cutover and vote-QC regressions exposed by the first workspace
    rerun.
  - Add a multi-block replay fixture that replays committed blocks into a fresh
    state and compares canonical WSV snapshot bytes against the originally
    committed WSV.
  - The real 4-peer restart integration test now commits route-sensitive
    asset, account, alias, and domain-owned state, removes optional sidecar
    metadata, rebuilds from Kura, and compares the restarted peers' rebuilt
    query surface.
  - Keep the fixture on the replay-specific validation entrypoint so legacy
    blocks without embedded context remain covered separately from newly
    proposed blocks.
  - Add golden old-block Norito fixtures produced by a pre-context binary,
    rather than only synthesized absent-field decode tests.
  - Profile the post-commit canonical WSV checkpoint hash under sustained load
    and either record the accepted overhead or replace it with a cheaper
    committed state-root path.
  - If operators need a network-authenticated replay proof, promote the WSV root
    from a local Kura sidecar into block-committed or certificate-bound metadata.
- Broaden alias auto-renew mutation coverage beyond the focused onboarding grant.
  - Add an integration test proving a user-signed enable/disable update can mutate the subscription NFT created by onboarding.
  - If a non-onboarding mutation path still hits `Can't modify NFT from domain owned by another account`, capture the exact submitter, NFT id, and permission token shape before changing the permission model again.
- Add a live multi-peer multisig test for previously unregistered signatories.
  - Start from the existing materialization coverage in `integration_tests/tests/multisig.rs`.
  - Add a case where a signatory is materialized by registration and then successfully authors `MultisigPropose` / `MultisigApprove` on the network.
  - Assert transaction-authority shape and final instruction execution, not only account materialization.
- Extend and burn down the translation metadata audit backlog.
  - Refresh the translated `docs/formal/sumeragi/README.*.md` bodies after the
    English-only frontier formal and 2026-05-03 process-hardening updates so
    `python3 ci/check_docs_i18n_metadata.py --paths docs/formal --require-current`
    can be restored for formal docs.
  - The Sumeragi frontier model, process invariants, mutation suite, TLC
    cross-check, and longer nightly bound are wired, and CI now publishes a JSON
    metadata report for the stale translated formal READMEs; the remaining
    formal-doc task is translation refresh only.
  - Clean the existing `docs/source` and `docs/portal` metadata debt, including files missing `source_hash` and `translation_last_reviewed`, before adding those trees to the CI gate.
  - Refresh only the files the checker flags, then record the clean audit command in `status.md`.
- Add a recorded capture gate for the default `sora-temple` petal styles.
  - Use `petal score-styles` with a published style set, profile, seed, and minimum success ratio.
  - Record the JSON baseline in `status.md` and keep the default style honest under aggressive capture.
  - Only add a stronger default variant if the current `sora-temple` family cannot meet the agreed gate.
