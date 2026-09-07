# SCCP semantic circuits

This standalone Go module owns the eight fixed SCCP final-V1 Groth16 circuit
profiles. Ethereum, BSC, and TRON use BN254. TON uses BLS12-381. Each lane has
one message circuit and one epoch-anchor-update circuit, with an independent
key domain and circuit-specific Phase-2 ceremony.

The module is intentionally outside the Cargo workspace. It uses Go 1.25.7,
pins gnark v0.16.3 in `go.mod` and `go.sum`, and carries the complete `vendor/`
tree. It does not add a Rust dependency and must not update any `Cargo.lock`.

## Security boundary

This source revision is **not production-admissible**. The remaining
implementation, resource, ceremony, audit, and destination-verification blockers
are enumerated in `manifests/semantic-coverage-final-v1.json`. That manifest is
informational and cannot issue a release verdict. Only the repository's signed
Rust/Python production corridor may authenticate actual artifacts and declare a
release admissible; structural hashes are not substitutes for exact bytes,
trusted signatures, or BLS pairing verification.

No command generates a trusted setup, accepts a caller-selected verification
key, or emits an accepting smoke verifier. `emit-kat` only emits deterministic
public test vectors. The checked-in vectors are solver-tested, including a
negative mutation of each of the eleven public signals.

The implementation constrains the fixed `0x40`–`0x44` network tags and
lane/role configuration; canonical XOR transfer framing; an exact canonical
Taira I105 single-controller Ed25519 sender, including the full 105-symbol
alphabet, minimal base-105 representation, and Bech32m checksum; positive
amount and route revision; recipient family and route identity; exact payload
and lane-bound message hashes; and the final-V1 message tree with a canonical
proof length of at most 64 sibling levels. The
message-tree root is inserted into an exact bare-Norito
`BlockHeaderConsensusProjectionV1`; its Iroha hash must equal the block hash in
the Commit vote, closing root/finalized-block mix-and-match attacks.

Finality constraints cover the governed anchor, exact 4–31 `3f+1` roster and
`2f+1` signer rules, all eight canonical Commit-vote execution shapes, exact
HeightContextIdentity v5 and V2FinalityArtifact v4 bytes, compressed
BLS12-381 key/signature binding, and the aggregate BLS-normal Commit-vote
pairing. Message/current-roster paths bind durable PoP hashes through the
authenticated anchor. An epoch transition additionally verifies the newly
activated roster's complete PoP batch using an in-circuit Fiat–Shamir
challenge, then binds the exact parent CommitQC and successor snapshot. The
retained current anchor authorizes one exact epoch/roster at or before its
boundary; the independently verified boundary QC emits its exact successor as
the next retained anchor, so advances compose across multiple epochs. A
same-height anchor must name the exact block/context/artifact in both message
and epoch-boundary authorization. The same
BLS-normal equations pass a Rust-derived Iroha QC fixture in both outer fields.

The statement commitment covers the complete constrained semantic bundle. It
deliberately does not hash its own future Phase-2 VK: the post-ceremony fixed
verifier must compare its compiled VK/code identity with the route deployment
binding. That generator/wrapper, cross-runtime evidence, resource-budget
closure, ceremonies, and audits remain blocking. These constraints and passing
KATs are not a partial production trust decision.

## Local validation

Use the pinned toolchain and force the vendored module mode:

```sh
GOTOOLCHAIN=local GOPROXY=off GOSUMDB=off go test -mod=vendor ./...
GOTOOLCHAIN=local GOPROXY=off GOSUMDB=off go build -mod=vendor ./cmd/sccp-circuits
```

`constraint-count --profile <closed-id>` compiles one exact R1CS and emits its
constraint count, canonical `ConstraintSystem.WriteTo` byte length, and SHA-256
identity as one JSON record. Compiler diagnostics go to stderr. The command
hashes serialized bytes without retaining an extra output copy or writing an
R1CS file, and generates no setup or key material. The pinned gnark serializer
still buffers internal sections, so run large profiles sequentially and monitor
memory pressure. Tune `GOMEMLIMIT` and `GOMAXPROCS` against shared host headroom;
the Go memory limit is a soft GC target rather than a process memory cap.

The fixed catalogue and one KAT can be inspected without key material:

```sh
go run ./cmd/sccp-circuits catalogue
go run ./cmd/sccp-circuits emit-kat \
  --profile sccp-final-v1-ethereum-mainnet-message
```

`KAT.md` defines the strict public vector schema, signal order, and role-specific
label derivation consumed by independent release validators.

## Reproducible builder

`builder/Dockerfile` is pinned to the official Go 1.25.7 Debian linux/amd64
platform manifest. `builder/build.sh OUTPUT_DIRECTORY` invokes BuildKit with
`--network none`, `linux/amd64`, the vendored module mode, CGO disabled, a
fixed environment, and deterministic linker flags. It recomputes and compares
the SPDX SBOM and every vendored file before building. The output directory
must not already exist.

Run the builder twice from the same signed clean commit and compare both
`sccp-circuits.sha256` files. The canonical signed production corridor
additionally requires the complete independently authenticated source,
toolchain, ceremony, key, KAT, verifier, prover, and audit inventory.

## Ceremony policy

`manifests/ceremony-policy-final-v1.json` requires:

- one public Phase-1 ceremony per curve, exactly eight independent
  contributions, and a publicly announced future beacon revealed after all
  contributions;
- eight circuit-specific Phase-2 ceremonies with exactly eight independent
  contributions each;
- independent keys and fixed-key verifiers for all eight profiles; and
- independent semantic/cryptographic, reproducibility/ceremony, and
  destination-integration audits with no unresolved critical, high, or medium
  finding.

Any circuit, constant, dependency, builder, R1CS, or witness-compiler change
invalidates the affected Phase-2 artifacts. Ceremony receipts, audit reports,
guardian material, and production keys are external release inputs and must
never be replaced by repository placeholders.

Composable epoch-anchor authorization binds each transition to the retained
checkpoint's exact epoch and ordered roster. Message authorization also binds
the checkpoint block, context, and
finality artifact when the message finality height equals the retained
checkpoint height. Focused tests cover matching and independently mismatched
identities in both outer fields while retaining authorization for later blocks
in the same epoch. This consistency repair changes all four message R1CS
definitions without changing their positive KAT public values. The manifest
records canonical R1CS byte lengths and SHA-256 identities for freshly measured
profiles and explicitly lists any still requiring measurement. Earlier affected
Phase-2 artifacts, verifiers, and deployments remain invalid; fresh ceremonies
and independent audits are required even when every local R1CS identity is current.
The manifest also binds a deterministic definition-source and dependency closure;
the focused inventory test rejects freshness claims after those inputs change.

The circuit uses the canonical Rust wire namespace: TRON domain `5`, payload
codecs text `1`, EVM `2`, TRON `5`, TON `7`, transfer payload tag `2`, and
transfer hub-message kind `5`. These values are separate from network tags
and destination-proof backend tags. Focused transfer-parser tests reject the
retired compact identifiers in all four lanes; the shared Rust transfer fixture
also guards circuit, release-script, and contract constants against drift.
Namespace alignment changes message KATs and invalidates affected R1CS identities.
