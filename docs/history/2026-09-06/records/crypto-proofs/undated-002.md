# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-b421b7cfebb8ab32b0a6d552635ebf72121c5b148b9010a7aa190910dade8b0d"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- Continue the crypto-boundary audit after the completed signature decoder
  hardening: `Signature` JSON and Norito admission now rejects empty/all-zero
  payloads centrally, `BlockSignature` wire decoding routes through the checked
  path, data-model block-signature codec fixtures use checked nonzero payload
  construction, DA ingest, SM Norito, query/transaction tampered-signature,
  alias attestation, confidential-wallet,
  snapshot signature sidecar, identifier/ISI/Torii receipt, Connect bridge
  identifier sample, and Core transaction/gossiper tampered-signature fixtures
  use checked admission, explicit `BlockSignatureWire` conversion is now
  fallible and checked,
  direct BLS signature parsers reject all-zero signature bytes before backend
  parsing, Torii operator WebAuthn/ISO20022 P-256
  verifier paths reject zero-coordinate SEC1 public-key material before backend
  parsing, Torii operator WebAuthn
  ES256 assertions, and ISO20022 X.509/OCSP/CRL DER P-256 verifier helpers
  reject high-S DER signatures before backend verification, ES256 COSE
  registration validates keys before persistence, IVM ML-DSA
  helper/syscall/opcode/circuit verifier paths reject all-zero public-key
  material before backend parsing, IVM ECDSA circuit verification rejects
  all-zero public-key and signature buffers before k256 parsing,
  ML-DSA aggregate wrapper coverage now pins valid inputs plus tampered,
  all-zero-signature, all-zero-public-key, and mismatched-length rejection,
  SoraFS governance log publisher/block/head and PoTR Dilithium3 signature
  verification now preflight ML-DSA-65 public-key and detached-signature
  material through `iroha_crypto::mldsa65_parse_signature` before opaque
  signature wrapping, shared `iroha_crypto::mldsa65_parse_signature` and Connect
  identifier receipt parsing, Connect native/Java detached verification, and JS
  host `cryptoVerify` plus Python generic verification now preflight ML-DSA-65
  detached signatures before opaque signature storage or backend verification,
  with C FFI, JNI helper, JavaScript host, Python, and guarded Kotlin/Java SDK
  parity regressions pinning valid, short, overlong, and all-zero ML-DSA
  behavior where backend support is required while keeping
  Swift malformed key/signature length rejection, identifier-receipt ML-DSA
  attestation length rejection, and exact-length bridge-unavailable reporting
  independent of bridge availability,
  and Torii operator-auth ML-DSA header signatures plus P2P handshake ML-DSA
  signatures now use the same shared parser before request or handshake-payload
  verification, and data-model signed transactions,
  submission receipts, signed queries, validation-fee policies, SoraFS
  moderation manifests, SoraNet ticket/proof helper and VPN usage vouchers,
  RAM-LFE receipts/openings, Core/Torii Soracloud provenance signatures, Torii
  app-auth single-signature/witness payloads, Torii DA receipt operator
  signatures, Torii DA Taikai SSM
  publisher signatures, Torii signed query requests, caller-signed Torii
  SoraFS repair command transactions, SoraFS orchestrator Taikai
  cache-admission envelope/gossip
  signatures, Core oracle observation signatures, SoraDNS directory-builder
  signatures, runtime-upgrade provenance signatures, Core snapshot ML-DSA
  signature sidecars, Core
  fraud-attestation signatures, JDG simple-threshold ML-DSA committee
  signatures, and Sumeragi direct vote/merge/RBC/VRF plus peer trust-gossip ML-DSA
  signatures use it
  before digest, typed-payload
  verification, governance-policy verification, moderation summary acceptance,
  transport accounting, provenance-payload, app-auth wrapping/validation,
  offline JSON, DA receipt, Taikai signing-manifest, query-payload,
  repair-transaction payload, cache-admission payload, observation-payload,
  directory-record, runtime-upgrade-provenance, offline-certificate,
  attestation-hash, consensus-preimage, or trust-book mutation,
  ML-DSA public-key recovery from a typed secret rejects all-zero secret-key
  material before unsafe unpacking, and SoraNet signed-ticket
  sign/decode/direct-verify paths reject unsupported versions,
  the final signer-admission inventory classifies remaining raw signature
  constructors as fixed Ed25519 formats, signer-aware generic branches,
  non-Ed25519 fallbacks, BLS-only checked paths, signer-context-free decode
  staging, or fixtures/examples, with the July 4 continuation recheck keeping
  identifier output-opening JSON in signer-context-free staging while pinning
  malformed Ed25519 `R` and all-zero rejection at policy-key verification, and
  the operator/tooling subset recheck classifying remaining xtask, SoraFS CAR,
  fixture-generator, CLI example, and SoraNet/SoraDNS helper raw constructors
  as fixture/example or codec-staging bytes while validating the
  operator-facing Ed25519 helpers with malformed-`R` and all-zero tests, and
  the July 5 SoraFS CAR manifest-signature continuation removes the invalid
  Ed25519 signer `hex:` multihash fallback from detached signature-file
  verification and emission, and the July 5 Connect identifier-receipt
  continuation routes fixed Ed25519 RAM-LFE output-opening signatures through
  malformed-`R` and all-zero admission while parsing structured receipt JSON,
  the latest July 4 raw `Signature::try_from_bytes` pass found no new
  externally supplied Ed25519 verifier gap after rechecking P2P, Torii,
  Offline issuer, snapshot, fraud/jurisdiction, Sumeragi, Python/JavaScript,
  Connect, signer-context-free decode, BLS aggregate, fallback, fixture, and
  tooling categories, and the production-readiness corridor records focused
  crypto/route-sensitive validation plus full workspace clippy as passing,
  reruns the final constructor/algorithm inventories with no new production
  signer-aware gap, and closes the prior non-crypto consensus/DA blocker with
  the full serialized `consensus_and_da` target green locally,
  and, at that checkpoint, externally supplied Ed25519 signing seed parsers for
  Torii SoraFS stream tokens, SoraFS CAR provider adverts, SoraFS CAR
  provider-admission council secrets, and `sorafs-validate` signing commands
  rejected all-zero seed material before constructing dalek signing keys. The
  Torii stream-token file-seed parser and local key path were later deleted by
  the V1 hard cut; current issuance accepts only the node-TOML policy plus an
  exact runtime-injected signer/custody-provider binding. SoraNet
  guard-directory issuer rotation now fails closed if the generated Ed25519
  issuer seed is all zero before constructing a dalek signing key or reissuing
  relay certificates,
  the continuation revalidation keeps the remaining raw-signature inventory in
  that same classification and pins representative bridge, JavaScript host, and
  Torii app-auth Ed25519 malformed-`R` rejection plus SM2 zero-scalar boundary
  regressions after restoring the lane-scheduler type annotation required to
  compile dependent admission crates, and the July 5 central/data-model
  revalidation keeps generic signature decode algorithm-agnostic while pinning
  Ed25519 small-order/noncanonical `R` rejection through central verification,
  signed-query JSON, signed transactions, submission receipts, RAM-LFE receipts,
  and identifier-resolution receipts once signer context is available,
  and the IVM Ed25519 verifier/opcode, deterministic batch, CUDA/Metal staging,
  and Halo2 adapter paths now funnel public-key bytes through the same strict
  noncanonical/small-order/all-zero preflight before backend or accelerator
  verification, with the central parser plus SoraFS manifest, SoraNet relay,
  and xtask SRCv2 readiness helpers now pinning noncanonical non-small-order
  Ed25519 public-key rejection so canonicality coverage is independent of
  weak-key rejection,
  unrepresentable expiries, all-zero ML-DSA relay public-key material, and
  all-zero ML-DSA signing secret-key material before backend work, while
  SoraNet admission-token minting rejects all-zero ML-DSA signing secrets and
  issuer fingerprints that do not match the public key reconstructed from the
  signing secret before RNG or signing work, and SoraNet directory issuer
  fingerprint helpers reject malformed Ed25519 issuer public keys plus
  malformed or all-zero ML-DSA-65 issuer public keys before hashing, with
  SoraFS orchestrator guard-directory negatives pinned to fail before
  invalid-key fingerprint derivation,
  streaming `KeyUpdate` admission rejects all-zero, small-order, and
  noncanonical Ed25519 remote identity bytes plus small-order/noncanonical
  signature `R` encodings before transport-key or suite state changes,
  BLS/GOST/secp256k1/SM2/hybrid/blinding raw imports now reject inert all-zero
  material, the Rust SDK SM2 wrapper now pins all-zero seed/private-scalar
  rejection at its public constructors, hybrid ML-KEM public-key, secret-key,
  and ciphertext constructors reject all-zero component bytes before backend
  validation, BLS normal/small
  public-key parsers and aggregate verifier admission reject all-zero
  public-key bytes before backend parsing across both
  w3f and blstrs backends, VRF normal/small proof parsing rejects all-zero
  compressed proof bytes before `blstrs` decompression, IVM VRF verify syscalls
  reject all-zero, identity, and non-canonical G1/G2 public-key/proof encodings
  before pairing, IVM Ed25519 helper single/batch coverage now pins
  small-order and noncanonical signature `R` rejection, VM, CUDA/Metal, and
  Halo2 verifier paths reject inert public-key material
  before Dalek parsing, and the Halo2 Ed25519 circuit verifier now rejects
  malformed signature `R` encodings before Dalek signature decoding,
  CUDA single/stub and public-helper admission also fails closed on malformed or
  weak Ed25519 public-key bytes and small-order/noncanonical signature `R`
  encodings, Sumeragi VRF commit/reveal admission now routes externally supplied
  Ed25519 signatures through the same small-order/noncanonical `R` preflight,
  BFV full-bootstrap release-audit signoff validation routes reviewer Ed25519
  signatures through the same all-zero, short, and malformed-`R` admission
  before backend verification, Taikai signing-manifest validation routes embedded publisher
  Ed25519 signatures through the same malformed-`R` admission before
  segment-manifest verification,
  the IVM SM2 syscall and Rust SDK SM2 verify wrapper pin all-zero and
  zero-scalar signature rejection through the typed SM2 parser, shared SM2 SEC1
  public-key parsing rejects uncompressed all-zero coordinate material before
  backend parsing, central Ed25519 public-key parsing rejects all-zero,
  small-order,
  and noncanonical material before curve decompression, JS-host and Python
  crypto verification and public-key multihash helpers pin the same Ed25519
  public-key admission, the shared Ed25519 single verifier rejects noncanonical
  or small-order signature `R` encodings before dalek `verify_strict`, generic
  `Signature::verify` and typed `SignatureOf<T>::verify` coverage now pin the
  same malformed-`R` rejection for opaque Ed25519 signatures constructed from
  bytes, checked raw and typed signature hex, JSON, Norito-framed, bare binary,
  query/transaction wrapper decode, and signed-query/signed-transaction
  container decode admission now explicitly reject empty or all-zero external
  payloads, IVM syscall/opcode/batch TLV coverage now pins
  the same small-order and noncanonical signature `R` rejection before backend
  verification, RAM-LFE
  signed receipt and output-opening coverage now pins
  all-zero, short, small-order-`R`, and noncanonical-`R` Ed25519 rejection before signature
  verification, identifier resolution receipt, SoraNet ticket body+commitment
  signatures, SoraNet relay bandwidth proof signatures, and SoraNet VPN
  usage-voucher coverage now pin the same malformed-`R` pair for externally
  supplied Ed25519 signatures, core snapshot sidecars, fraud-assessment
  attestations, RBC
  ready/deliver messages, vote verification, direct vote-signature parsing,
  Core consensus-preimage and peer-trust gossip fixtures, Sumeragi vNext
  rechain/view-change and local commit-vote fixtures, merge-committee
  signature helper, oracle observation, SoraDNS,
  Kagemusha certificate, Soracloud provenance, and contract-manifest
  provenance tests now cover both malformed-`R` encodings from valid
  signatures, Torii DA receipt-log,
  transaction-batch deterministic precheck, signed-query routing, Soracloud
  provenance, and repair command transaction admission
  tests pin the same pre-backend admission boundary, Connect bridge native and
  Java detached verification plus `_with_alg` envelope/control encoders and
  algorithm-explicit identifier receipt signed attestations now pin both
  malformed-`R` encodings before accepting Ed25519 signature bytes, the
  Connect bridge native and Java detached verification helpers also reject
  all-zero, small-order, and noncanonical Ed25519 public-key material at parse
  time, and Connect identifier receipts keep nested RAM-LFE output openings
  outer-valid while
  rejecting malformed opening Ed25519 `R` at signer-context verification, and
  Torii offline v1/v2 JSON/base64 signature ingress and RAM-LFE
  identifier-resolution output-opening admission now pin the same malformed-`R`
  rejection for externally supplied Ed25519 signatures, Swift identifier-receipt
  Ed25519 verification rejects short, all-zero, small-order-`R`, and
  noncanonical-`R` signature material plus all-zero, small-order, and
  noncanonical resolver public keys before CryptoKit verification, the shared
  Swift offline Ed25519 helper rejects the same weak or noncanonical public-key
  encodings before CryptoKit verification;
  SoraNet
  SRCv2 certificate verification now routes Ed25519 signatures through that
		  strict `R` parser before backend verification, SoraFS manifest Ed25519
			  verifier helpers reject noncanonical or small-order signature `R` material
			  plus noncanonical, all-zero, or small-order public-key encodings with
			  distinct diagnostics, provider-advert CLI verification pins the same
					  malformed-`R` boundary, PoTR receipt validation routes Ed25519 receipt
						  signatures and provider-admission council signatures through the same
						  malformed-`R` preflight, gateway GAR compact-JWS
					  signatures use the same preflight, alias-proof council signatures reject
					  inert all-zero payloads and malformed `R` encodings before bundle
					  verification, alias-proof council fixture signatures use checked
					  opaque admission before digest verification, Connect wallet Ed25519 signature helpers route raw bytes through
					  the central malformed-`R` parser before storing opaque signatures, and
					  Connect FFI `_with_alg` encoders and native/Java detached verification helpers
					  reuse that Ed25519 admission path before frame/envelope encoding or signature
					  verification,
					  SoraFS stream-token verification now rejects small-order verifier keys before
					  Dalek strict verification,
					  SoraFS CLI manifest signature verification rejects
					  noncanonical/small-order signature `R` and public-key encodings through the
					  central Ed25519 admission helper before
			  Dalek verification, CAR `sorafs_manifest_builder` and chunker vector-exporter
			  signature-file verification pin the same malformed `R` rejection and chunker
			  manifest signature fixtures use checked opaque admission for
			  council signatures, SoraFS proof-token minting parses its nonzero
  scratch signature through the strict `R` helper before attaching the real
  Ed25519 token signature, structural proof-token decode and `try_encode`
  fixtures reuse that checked nonzero placeholder instead of inert all-zero
  filler, proof-token binary decode routes frame signatures through the strict
  `R` parser, direct proof-token signature verification re-parses stored
  signatures through the strict `R` parser before Dalek strict verification,
  and proof-token raw-byte verification routes public keys through central
  Ed25519 admission, Torii
  proof-token verify requests route supplied `verifying_key_hex` through the
  same central Ed25519 admission path, SoraFS CAR proof-token verifier raw-byte
  construction routes supplied Ed25519 keys through that admission path,
  SoraNet
  guard-directory and relay-tool issuer public keys route through the same
  central admission before relay certificate verification, xtask SoraNet
  gateway PQ readiness parses SRCv2 identity keys through central Ed25519
  admission, Torii
  SoraFS manifest-envelope validation rejects all-zero, noncanonical, or
  small-order envelope signer keys plus noncanonical or small-order envelope
  signature `R` encodings through the same verifier path,
	  SCCP EVM/BSC recoverable-signature codecs require Ethereum-style `27`/`28`
	  recovery ids plus nonzero in-range `r` and low nonzero `s` before secp256k1 recovery, SCCP
	  Solana vote-proof and TON validator-signature structural preflight rejects
	  all-zero 64-byte signature payloads before source-adapter admission, and
	  TON validator-signature transcripts reject the same inert payloads before
	  masterchain or validator-set transition transcript hashing, Torii app
  API detached `signature_b64` admission routes short, all-zero,
  noncanonical, or small-order Ed25519 buffers through the central
  malformed-`R` parser before transaction attachment, Torii app-auth header
  and body signature-base64 decoders require exact standard-base64 text and
  keep decoded bytes raw until signer-resolved Ed25519 admission rejects
  malformed-`R` encodings before opaque signature wrapping, the latest
  `Signature::try_from_bytes` inventory classifies the remaining
  production-looking raw constructors as signer-strict verifier branches,
  non-Ed25519 fallbacks, BLS-only paths, signer-context-free decode staging, or
  fixtures/examples rather than unpatched Ed25519 verifier gaps; Swift,
  Kotlin/JVM, Java Android, JavaScript, and Python Torii request builders
  mirror exact standard-base64 `signature_b64` encoding for contract, multisig,
  bridge-proof submit, and bridge-message submit requests, Iroha client
  operator-panel signature fixtures parse decoded header material through
  checked opaque-signature admission before verification,
  JavaScript standalone multisig instruction-builder DTO
  helpers, native Norito multisig DTO encoders, and JavaScript/Python SCCP
  bridge-proof submit helpers reject the same whitespace, missing-padding, and
  pad-bit aliases before preserving detached signatures, Torii app-auth
  multisig witness verification routes signer-resolved Ed25519 signature
  material through that same parser before backend verification, Torii operator
  signature headers apply the same Ed25519 `R` preflight after parsing the
  operator public-key algorithm, Torii operator WebAuthn Ed25519 assertions
  use the same malformed-`R` admission and now pin small-order/noncanonical
  public-key rejection before assertion verification; alias
  storage emits a checked nonzero placeholder
  attestation signature until real alias attestation signing is wired in,
  query signature archived Norito decoding now has fallible checked
  `QuerySignature`/`SignedQuery` admission instead of infallible decode
  expectations, DA receipt signing preimages use a checked nonzero operator
  signature placeholder before attaching the real receipt signature, data-model
  identifier receipt negative/tamper fixtures and the shared data-model
  attestation registration certificate constructor use checked nonzero
  transient issuer signature placeholders before attaching the real issuer
  signature, Torii Shared Connect
  examples and approve-control fixtures use checked nonzero wallet-signature
  placeholders, SoraDNS resolver positive RAD fixtures use checked nonzero
  operator/governance-signature
  material, Iroha DA ingest receipt fixtures, data-model DA commitment and
  block DA-bundle fixtures, core DA commitment/receipt/proof/store fixtures,
  Torii DA commitment/request fixtures, `iroha` client DA commitment request
  fixtures, CLI DA smoke receipt fixtures, Torii Connect approval fixtures,
  SoraDNS directory-record fixtures, JDG SDN commitment fixtures, xtask DA
  reconciliation, SoraNet relay positive and bandwidth-proof fixtures,
  core block validation DA sidecar fixtures, core state DA index and
  lane-lifecycle fixtures, Sumeragi block-created and main-loop DA
  spool/proposal/payload fixtures, telemetry DA fixtures, and SoraNet ticket
  envelope roundtrip fixtures use checked nonzero
  signature material, xtask OpenAPI manifest, rANS table,
  SoraFS, SoraNet rollout/testnet, FastPQ, Ministry tool verifier paths, the
  Norito fixture exporter, and the SoraFS DA reconstruction fixture
  regenerator route decoded or generated signature material through checked
  admission, Kotlin fixture generation and Iroha/SoraFS
  examples now verify generated signatures through checked admission, Python
  native crypto verify, Connect wallet signatures, and wallet-provided
  transaction finalization now reject malformed Ed25519 `R` material before
  backend verification or typed storage, the follow-up core DA/autoscale sweep
  closes the scale-in cleanup, unknown-lane hydration replay, and duplicate DA
  receipt proposal blockers under focused reruns, xtask OpenAPI manifest,
  rANS table,
  SoraFS manifest/gateway fixture, and I3 proof-handler verification route
  Ed25519 signatures through the same malformed-`R` admission, JS host native
  crypto verification and P2P handshake hello verification route remote
  Ed25519 signatures through the same
  malformed-`R` admission before backend verification, peer-trust gossip
  records now pin malformed Ed25519 `R` rejection before trust-state updates,
  DeFi oracle attestations route Ed25519 signatures through the same
  malformed-`R` admission and reject all-zero, small-order, or noncanonical
  signer public-key encodings before generic verification, JDG
  simple-threshold attestations route Ed25519 signer
  signatures through the same malformed-`R` admission before threshold
  verification, Oracle observation typed signatures route provider-resolved
  Ed25519 material through the same malformed-`R` admission before
  `ObservationBody` verification, fraud assessment attester signatures route
  Ed25519 material
  through the same malformed-`R` admission before assessment verification,
  snapshot signature sidecars route Ed25519 material through the same
  malformed-`R` admission before digest verification,
  Sumeragi RBC READY and DELIVER peer signatures route Ed25519 material through
  the same malformed-`R` admission before RBC preimage verification,
  Sumeragi vote-verifier single-signature fallbacks route Ed25519 material
  through the same malformed-`R` admission before vote preimage verification,
  World ISI manifest provenance, runtime-upgrade provenance, and domain
  endorsement signatures route signer-resolved Ed25519 material through the
  same malformed-`R` admission before backend verification,
  Soracloud service/app/config/secret/rollout/FHE/decryption/training/model
  provenance signatures route signer-resolved Ed25519 material through the same
  malformed-`R` admission before backend verification,
  Torii Soracloud app-facing provenance signatures route signer-resolved
  Ed25519 material through the same malformed-`R` admission before backend
  verification across service, app-infra, config/secret, FHE, training/model,
  uploaded-model, decryption/query, rollout, agent, HF, and model-host request
  paths,
  SoraDNS resolver-directory builder signatures route signer-resolved Ed25519
  material through the same malformed-`R` admission before draft or publish
  verification,
  Torii operator signature headers parse Ed25519 material through the same
  malformed-`R` admission before freshness or backend verification,
  Torii operator WebAuthn Ed25519 assertions use the same malformed-`R`
  admission before assertion verification,
  the shared SoraFS manifest Ed25519 signature helper routes raw signature
  material through the central malformed-`R` parser, and the shared SoraFS
  orderbook, provider-admission, alias-proof, gateway GAR, and PoTR receipt
  verifiers now wrap Ed25519 signature material through that
  parser before backend verification,
  Governance DAG publisher signatures pin the same malformed-`R` rejection
  before governance node verification,
  signed SoraFS replication orders pin malformed-`R` rejection before capacity
  assignment verification,
  SoraFS POP credential issuer signatures pin malformed-`R` rejection before
  credential admission,
  SoraFS stream tokens route Ed25519 signatures through malformed-`R` admission
  before gateway token verification,
  SoraFS moderation reproducibility manifest signatures route Ed25519 material
  through malformed-`R` admission before manifest-body verification,
  SoraNet VPN usage-voucher signatures route Ed25519 material through
  malformed-`R` admission before voucher-body verification,
  SoraFS node gateway PoR proof signatures route Ed25519 material through the
  same malformed-`R` admission before proof-digest verification,
  Core SoraFS council-envelope approval signatures route Ed25519 material
  through malformed-`R` admission and council signer keys reject all-zero,
  small-order, and noncanonical Ed25519 public-key encodings before manifest
  approval verification,
  Connect wallet-signature, Nexus wallet finalization, and wallet-provided
	  transaction finalization paths now check external signature buffers before
		  storage or backend verification, data-model signed transaction, sealed
		  commitment, and multisig member signatures route signer-resolved Ed25519
			  material through malformed-`R` admission before typed payload verification,
			  transaction submission receipts route signer-resolved Ed25519 material
			  through all-zero, short, and malformed-`R` admission before receipt
			  signing-bytes verification,
					  SignedQuery Norito and JSON wrapper conversion paths route
					  signer-resolved Ed25519 material through malformed-`R` admission before
					  query payload verification,
						  Torii's signed-query request verifier routes signer-resolved Ed25519
						  material through malformed-`R` admission before authenticated query
						  execution,
							  Torii SoraFS repair command transactions route signer-resolved
							  Ed25519 material through malformed-`R` admission before native
							  instruction authorization,
						  Torii durable DA receipt-log operator signatures route signer-resolved
						  Ed25519 material through malformed-`R` admission before receipt payload
					  verification,
					  validation-fee governance policy signatures route signer-resolved Ed25519
					  material through malformed-`R` admission before threshold accounting,
	  SoraFS gateway conformance
	  council-envelope and attestation evidence verification now rejects inert
  decoded signature material before backend verification, RAM-LFE
  output-opening, execution-receipt, and identifier-resolution receipt
  verification now re-admit stored signature material through algorithm-aware
  checked admission before typed verification,
  SoraNet relay PoW expiry-overflow errors map to the clock-error telemetry
  bucket instead of leaving the failure-reason classifier non-exhaustive,
  SoraNet relay guard-directory loading, directory tooling, and handshake
  certificate config parsing now reject all-zero or small-order issuer Ed25519
  public-key material before verifier-key construction, SoraNet VPN
  helper-ticket parsing plus `sora-vpn-helper` metadata decoding now pin
  all-zero, small-order, and noncanonical Ed25519 metering public-key rejection
  before ticket acceptance, Torii VPN quote/session metering-key parsing now
  pins all-zero, small-order, and noncanonical Ed25519 public-key rejection,
  data-model account-address canonical single-key and multisig decode now pin
  all-zero, small-order, and noncanonical Ed25519 controller-key rejection, and
  core smart-contract P-256 assertion-key validators reject all-zero coordinate
  material before backend parsing.
  The follow-up raw constructor inventory now also covers
  `Signature::from_bytes`, `SignatureOf::from_signature`, `Signature::from_hex`,
  and direct Dalek `Signature::from_bytes` sites; remaining hits are
  test/fixture mutation helpers, non-Ed25519 paths, placeholders, or locally
  preflighted decode-to-verify paths. The SoraFS CLI manifest verifier and
  shared SoraFS manifest verifier helper now alias their post-preflight dalek
  conversions as `DalekSig`, so raw Iroha signature-constructor scans no
  longer conflate those strict Ed25519 verifier boundaries with unchecked
  opaque `iroha_crypto::Signature` admission.
  The 2026-07-03 SoraNet continuation rechecked the current
  `Signature::try_from_bytes` inventory after the ticket and relay
  bandwidth-proof hardening and found no additional production Ed25519 verifier
  gap in that raw-parser set. The direct custom-codec follow-up also inspected
  the IVM verifier family, SoraNet certificate parsing, SoraFS proof-token and
  manifest helpers, and CAR/stub manifest-signature helpers; those paths already
  use `ed25519_parse_signature` or local checked helpers where externally
  supplied Ed25519 bytes reach backend verification. The continuation
  validation also rechecked SoraFS governance/PoTR, RAM-LFE, Torii app-auth,
  signed-query, SoraFS proof-token, and CLI manifest-verification surfaces
  without finding a new production verifier gap. Remaining work is to keep that
  classification current as new custom protocol codecs are added and route any
  future external Ed25519 verifier bytes through checked constructors before
  release.



