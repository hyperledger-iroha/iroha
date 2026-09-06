# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-5f9b74bba24966e9b4689b26bee80c9b976644fc7ead4e366f983b6a1114d42b"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- BFV full-bootstrap release artifact binding now includes the typed arithmetic
  AIR constraint-system artifact in governed circuit material and
  artifact-bundle digests, and proof-key material now binds the non-circular
  evaluator artifact set it verifies. Crypto regressions now pin the proof
  public-input schema payload digest to exact typed Norito schema bytes, the
  schema artifact digest to the governed registered-profile envelope, the
  arithmetic trace profile, arithmetic AIR contract, and native proof-circuit
  fingerprint digests to encoded self-describing Norito material, the
  native prover/verifier generated circuit body digest to canonical body bytes
  under raw SHA-256 plus the embedded canonical AIR material bytes, the
  circuit-material, evaluator-artifact-set, and concrete artifact-bundle
  digests to encoded self-describing Norito material under their dedicated
  governance digest domains, and pin proof-key pair/material commitments to
  their explicit domain-separated proof-profile and key-material transcripts,
  with proof public-input schema artifacts now rejecting compressed or otherwise
  noncanonical Norito schema-payload framing, arithmetic AIR artifacts rejecting
  compressed or otherwise noncanonical typed AIR payload framing, proof-key
  artifacts rejecting compressed or otherwise noncanonical prover/verifier key
  payload framing, artifact-derived proof-key commitment helpers rejecting the
  same noncanonical proof-key artifact payload framing, typed evaluator
  artifacts rejecting compressed or otherwise noncanonical linear-transform,
  blind-rotation, accumulator, and sample-extraction payload framing, governed
  artifact envelopes rejecting compressed or otherwise noncanonical envelope
  framing before bundle validation, evaluator-set digesting, or artifact-derived
  proof-key commitment derivation, native proof-key material rejecting compressed
  or otherwise noncanonical framing around canonical native payloads, generated
  circuit-body validation rejecting compressed or otherwise noncanonical body
  and embedded-AIR framing, proof-key
  material envelopes rejecting compressed or otherwise noncanonical framing
  before key-material commitments can hash them, and individual material
  commitments decoding the canonical proof-key material envelope before hashing
  so opaque or noncanonical key bytes cannot mint governed commitments, while
  release-audit evidence, signoff, record, manifest, and package byte admission
  rejects compressed or otherwise noncanonical framing before caller-pinned
  audit bytes are hashed;
  the material/execution Soracloud public schemas now advertise their statement
  digest domains, those proof-key commitment domains, and the circuit-material,
  evaluator-artifact-set, and artifact-bundle digest domains bound by
  release-audit evidence, while the execution schema also advertises
  release-prover proof/prover-input,
  AIR-evaluation, trace-material, and AIR-constraint-system digest domains.
  Core's STARK/FRI AIR builder and verifier
  now accept explicit caller-owned trace rows and composition vectors, and the
  Soracloud release-prover handoff feeds typed BFV AIR evaluation material into
  finalized BFV-native execution proof attachments accepted by the governed
  verifier under the configured STARK enablement and proof/envelope byte caps.
  Material-native AIR uses the same explicit verifier corridor with
  verifier-reconstructed zero composition values, preserving the v1 FRI
  final-zero invariant while binding typed material through trace and
  composition roots; the material AIR composition helper now also checks row
  kind, row index, statement digest limbs, input-material digest limbs, and
  row seed limbs plus canonical Goldilocks field elements before accepting the
  zero-composition vector. A deterministic
  release audit evidence payload and digest now bind the generated
  artifact-bundle digest, evaluator artifact-set digest, prover/verifier pair
  commitment, native payload digests, and proof-profile
  field counts for release bundles, with regressions pinning release-audit
  evidence, record, manifest, and package digests to encoded self-describing
  Norito payloads under their dedicated domains, and a signed release-audit
  signoff payload now binds that evidence digest to the external audit
  report/archive digests and reviewer public key. Signoff validation can
  rederive the evidence from governed material and concrete artifacts before
  accepting the reviewer signature, and a canonical release-audit record now
  packages evidence plus
  signoff under its own digest domain for release archives. A release-audit
  package now carries the external report/archive bytes, checks them against the
  signed hashes, pins external report/archive digests to canonical headered
  artifact bytes rather than body-only hashes, rejects empty or all-zero audit
  artifacts, enforces bounded byte payloads, requires canonical v1
  report/archive byte headers with nonempty nonzero bodies, rejects blank or
  sub-64-byte audit artifact bodies,
  rejects canonical nested audit headers even after leading body whitespace,
  rejects placeholder-style audit artifact bodies across the full bounded body
  including draft, `not for production`, `not production ready`, and
	  `replace before production` markers with dash/underscore variants, and copied
	  report/archive bodies, including edge-whitespace- and
	  alphanumeric-normalized decorated copies,
  and requires
  caller-supplied trusted reviewer id/key validation before publication. The
  same package now carries a machine-checkable release audit manifest and
  manifest digest that
  require an approving verdict, canonical audit scope, signed record digest,
  evidence, artifact, evaluator-set, proof-key, prover/verifier-key,
  native-circuit, and report/archive commitment binding, and reviewer id/key
	  agreement before publication. The crypto release-audit validator now offers a
	  single governed-artifacts/trusted-reviewer/caller-pinned-digest gate that
		  rejects zero, known placeholder, or leading-whitespace
		  delayed-placeholder pinned package digests and record/manifest
			  digest aliases plus signed inner commitment aliases, including
				  native-payload commitments, before stale package validation or
				  comparison can mask the caller-pinned digest error; raw package-byte
				  and artifact/package-byte admission paths now exercise the same
				  record/manifest/native-payload alias rejection, and the gate preflights
				  caller-supplied reviewer id/key inputs, including malformed or
	  all-zero reviewer public-key payloads, before package or artifact validation
	  can mask malformed trust configuration. Shared execution-policy validation
	  now also recomputes embedded release package digests, rejects
	  post-signature reviewed report/archive byte mutations, and rejects stale,
	  placeholder, or leading-whitespace delayed-placeholder pinned digests plus
	  record/manifest digest aliases before runtime policy context can be
	  admitted, and Core's runtime release-audit context pins the same
	  package-digest sentinel rejection before artifact execution. Standalone release-audit
	  signoff, record, and manifest trusted-reviewer validators now share that
	  caller trust-anchor preflight before stale signed objects are parsed, and
	  signoff payload construction preflights reviewer id/key plus external
	  report/archive digests before stale evidence can mask malformed operator
	  inputs. Release-audit record and package construction now reject malformed
	  reviewer ids, non-Ed25519 reviewer signing keys, and all-zero Ed25519
	  reviewer private-key payloads before public-key derivation, evidence
	  derivation, or audit-byte validation, and package construction now shares
		  the report/archive byte-pair preflight so edge-whitespace- and
		  alphanumeric-normalized copied audit bodies fail before evidence
		  derivation or record signing.
	  Release-audit package validation and digesting now also reject placeholder
	  stored record/manifest digest sentinels before canonical digest
	  recomputation can collapse them into generic mismatch diagnostics, and the
		  material/execution public-input schemas advertise those package digest
		  sentinels plus caller-pinned record/manifest digest alias rejection under
		  pinned schema hashes, and Core audited prover tests pin the wrapper-surfaced
		  record/manifest alias diagnostics. Package digesting now also rejects tampered signed
	  report/archive bytes through the same digest mismatch gates as package
	  validation. External-review fixture package/digest construction now routes
	  through the crypto marker-enforcing builder, while deterministic
	  machine-generated report/archive bytes now label the audited proof-profile
	  field count, transcript-label obligations, release-prover digest domains,
	  and proof-key material/pair commitment domains, remain structural evidence
	  material only, and are rejected by the production-pinned path.
		  Core's audited material and execution prover wrappers require that gate before
			  native BFV proof attachments are emitted, including copied report/archive body
			  rejection after edge-whitespace and alphanumeric normalization plus refresh transcript
		  public-key digest validation at the wrapper boundary; the material-native
		  AIR builder now replays generated envelope bytes against the governed
		  material AIR context before wrapping; execution witness material also carries a domain-separated
	  Galois-key-set digest canonicalized by automorphism power so artifact-aware
	  replay rejects same-shape stale automorphism key substitutions before
	  proof-input or release-prover package hashing; lower-level typed material/prover-input helpers stay internal, and
	  the material and execution public-input schemas advertise the package-digest
	  pin, package-level header-only/nested-header/whitespace-prefixed
	  nested-header, zero-body, blank-body, padded zero/blank-body, and
	  placeholder/case-decorated/whitespace-prefixed/delayed-placeholder
	  external-digest rejection,
		  nested, whitespace-prefixed nested, blank, sub-64-byte, and
		  full-body plus leading-whitespace delayed-placeholder and
		  separator-obfuscated material-digest and placeholder audit-artifact body/digest-only rejection, manifest/signoff/record
		  external-digest alias rejection for signed native-payload commitments, canonical audit
		  artifact-header/body and distinct-body requirements, and the execution
	  witness Galois-key-set binding. Standalone release audit evidence
	  validation now also rejects reused artifact/profile/native-payload commitments
	  plus empty/all-zero and short, long, padded, binary-decorated,
	  case-decorated, whitespace-prefixed, delayed-content placeholder,
	  generated hyphen/dot/underscore separator-spelled handoff, draft, `replace-me`,
	  `changeme`, `stub`, `test-only`, `your-*`, `sample`, `template`,
	  `example`, `not for production`, `not production ready`, or
	  `replace before production` variant placeholder
	  native-payload digest sentinels, with explicit material/execution
	  schema markers for direct and whitespace-prefixed sample/template/example
	  native-payload digest sentinel rejection,
	  governed material digest admission rejects the same direct and
	  deterministic delayed-content
	  draft/not-for-production/replacement/handoff/sample/template/example/mock/fixture marker family
	  before circuit material,
	  proof-key material envelope/profile metadata, blind-rotation accumulator
	  material, caller-expected material proof-profile digests, material/execution
	  proof-input statement hashes, public-padding AIR rows, release-audit evidence,
	  signoff, manifest, or caller-pinned package digest slots can pass,
	  standalone record construction plus signoff/manifest validation rejects
	  external audit digest aliasing with signed release commitments plus known header-only, nested-header,
	  whitespace-prefixed nested-header, padded zero/blank-body,
	  short/long/padded/binary-decorated/case-decorated/whitespace-prefixed
	  placeholder report/archive digests, and generated hyphen/dot/underscore
	  separator-spelled audit-artifact marker variants, and
	  public crypto helpers build canonical report/archive bytes from
	  externally supplied bodies while shared body extraction enforces nested
	  headers and delayed placeholder text before release tooling packages them. Manifest adversarial coverage now also exercises stale manifest
	  version/field-count values, stale manifest-authorized package version/count
  values, padded scopes, and rejected verdicts through direct validation,
  manifest digesting, package validation, and package digesting. Core audited
  material and execution prover regressions now also prove rejected manifest
  verdicts and delayed nested report/archive audit headers stop at the
  release-audit gate before native proof attachments are emitted. The execution
  public-input schema advertises that distinct evidence-commitment requirement.
  The BFV AIR composition evaluator now derives
	  per-row/column challenges from the public statement hash, canonical
	  row-major trace-material digest, row index, and column index, remapping zero
	  challenges to one so residuals are bound to the evaluated witness package,
	  and the release-prover
	  input digest path now hashes AIR evaluation material only after the same
	  trace-bound composition-vector validation and hashes witness, embedded
	  proof-input, plus outer release-prover material only after artifact-aware
	  prefix-trace replay; the artifact-aware exact and bounded prefix
	  constructors now validate constructed traces and aggregate prefix-bound
	  vectors plus assembled witness material before returning them and reject
	  inert all-zero trace material, invalid propagated bounds, or stale witness
	  packages, while direct/artifact-aware execution preflight rejects inert
	  all-zero ciphertext inputs before artifact fallback or prefix execution at
	  the executable crypto boundary;
	  material-proof input digests now also have a
	  caller-bound path that reconstructs the package from caller-owned
	  evaluation keys and artifacts before hashing, and Core's material native
	  AIR handoff consumes that caller-bound digest before proof emission. Core's
	  execution native AIR handoff now consumes the artifact-bound release-prover
	  digest before proof emission as well. The material and execution native AIR
  proof wrappers also decode the native STARK/AIR envelope before attachment
  construction and reject transcript-label, circuit-id, missing-AIR-section,
  or public-digest/statement-hash drift before proof validation can rely on
  the wrapper. The shared BFV-native verifier now also has a public-padding
  entry/context path that rejects zero, direct-placeholder,
  leading-whitespace delayed-placeholder, and direct/delayed separator-spelled
  placeholder statement hashes and
  checks the canonical parameter-profile/domain-tag, statement hash, public
  slot-capacity/bound-mode header, canonical public openings, and zero public
  composition samples without private row-major trace material; Core's active
  full-bootstrap execution admission now consumes that
  verifier-facing path before governed
  trace/composition replay, and the release-prover trace-material replay helper
  now uses the same public-padding gate before explicit row/composition
  verification; that gate now also preflights the public trace-material digest
  through the transcript-derived opening schedule so zero, direct-placeholder,
  delayed-placeholder, or direct/delayed separator-spelled placeholder trace digests fail before
  generic envelope replay, and Soracloud native BFV AIR boundary coverage pins
  the same zero/direct/delayed-placeholder/direct-or-delayed-separator-spelled trace-digest
  rejection before release-prover envelopes are accepted, with the same separator-spelled
  trace-digest preflight covered in AIR evaluation material, composition
  challenge, direct/delayed separator-spelled material/execution proof-input
  statement hash validation, and execution prover-input validation, including
  proof-input digesting plus AIR artifact and AIR evaluation digest slots. The
	  shared STARK/AIR
			  prover and verifier now derive duplicate-free query schedules by
			  bound-specific transcript rejection sampling without replacement,
						  require noncanonical transcript labels, including BFV
						  native AIR suffixed-label aliases, malformed domain tags,
					  and malformed AIR or verifier-key circuit ids to fail closed before query replay or envelope verification,
					  keep caller-provided verifier limits from relaxing canonical
					  STARK structure and envelope-byte caps, and reject
					  blowup/domain parameter pairs where `blowup_log2` exceeds
					  `n_log2` before proof synthesis, verifier-key admission, or
					  envelope verification,
			  while failing closed when a duplicate-free schedule cannot exist, so
			  duplicate openings cannot reduce effective sampling. Native-AIR proof
		  synthesis now also rejects nonzero final FRI folds before BFV-native or
		  public AIR proof bytes are returned. The BFV material and execution
		  native-AIR builders still retry bounded statement/material-domain
		  query nonces for privacy-policy public-row constraints before returning
		  duplicate-free proof envelopes, and the BFV execution wrapper now has a
		  structurally valid generic-AIR negative control proving allowed transcript
		  labels with private-row openings fail the BFV no-unmasked-private-row
		  policy before native acceptance. The ZK-ACE native AIR prover now routes
		  generated query chains through the same duplicate-free validator and
			  self-verifies encoded envelopes before returning proof bytes. BFV native
			  STARK/FRI proof-key material, verifier payloads, and release-audit proof
			  profiles now also reject blowup/domain parameter pairs where
			  `blowup_log2` exceeds `n_log2` before key material or evidence can be
			  admitted. Shared STARK/FRI verification now keeps auxiliary generic
			  composition payloads (`comp_root`/`comp_values`) scoped to the generic
				  binding AIR context and rejects them for caller-owned explicit AIR and
				  ZK-ACE AIR before statement replay, and generic sidecars must rederive
				  the AIR public digest from strictly ordered auxiliary terms before their
				  composition leaf is accepted. Caller-owned explicit AIR trace roots now
				  reject non-canonical Goldilocks row elements before hashing, so malformed
				  row material cannot be bound under an otherwise valid explicit AIR
				  verifier context. STARK `OpenVerifyEnvelope` wrapper
			  verification rejects inner auxiliary sidecars for both generic binding
			  and ZK-ACE wrappers, keeping generated wrapper proofs canonical.
			  Generic STARK `OpenVerifyEnvelope` construction and verification also
			  require verifier-key payloads to meet the ledger-grade production FRI
			  floor and verifier-key backend labels to exactly match the requested
			  proof backend before wrapper proofs can be emitted or accepted.
			  ZK-ACE STARK circuit ids are classified after backend normalization,
			  and public generic AIR construction plus generic STARK wrapper
			  construction reject ZK-ACE circuit aliases before they can fall back
			  to binding AIR; ZK-ACE proving, verification, and preverify/dedup
			  admission also reject noncanonical backend labels plus malformed
			  canonical ZK-ACE public-input or wrapper shapes before cache
			  insertion.
			  BFV full-bootstrap STARK circuit ids are now classified the same way,
			  so public generic AIR construction plus generic wrapper construction
			  and verification reject BFV aliases before the generic binding AIR can
				  admit native full-bootstrap proof attachments without BFV-specific
					  public-opening checks, while Soracloud FHE attachment admission rejects
					  noncanonical STARK/FRI backend labels and base FHE verifier paths
					  reject wrong-circuit STARK verifier-key payloads, including native
					  metadata verifier payloads, before verifier lookup,
					  preverified-cache acceptance, or native full-bootstrap verifier
					  dispatch.
			  Wrapper verification also pins inner AIR transcript labels to the
			  canonical generic binding or ZK-ACE domain so regenerated proofs under
			  alternate STARK transcript domains fail closed.
			  The STARK `ivm-execution-v1` helper also checks the normalized circuit
			  id before wrapper construction, public generic AIR/wrapper construction
			  reserves IVM execution aliases, and wrapper verification pins IVM
			  payloads to the canonical schema plus 16 single-row commitment columns,
			  with preverify/dedup admission enforcing the same IVM wrapper shape
			  before cache insertion,
			  so matching non-IVM verifier keys cannot be used to emit IVM-shaped
			  proofs for another circuit or arbitrary generic schema.
			  Governed
			  full-bootstrap material admission
			  now also rejects known nonzero pending, placeholder, native proof-key
			  payload, draft, not-for-production, and replacement digest literals before
			  artifact, proof-key pair, key-material envelope/profile metadata, blind-rotation
			  accumulator material, coefficient/slot linear-transform diagonals,
			  sample-extraction switch-key digit limbs, wrong-secret and
			  key/sample-mismatch sample-switch diagnostics, all-zero/malformed/stale
			  evaluator artifact-set envelopes, opaque evaluator artifact payloads,
				  placeholder or duplicate evaluator/bundle digest-material fields,
				  extra, missing, or stale full-bootstrap execution Galois keys,
				  malformed same-schedule full-bootstrap Galois key-switch entries before
				  ciphertext or bound metadata use, inert all-zero
				  Galois/relinearization key-switch entries,
			  BFV public-key digest, seeded-encryption, identifier
			  public-parameter/ciphertext slots, all-zero BFV public-key
			  components, bootstrap statement,
			  full-bootstrap material statement, refresh-transcript, public/secret
			  consistency public-key material, all-zero secret-key material, and full-bootstrap execution
			  claim/trace ciphertext and raw-sample material,
			  bootstrap public-key digest metadata,
			  evaluation-key bundle digest refresh masks, bootstrap zero-refresh
			  proof-statement refresh ciphertexts,
			  aliased execution witness digest commitments,
			  caller-expected material proof-profile digests,
			  material/execution proof-input statement hashes, public-padding AIR rows,
			  or release-audit evidence
				  commitments can be accepted, including standalone release-audit signoff
				  and manifest commitments, release-audit key evidence rejects placeholder key
				  digest/material commitments plus inert native-payload digest sentinels
				  including generic proof-key placeholders, native proof-key material admission rejects oversized raw payloads before placeholder scanning, raw placeholder text, leading/trailing binary-decorated raw placeholder text, and digest-correct inert native
				  payloads including the same generic placeholder bytes. Native-payload,
				  material, and report/archive inert/placeholder artifact digest gates use
				  deterministic cached sentinel tables for repeated admission checks,
				  and execution proof
				  statement hashing rejects the known pending, direct or delayed
				  separator-spelled pending, delayed, or transient pre-finalization
				  execution witness digest literals. Generated
					  execution claims still use that transient value only internally before
					  deriving the governed digest. The BFV native
					  STARK/AIR prover/verifier wrapper now derives the domain tag from the
					  execution statement hash, pins the BFV native AIR transcript label to
					  the canonical base label, rejects suffixed-label alternate proof
					  encodings, pins the canonical circuit id and FRI profile,
					  and rejects sampled openings unless they are the statement-bound
				  public-padding rows; the Core execution native-AIR boundary also
				  requires governed trace rows and AIR composition values before root
				  reconstruction, sampled opening replay, Merkle/FRI validation, or
					  dedicated-verifier fallback; the Soracloud execution proof wrapper
						  now replays the same canonical base-label preflight before native
						  AIR envelopes are packaged into execution proof attachments, and
						  the typed crypto proof schema and execution public-input schema
						  require that canonical base-label/suffixed-label rejection contract; the
						  material-native AIR boundary now
				  also checks sampled Merkle path shape, commitments, and FRI/AIR
				  values before dedicated verifier dispatch; release-audit reviewer admission now
				  requires Ed25519 public keys across signoff, record, manifest, and
				  package validation, rejects empty or all-zero reviewer public-key
				  payloads before stale signed objects can mask malformed trust
				  anchors, fail-fast Ed25519 reviewer signing keys during
				  record/package construction, rejects duplicate production
				  external-review report/archive marker fields plus
				  case-insensitive same-statement marker-token replays and
				  padded-colon marker replays before trusted package admission,
				  rejects non-printable marker-statement bytes, marker-token
				  separator reviewer ids, and separator-alias reviewer labels,
				  and advertises that trusted-reviewer
				  payload contract in the Soracloud full-bootstrap schemas; Soracloud
				  FHE execution-policy validation now shares those reviewer-id and
				  reviewer-public-key preflights, so placeholder reviewer ids fail on
				  the trusted-reviewer policy field before package-level matching, and Core's
				  full-bootstrap release-audit runtime context replays that preflight
				  before governed artifact execution, while shared artifact preflight
				  rejects valid artifacts if that context is absent after preserving
				  narrower artifact drift/role/key-material diagnostics, and the raw artifact-aware crypto execution/bound
				  helpers are crate-private so external fixtures route through the
				  release-audited helper surface; Torii signed FHE job preflight
				  now pins the same field-specific rejection order for placeholder
				  reviewer IDs and non-Ed25519 reviewer keys before package matching;
				  the material
				  and execution full-bootstrap public-input schemas also advertise the enforced
				  release-audit evidence/signoff/record/manifest, proof-profile,
				  non-production native-payload text/digest sentinel,
				  artifact-binding, trusted-reviewer, and caller-pinned
				  non-production/placeholder package-digest
				  subcontracts under pinned material
				  `37c84b7bec1f3a0c414754fcafd146d3c0160e0f77dda80d3c9d33317c959789`
				  and execution
				  `0f32eb03923145dec3264be23a757180ccab899c2a428050b5a53aa674817f0d`
				  schema hashes; release-audit evidence now also binds registered
				  BFV profile digests and canonical proof-schema/AIR artifact
				  envelope digests before aggregate evidence recomputation; crypto now exposes release-audit-gated exact and
				  bounded artifact-aware execution plus bound helpers that require
				  the caller-trusted reviewer id/key and caller-pinned package
				  digest to validate before governed artifact execution or public
				  bound propagation proceeds, and Core's release-audited execution
				  prover now recomputes caller-supplied outputs and bounds through
				  those audited helpers before native proof construction. The signed
				  Soracloud FHE execution policy and Torii/Core runtime path now
				  carry the same release-audit package, digest, trusted reviewer id,
				  and reviewer public key binding, so `FullBootstrapV1`
				  artifact-backed jobs fail before exact or bounded artifact
				  execution when that trusted audit context is missing, stale, or
				  caller-pinned to non-production package digests, including
				  leading-whitespace delayed-placeholder package-digest sentinels;
				  release-audit artifact bodies now
				  reject delayed nested audit report/archive headers anywhere in the body,
				  reject delayed handoff/sample/template/example/mock/fixture audit-body markers before
				  native material or execution proof generation, and signed digest-only gates
				  reject deterministic delayed placeholder report/archive sentinels while
					  advertising the broader package-byte gates in the same schemas;
					  generic STARK `OpenVerifyEnvelope` admission now
					  refuses to route that BFV circuit through the binding AIR fallback,
					  including bare `stark/fri` and alternate production-profile aliases.
			  Soracloud BFV input-admission, bootstrap-key, full-bootstrap material,
			  and execution proof attachments now require the canonical BFV STARK/FRI backend
			  (`stark/fri/poseidon-x7-goldilocks-6x64-v1`) and advertise that backend in their
			  public-input schema descriptors, so alternate production STARK profiles
			  cannot satisfy governed BFV proof gates.
				  BFV full-bootstrap proof-key profile validation also rejects known
				  placeholder/draft/not-production/handoff/sample/template/example/mock/fixture sentinel hashes
				  plus internal transient before-finalization commitment hashes
				  in the registered parameter/RNS/decomposition profile, pair, and
				  material commitment slots
				  before commitment recomputation or governed material matching, and
				  generated proof-key construction no longer uses a known pending
				  material-commitment sentinel while deriving canonical pair and per-key
				  commitments. Proof-key metadata and material envelopes now also carry
				  the native verifier-floor obligations advertised by the generated
				  circuit body/fingerprint, and both material and pair commitments bind
				  those ordered obligation flags.
				  Target-limb bounded multiplication now also rejects structurally valid
				  centered scale-round source chains that are not evaluator prefixes before
				  malformed relinearization-key or ciphertext payloads.
			  Artifact-aware BFV execution witness validation now reports the first
			  mismatched governed trace/bound field, and regressions pin diagnostic
			  slot-to-coefficient plus sample-switch output drift as artifact-only
			  replay failures rather than shape-only witness-material failures.
			  The BFV AIR composition challenge contract/schema now pin the challenge
			  domain, statement hash, trace-material digest, row index, column index,
			  and nonzero remapping policy, so regenerated artifacts cannot silently
			  fall back to statement-only or coordinate-agnostic composition streams.
					  Full-bootstrap artifact profile decoding now mirrors encoder/evaluator
					  admission for hand-crafted release envelopes by rejecting all-zero
					  outer envelopes and non-empty all-zero inner payloads before
					  role-specific Norito decoding, including the pre-material proof-key
					  commitment helper used during release material derivation.
					  Release-audit evidence digesting now also rejects matched stale
					  prover/verifier generated-circuit body digests through the public
					  evidence digest helper, not only standalone evidence validation.
					  Release-audit signoff payloads now also carry that generated-body
					  digest as a signed commitment and validate it against evidence
					  before manifest construction, with regressions covering
					  stale signoff generated-body commitments and runtime-gate
					  rejection before exact/bounded execution preflight.
					  Bounded-noise exact-RNS and target-limb basis-extension bootstrap
				  round-zero wrappers are now covered by the same chain-before-shape
				  preflight regressions as indexed and multi-round refresh paths.
				  Bounded-noise bootstrap proof-statement coverage now also rejects
				  all-zero nonzero-index refresh rounds and proves refresh-round
				  tampering or reordering changes the bounded statement digest.
				  Transcript-bound bootstrap proof-statement coverage now also proves
				  the bounded-noise API rejects exact encrypted-zero refresh masks
				  before deriving bounded transcript statements.
					  Bounded transcript proof-statement coverage now also binds
					  bootstrap round count/key material and rejects reordered
					  nonzero-index refresh rounds during deterministic transcript
					  validation.
					  Exact transcript proof-statement coverage now mirrors that
					  round-count/key-material binding and reordered-round rejection
					  before a transcript-bound statement digest is emitted.
					  Full-bootstrap evaluation-key bundle coverage now also proves
					  `FullBootstrapV1` material stays on the material-proof path:
					  transcript inventory digests admit no-seed material binding,
					  while exact and bounded zero-refresh transcript statement APIs
					  explicitly reject full-bootstrap keys and supplied deterministic
					  bootstrap transcript seeds.
					  Governed `FullBootstrapV1` keys now also use the
					  `full_bootstrap_key_from_material_v1` no-refresh constructor and
					  must carry `max_refresh_rounds = 0`, empty `zero_refresh`, and
					  empty `round_refreshes`; crypto/Core admission, proof-statement,
					  execution preflight, and release-audited prover paths reject mixed
					  full-mode/encrypted-zero-refresh key shapes before package
					  digesting or artifact execution.
				  Release-prover input regressions now also pin all-zero
				  arithmetic trace material and arithmetic AIR contract digest
				  sentinel rejection before stale digest comparisons.
				  Core BFV-native public-padding verification now also rejects
				  auxiliary generic composition sidecars before treating an AIR
				  envelope as a canonical public verifier proof, and malformed
				  proof/AIR metadata, opening-path/sample drift, parameter-profile
				  drift, and caller limit regressions now pin the same public-padding
				  rejection boundary. Crypto-side public-opening schedule tests also
				  reject zero trace-material digests, direct/delayed/separator-spelled
				  placeholders, binary-framed `0xff` placeholder trace digests,
				  placeholder statement hashes, missing next-row openings, and stale
				  caller slot/bound-mode context before Core verifier use. Core
				  BFV-native governed AIR verification
				  now also requires a verifier-owned expected trace-material digest in
				  the public-padding context, so missing or mismatched digest pins fail
				  before governed trace row replay or preverify-cache use. The
				  execution prover-input package now carries the trace-bound
				  public-opening material plus digest, so stale or trace-retargeted
				  opening packages fail inside typed release-prover validation, and
				  Core's artifact-bound BFV AIR verifier compares decoded native AIR
				  openings against that typed package before accepting the envelope.
				  The canonical crypto proof public-input schema now advertises the same
				  verifier-owned trace-material digest obligation and rejects schema
				  downgrades before generated proof-key artifacts can bind stale
				  schema bytes. Proof-key metadata, proof-key material envelopes,
				  native generated circuit bodies, and native proof-circuit fingerprints
				  now also bind the typed public-opening material validation obligation
				  before proof-key material or generated native payloads are admitted.
				  Canonical governed circuit material, arithmetic trace-profile
				  material, AIR contract material, proof public-input schema
				  material, execution Galois key-set, artifact-bundle archive,
				  material proof-input, execution witness, execution proof-input,
				  trace, AIR evaluation, public-opening, and release-prover input
				  material byte admission now rejects compressed,
				  reordered, or otherwise noncanonical Norito framing before circuit
				  material, artifact-bundle material, Galois key-set, witness, material
				  proof-input, proof-input, or release-prover material bytes are hashed.
				  Execution proof-input byte admission now also has
				  caller-public-key-bound companions, including a public-key/proof-input
				  byte-pair handoff that rejects decoded key substitution before
				  governed artifact replay. Release-prover input byte admission mirrors
				  that caller-public-key boundary with public-key/prover-input byte-pair
				  handoff before governed proof-key replay.
				  Execution proof/prover governance-byte handoffs now also decode
				  public-key, evaluation-key bundle, artifact-bundle, Galois-key-set,
				  and proof/prover input bytes together before returning decoded
				  release material.
				  Material proof governance-byte admission now also preflights the
				  decoded public key against the `FullBootstrapV1` bootstrap public-key
				  digest before artifact-bundle or proof-input bytes are admitted.
				  Release-audit key evidence now also rederives canonical role-specific
				  native prover/verifier payload digests from the audited circuit id
				  before evidence, signoff, record, or package validation can accept
				  signed native payload commitments. Signed release-audit payloads and
				  manifests now also repeat those canonical native prover/verifier
				  payload digests so reviewer signoffs and published manifests expose the
				  payload commitments directly. Release-audit evidence archives now also
				  repeat the signed canonical prover/verifier native payload digests so
				  externally reviewed archive bodies cannot omit or drift those payload
				  commitments while still carrying canonical payload hex.
				  Release-audit evidence derivation from artifact-bundle bytes can now
				  return decoded artifacts, derived evidence, and the evidence digest
				  from one canonical byte stream.
				  Evidence/artifact-bundle byte-pair admission can now also return the
				  decoded pair plus the evidence digest derived from the admitted
				  evidence bytes.
				  Signoff/evidence byte-pair admission can now also return both
				  the evidence digest and signoff digest derived from the admitted
				  byte streams before trusted-reviewer checks.
				  Signoff/artifact-bundle byte-pair admission mirrors that boundary
				  after deriving evidence from canonical artifact bytes, returning the
				  derived evidence digest and admitted signoff digest before
				  trusted-reviewer checks.
				  External-review package builders can now also start from canonical
				  artifact-bundle bytes before signing and returning the admitted
				  artifact-bundle digest plus caller-pinnable package digest.
				  Deterministic release-audit report/archive inventory builders can also
				  start from canonical artifact-bundle bytes before emitting the generated
				  byte pair.
				  Deterministic report/archive builders can now also return the generated
				  report and archive digests from the same typed or artifact-bundle byte
				  generation path.
				  Deterministic release-audit package builders mirror that boundary before
				  returning generated packages, admitted artifact-bundle digests, and
				  caller-pinnable package digests.
				  Signed release-audit record builders now mirror the same artifact-byte
				  boundary before returning signed records and record digests.
				  Trusted record validators now enforce reviewer identity on the same
				  canonical artifact-bundle/record byte handoff.
				  Record/artifact-bundle byte admission can now also return the decoded
				  pair plus the artifact-bundle and record digests derived from the
				  admitted byte streams.
				  Trusted manifest validators now extend that handoff across canonical
				  artifact-bundle, manifest, and record bytes.
				  Manifest/record byte admission can now return the record and
				  manifest digests, and artifact-bundle/manifest/record byte admission
				  can also return the artifact-bundle, record, and manifest digests
				  derived from their admitted byte streams.
				  Release-audit signoff validators now also derive evidence from
				  canonical artifact-bundle bytes before trusting signed reviewer payloads.
				  Release-audit manifest builders now start from canonical record or
				  artifact-bundle bytes before returning manifests and manifest digests.
				  Release-audit package builders now also accept canonical signed
				  record/manifest bytes before returning externally reviewed packages and
				  package digests, with richer helpers returning the record, manifest,
				  and package digests from the same admitted byte streams.
				  Package-byte production admission can now derive the caller-pinnable
				  package digest directly from canonical package bytes at both decoded
				  artifact and artifact-bundle byte boundaries, and the artifact-bundle
				  byte boundary can return the admitted artifact digest with it.
				  Material proof governance-byte admission can now return the
				  public-key, evaluation-key bundle, artifact-bundle, and proof-input
				  digests derived from the same canonical byte handoff.
				  Execution proof-input and release-prover governance-byte admission can
				  now also return the public-key, evaluation-key bundle, artifact-bundle,
				  Galois key-set, and proof/prover-input digests derived from one
				  canonical byte handoff. Exact and bounded admission proof-input
				  byte-pair validators can likewise return public-key, ciphertext, and
				  proof-input digests from the same canonical handoff.
				  Raw native payload admission now requires exact canonical uncompressed v1
				  encoder bytes before those commitments are accepted. Soracloud
				  material/execution proof public-input schemas now mirror the same
				  release-audit contract with signoff payload count `18`, manifest count
				  `23`, and evidence-archive native payload digest requirements.
				  Release-audit archive field-index tests
				  now also require native prover/verifier payload hex and generated
				  circuit body hex labels/values plus generic governed artifact hex
				  labels/values to be canonical lowercase, rejecting raw
				  native/generated/governed artifact bytes, separator-delimited byte
				  text, and duplicate governed artifact aliases before package-level
				  validation; generated/native payload archive validators now reuse
				  the governed artifact hex decoder before comparing decoded bytes.
				  Release-audit artifact placeholder scanning now also prefilters
				  bodies by possible marker-leading bytes before running the broader
				  case-insensitive and separator-spelled marker scans. Full-bootstrap
				  artifact-vector regressions now also pin marker tokens split across
				  non-text bytes at outer envelope, evaluator-set digest, builder payload,
				  and decoded inner-payload boundaries. Exact and bounded
				  release-audited runtime regressions now also prove deterministic
				  machine-generated release-audit packages with correct caller-pinned
				  digests cannot authorize execution or bound propagation before
				  external-review marker admission. Core's policy-pinned runtime
				  context now mirrors that gate before returning a release-audit
				  context to execution. Release-audit manifest/record byte-pair
				  admission now also rejects compressed framing, binary-split
				  placeholders, stale manifest commitments, and trusted-reviewer
				  drift before publishing or digesting manifest evidence.
					  Remaining BFV full-bootstrap production work is the
					  audited arithmetic proof-producing backend plus externally
					  audited generated prover/verifier artifacts and report/archive
					  production with canonical v1 headers and externally audited
					  nonzero generated-circuit bodies. The Core verifier,
					  governed circuit-material and proof-key/schema/native-envelope
					  corridors with constructor/schema/profile/advertised-commitment/envelope digest-alias preflights,
								  release-audit evidence-wide native-payload/key-evidence raw and embedded generated-body digest-alias/signoff/manifest/package/byte-admission preflights, signoff byte-pair and package-byte trusted-reviewer input preflights, artifact-byte record/manifest/package builder signer and audit-digest input preflights, record/manifest and artifact-bundle package audit-artifact byte preflights, combined package/artifact package-byte admission ordering, evidence/artifact evidence-byte admission ordering, signoff/artifact signoff-byte admission ordering, record/artifact record-byte admission ordering, manifest/artifact/record manifest-record byte admission ordering, caller-pinned package generated-body subcommitment alias preflight, and package builders,
					  reviewed-byte tamper preflights,
									  external-review marker gates, external report/archive and case-decorated material placeholder digest gates, native material/payload/generated-body
									  placeholder, proof-key constructor placeholder, expected-circuit preflight before native payload bytes, and raw native-payload/generated-body profile/schema/contract digest-alias preflights and circuit-fingerprint binding, public-opening/native AIR replay
					  with transcript-seed distinctness, public-opening/AIR-evaluation trace/constraint-bound cross-layer digest-alias preflights, and AIR-evaluation digest-alias
					  preflight plus admission/full-bootstrap
																	  proof public-key, full-bootstrap-key public-key/material/artifact/evaluator aggregates, artifact-aware public-key/artifact aggregates, execution-claim ciphertext/statement-role, execution-witness public-key, execution proof-input witness/ciphertext/proof-key-role, trace-material cross-layer, prover-input public-key/prover/verifier full proof-key-profile/pair-commitment, prover-input cross-layer/embedded-material, material-proof evaluation-key-bundle/proof-key-role digest-alias preflights, material/execution proof/prover governance input-byte admission ordering with material artifact-byte and execution artifact/Galois-byte preflight before proof/prover bytes, typed material-proof and execution witness/proof/prover artifact-context preflight, public-key-byte proof/prover artifact-context preflight before public-key/input bytes, witness-bound Galois key-set byte preflight before key-set decoding, context-bound material/trace/opening/AIR byte admission ordering, public-key/ciphertext statement capacity/declared-bound pre-decode admission, typed public-key/ciphertext proof-input caller-object/capacity/declared-bound pre-decode admission, raw proof-input byte-tuple capacity/declared-bound pre-decode admission before public-key/ciphertext/proof-input bytes, and admission proof-input ciphertext byte ordering with ciphertext-byte preflight before proof-input bytes,
					  Core verifier-key material envelope/key-binding admission, canonical STARK proof-byte admission,
						  artifact-byte archive matching, exact generated-body length scalars,
						  and policy-pinned Core/Torii artifact-preflighted release-audit
						  runtime gate are already shipped.
				  Soracloud public refresh-transcript metadata also rejects all-zero
					  rotation/bootstrap seeds before crypto refresh-key recomputation can mask
					  malformed inventory behind unrelated bundle-shape diagnostics, and the
					  bootstrap-key zero-refresh public-input schema advertises that rejection
					  under its refreshed pinned schema hash. Scalar/RNS exact and
					  bounded-noise refresh-only bootstrap key constructors reject inert
					  all-zero public-key material and zero or oversized refresh-round
					  capacities before deriving encrypted-zero refresh masks, the same
					  public-key preflight is advertised in the
					  bootstrap-key zero-refresh schema, and scalar/RNS exact and
					  bounded-noise outer-slot rotation, refresh-only bootstrap execution,
					  refresh-transcript admission, key-owner bundle diagnostics,
					  evaluation-key bundle digest-refresh admission, the shared exact
					  and bounded zero-plaintext diagnostic helpers, and residual-bound
					  diagnostic helpers now also reject inert all-zero
					  refresh masks, `zero_refresh` drift from
					  `round_refreshes[0]`, plus duplicate or all-zero per-round refresh
					  ciphertexts before applying, admitting, or summarizing public refresh
					  material.
					  Core audited release-package
					  wrappers also preserve field-level transcript diagnostics through exact/bounded
					  fallback and reject all-zero rotation transcript seeds before native proof
					  generation. Exact and bounded-noise bootstrap refresh keys also reject
					  advertised round capacities that exceed deterministic refresh headroom before
					  transcript recomputation or key-material use.

