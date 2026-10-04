# Local DKG private checkpoint owner

`PreparedDkgSecretsCheckpointV1` prepares two envelope buffers, the private record
singleton, committee-sized accepted-share backing and both Norito controls before
claim or RNG. Core owns three independent phase banks in the original operation pool.

The private fixed-field canonical Norito record stores hybrid recipient secrets,
up to 11 triple polynomial coefficients and up to 31 ordered private contributions.
Unused slots are zero. Only the immutable ChaCha20-Poly1305 nonce/ciphertext/tag
envelope leaves the secret owner. Its key derives from the original BLS lifecycle
private owner; plaintext backing is erased on completion, rejection and drop.
This software defense provides no hardware erasure guarantee.

Canonical AEAD context binds purpose, network, transition, authority generation,
session, ordered roster, seat, lifecycle owner, provider handle and revision,
original schedule and wider cutoff, exact authorization/execution source, immutable
producer-intent hash, signed public output, authenticated input and prior checkpoint.
A raw crypto context cannot authorize a protocol operation. Core's private-field
checked attempt and checkpoint owners authenticate the original signed body or
native proof and derive the frozen schedule; the crypto primitive verifies context
binding and original private equations.

Signed H1 authorizes generation only. Its source variant has no result R. Ordinary
phase sources require the original native finality graph; H1 execution needs genuine
H2 authentication. A copied hash or caller-constructed source enum grants no Core
authority. The binding has no retired raw-hash or JSON compatibility decoder.

Restoration uses the sole generated Norito field walk. Checked hybrid parsing verifies
both original public components; original validated Schnorr commitments verify every
coefficient equation; accepted contributions verify their exact ordered dealers and
recipient. No RNG, reproof, new capsule, signature or replacement backing is used.
Generation moves its original opaque owners once while retaining the exact funded
ciphertext for the next checkpoint relation. Core restores original signed rows into
the signature buffers and ledgers funded before the claim.

The daemon publishes original claim, canonical producer intent, private checkpoint,
public output and complete head in that order, with file/directory sync at each write.
The intent commits the original suspend-inclusive native expiry and OS boot identity,
claim root/child/path and inherited input-stream identities inside AEAD. Same-boot
restart restores only a complete generation head before any input marker or later
producer intent. It cannot open a new FIFO, reroll, re-sign or grant a fresh deadline.
Writer refusal retains the original source, descriptor, pointer and progress. The
runtime polynomial retires only after its exact delivery output is durable.

TODO: restore all later original input/proof cursors, delivery and acceptance graphs,
aggregate/extraction owners, and an independently authenticated rollback-resistant
head. A complete generation file set authenticates content, not deletion of an entire
later suffix by the disk owner. An interrupted later producer remains closed pending
its own restoration or certified cancellation. The bootstrap publication-to-genuine
native-execution graph and all-seat restart remain qualification boundaries; these
component controls do not establish a network release or settlement claim.

Primitive file/drop/restore tests run on all hosts; only Unix owner-mode operations
are conditional on Unix and they make no Windows ACL claim. Whole-candidate formal,
DA, liveness, workspace, SDK and disposable-network gates remain required.
