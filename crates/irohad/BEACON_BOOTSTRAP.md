# Native beacon bootstrap contract

`iroha3d_taira beacon-bootstrap` is an offline, centralized four-seat custody
owner. It uses the production Core DKG and existing runtime credential codec;
it never submits transactions, changes a ledger, or resumes secret state.
The intended Taira owner operates four validators in the Linux guest on the
MacStadium host in Dublin.

`provision` requires `--request`, `--genesis-manifest`, `--genesis-signed`,
`--genesis-public-key`, `--observed-height`, `--height-fd`, and a new `--output`
directory. The public request schema is
`iroha.global-beacon.bootstrap.request.v1`: `dkg_session` is the existing native
`GlobalThresholdBeaconDkgSessionV1`, `target_roster` and `authorization_roster`
are ordered native peer arrays, and `provider_handles` plus `provider_revision`
bind four distinct production provider slots. Both rosters must equal the
verified native height-context BLS/PoP roster, ordered by validator identity;
caller insertion order does not select seats. The signed mode must be NPoS.
This command does not attest a changed retained network roster. The network identity is the independently verified signed
genesis hash, with the fixed Taira chain ID and discriminant 369.

A controller must authenticate every supplied committed height. The initial
height must be inside the native sharing phase. The dedicated inherited pipe
carries strictly increasing decimal heights, one per line; descriptors 198–200
are reserved for credentials. Fresh dealer material remains in memory, every
recipient contribution is verified during sharing, and dealer polynomials are
erased before waiting. Finalization uses the actual observed height after the
native response phase. EOF, malformed/decreasing heights, cryptographic errors,
or the single `--timeout-ms` deadline abort the ceremony; the default is 180000
and the maximum is 3600000. An aborted, uninstalled ceremony requires new public
preparation and fresh randomness. It has no process-resumable secret snapshot.

The new output directory and seat subdirectories are mode 0700. Each `seat-N`
contains the existing mode-0600
`iroha-global-beacon-partial-signer-v1.norito` credential. Directory descriptors
bind writes, existing paths are never overwritten, and path replacements fail.
`sharing-snapshot.json` is public; `public-bundle.json` is published after all
four credentials. Success requires both that final bundle and exit zero.
The bundle schema `iroha.global-beacon.bootstrap.bundle.v1` contains the request,
complete public genesis proof, finalized public DKG record, unsigned installation
certificate, and each seat's public handle/revision/inventory digest.

`sign-install --bundle ... --signer-index N --key-fd 198 --output ...` independently
checks that public bundle and uses the existing owner-private, single-link,
71-byte canonical BLS key record. The inherited temporary copy is consumed,
zeroed, and truncated by the shared native loader; the persistent supervisor
original is not passed. Authorization indices are zero-based. The output is one
public native lifecycle signature, not an account transaction.

Alternatively, `--config-fd 198` consumes an owner-private copy of the native
validator TOML (at most 1 MiB). The two descriptor options are mutually exclusive.
The native configuration reader projects only explicit inline chain, discriminant,
BLS public/private key and `genesis.expected_hash`; these must match the bundle
and selected zero-based authorization seat. It rejects `extends` and external
consensus-key/genesis-identity selectors. Unrelated onboarding, faucet, streaming
and registry paths are neither opened nor treated as signing authority. The
persistent validator config stays with the supervisor; only the disposable
inherited copy is scrubbed and truncated.

`assemble-install --bundle ... --signature ... --signature ... --signature ...
--output ...` requires exactly three valid, ordered, distinct authorization
signatures. It emits a native instruction array containing
`ApplyThresholdKeyLifecycleCertificateV1`. The certificate's effective height
is the final observed height plus one, strictly before the first signed NPoS
mandatory pulse, with no active predecessor; the normal
on-chain lifecycle verifier remains authoritative. The maintained once-only
transaction workflow owns submission and its durable journal. Actual required
bootstrap operations must advance the DKG phases; empty blocks, fabricated
observations, and no-op carrier transactions are not part of this contract.

Before readiness can become true, each validator's exact public provider binding
and matching retained credential must be installed through the existing
supervisor credential lifecycle (FD200 launch copy). Initial setup can run with
beacon readiness false so the real installation transaction can commit. This
command does not weaken mandatory beacon pulses or provide a retained-network
missing-session bypass.

The adjacent native tests cover fresh four-seat provisioning, native custody
import and signatures, lifecycle quorum, malformed public inputs, phase/deadline
failure, descriptor/path custody, and consumed authorization keys. Runtime and
end-to-end qualification remain separate from source review.
