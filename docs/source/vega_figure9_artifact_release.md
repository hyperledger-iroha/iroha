# Vega Figure 9 governed artifact corridor

The repository contains the canonical Microsoft Vega-MC Figure 9 engine and a
fail-closed artifact-and-evidence corridor. It does **not** contain the
full-shape proving key or verifier key. The Vega Exact12 operation therefore
remains unavailable until the external canonical pair passes this corridor and
a separate reviewed governance transaction is finalized.

`vega_figure9_artifact_tool` is the native authority for a candidate PK/VK
pair and its four mandatory evidence stages. It performs bounded decoding,
byte-for-byte canonical re-encoding, exact compiled-profile and dimension
checks, complete PK/VK component pairing, and the nonreplaceable Core install
preflight. After installation, it invokes the production Figure 9 prover and
verifier through `run_privacy_release_stage_v1` for the canonical positive,
public-statement mutation, proof corruption/truncation, and maximum-shape
resource cases. Every stage is admitted through the closed release-evidence
validator, canonically encoded as Norito, decoded and exact-compared in memory,
and written as an owner-only file. It has no setup generator, ambient key
lookup, or network input. A release build must embed all of these provenance
values:

- `IROHA_VEGA_SIGNED_SOURCE_COMMIT`: the exact 40-character SSH-signed Iroha
  commit;
- `IROHA_VEGA_WORKSPACE_SOURCE_MANIFEST_SHA256`: the reviewed source-manifest
  digest for that commit;
- `IROHA_VEGA_CARGO_LOCK_SHA256`: the unchanged reviewed `Cargo.lock` digest;
- `IROHA_VEGA_SOURCE_ALLOWED_SIGNERS_SHA256`: the reviewed SSH allowed-signers
  file digest;
- `IROHA_VEGA_SOURCE_REVOCATION_SHA256`: the reviewed signer-revocation input
  digest.

An ordinary build may compile the tool, but the tool refuses every
qualification request if any provenance value is absent, malformed, or zero.
Build and hash the tool from the frozen signed source, then stage the real
canonical PK and VK as distinct, singly linked, owner-only regular files
outside the source tree.

Create a candidate package with explicit canonical absolute paths:

```text
python3 -I -B scripts/package_vega_figure9_artifacts.py package \
  --native-validator /absolute/release/vega_figure9_artifact_tool \
  --expected-native-validator-sha256 <reviewed-validator-sha256> \
  --proving-key /absolute/private/figure9.pk \
  --verifier-key /absolute/private/figure9.vk \
  --output-root /absolute/private/candidate-packages
```

The packager authenticates and copies the validator and keys into an
owner-private staging directory before executing or decoding through them. It
then runs complete native qualification twice against only those copied bytes.
Each run must reproduce byte-for-byte all four of these archives:

- `vega-evidence-16-positive-canonical-end-to-end.norito`;
- `vega-evidence-17-public-statement-binding-mutation.norito`;
- `vega-evidence-18-proof-corruption-and-truncation.norito`;
- `vega-evidence-19-maximum-shape-resource.norito`.

The controller independently validates the closed stage coordinates, proof
lengths and SHA-256 identities, canonical decoder ceiling, statement hashes,
failure classes, and the fixed Figure 9 resource facts (2,359,296 application
constraints, 1,048,576 maximum circuit variables, and 21 relaxed sum-check
rounds). It seals the four exact Norito archives beside the keys and native
report, then atomically publishes a directory named by the canonical package
manifest SHA-256. Reverification always requires that externally retained
package digest and regenerates all four stages again:

```text
python3 -I -B scripts/package_vega_figure9_artifacts.py verify-package \
  --package /absolute/private/candidate-packages/<package-sha256> \
  --expected-package-sha256 <package-sha256>
```

The package manifest records
`native_release_qualification=passed-native-four-case`, while deliberately
retaining `availability=unavailable-pending-reviewed-governance`,
`network_activation_authorized=false`, and `release_boundary=candidate-only`.
The first field is valid only because the real native pair and all four exact
archives qualified. Packaging is not governance and cannot make Vega network
available.

The production compiled-profile catalog enforces the same boundary. Offline
qualification and evidence tools derive deterministic Vega candidate profile
material, but `compiled_privacy_profile_v1` returns `EngineUnavailable` until
one signed source release pins the exact PK and VK lengths and raw SHA-256s,
the derived artifact-manifest SHA-256, the content-addressed package SHA-256,
the four-stage evidence-set SHA-256, and the reviewed governance-authorization
SHA-256. Those pins are currently wholly zero/open. Process-local artifact
installation is an additional deployment requirement; its runtime `OnceLock`
state is never consensus authority and cannot open the catalog gate. While the
pins are wholly open, the explicit installer may accept qualified candidate
artifacts for evidence generation. Partially populated or inconsistent pins
fail closed. Once the pins are complete, both the verifier-only and prover
installers require the runtime manifest to match the exact source-pinned PK/VK
lengths, raw hashes, and artifact-manifest hash before any key bytes are lent
to the cryptographic installer.

The production activation tuple is also distinct from the candidate tuple.
Its engine-manifest digest binds a source-derived release digest covering the
exact PK/VK identities, artifact manifest, content-addressed package,
four-stage evidence set, and reviewed governance authorization. Candidate-only
proofs therefore cannot be presented as production activation evidence after
the release gate opens.

## Remaining release authority

Before changing Vega availability, the release still needs all of the
following external evidence:

1. the actual canonical full-shape Figure 9 PK/VK pair accepted by the native
   validator, plus the resulting v2 package and evidence-set digests;
2. a reviewed and signed governance transaction binding the package, native
   artifact-manifest, PK, VK, validator binary, signed source, source manifest,
   signer policy, revocation input, `Cargo.lock`, and four stage-archive
   digests;
3. state-preserving installation of the VK on every validator and the PK/VK
   pair only on the qualified prover owner, followed by one-validator-at-a-time
   restart and identical finalized state queries;
4. a committed Vega issuer record, the 300-block activation notice, one real
   terminal Vega credential-presentation transaction, exact semantic ledger
   evidence, replay rejection, and post-restart convergence on all four
   validators.

Until every item exists, the Exact12 driver and Taira rollout plan must retain
`MissingGovernedFigure9ProverArtifacts`; compact cubic fixtures, synthetic
keys, local preflight, HTTP acceptance, or an unfinalized governance proposal
do not close the blocker.