<a id="record-14b0652846a8605bfa6ef98ec0b0f21e6af4df4276bcd0d146c4dc28ecb1a678"></a>

- ZK asset light-client readiness now has a Torii `POST /v1/zk/merkle-path`
  endpoint for current confidential-v2 commitment inclusion paths, and the
  Kotlin/JVM, Android Java, and Swift Torii Merkle providers call it directly.
  Swift also has a local audited-frontier provider for offline callers.
  Torii-backed providers verify returned sibling paths against requested
  commitments and roots, and SDK path models enforce leaf-index direction
  consistency before wallet or prover code receives paths. The SDK Torii
  clients also reject quoted or fractional numeric fields in zk roots/path
  responses so wallet code sees the same integer shapes the node emits. SDK
  response parsers must also keep node-supplied Merkle paths structurally
  exact: duplicate keys, overflowing counters, non-lowercase fixed32 hex,
  missing witness nodes, depth/array mismatches, root mismatches,
  direction-bit mismatches, reordered commitment responses, and
  `leaf_index >= frontier_len` fail before wallet code can consume proof
  material. Keep local providers limited to audited caller-supplied frontier
  material and rejecting duplicate commitments or mismatched root history. The
  SDK parity guard now pins the Kotlin/JVM, Android Java, and Swift Torii
  parser-shape tests so duplicate-key and non-canonical numeric regressions
  cannot be dropped from the focused SDK lanes.

