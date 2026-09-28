# SoraFS native provider admission

The sole runtime authority is the council policy and provider history initialized
by signed genesis or enacted by SORA Parliament in Core. Torii constructs its registry from the same Core State
used by its other authoritative reads. Local files, configured public keys and
signed envelopes alone cannot authorize discovery, PoR, PoTR or gateway admission.

## Signed genesis initialization

`InitializeSorafsProviderAdmissionV1` carries a network-independent
`InitialProviderAdmissionCouncilV1` and up to 64 `InitialProviderAdmissionV1 {
owner, material }` entries, strictly ordered by provider id. Material is the canonical
`ProviderAdmissionGenesisMaterialV1` proposal, advert body and validity interval;
it contains neither a network id nor invented council signatures. The complete
instruction is capped at 4 MiB. Register universal owner accounts before it. The
initializer establishes an absent owner binding or requires the exact existing
owner; conflicting bindings, duplicate providers and invalid or expired material
fail before journal writes.

The signed genesis transaction has the explicit Genesis domain. After the signed
header exists, Core derives the actual network identity and the revision-one council
policy. The journal records the exact initializer entrypoint and canonical instruction
digest. Current reads authenticate the successful direct instruction from that exact
State/Kura genesis and its complete execution proof, anchored by the executed wire
the genesis frame's result-only commit certificate commits, then compare the
retained material against the original template. This avoids a genesis-hash fixed
point. Genesis material is explicitly distinct from council-verified envelopes;
offline material construction alone cannot authorize a provider.

Core accepts the initializer only as the exact direct instruction in the original
signed genesis transaction with an empty committed history and admission namespace.
A consumed direct-source marker excludes contract, trigger, sealed and batch
execution. A second initializer rolls back its containing transaction. Genesis
uses the same canonical policy, head, immutable history and resource counters as
Parliament. Ownership-only pre-genesis configuration cannot admit providers. Once
initialized, all council changes, additional providers, renewals and revocations
use Parliament and ordinary network-bound council signatures.

## Native effect and history

`ProposeSorafsProviderGovernance` carries the closed `Admission` action:

- `ConfigureCouncil` enacts the canonical `ProviderAdmissionCouncilPolicyV1` frame.
  The first revision is one; successors bind the immediate policy digest and retain
  the exact network and policy identity.
- `Admit` requires a currently governed provider owner, a policy-bound council quorum,
  an unexpired signed envelope and no retained provider admission history.
- `Renew` requires the current owner and exact preceding admission material digest and
  revision. The enacted current council authorizes the new envelope, including a
  changed advert key. Policy rotation requires renewal under the new policy before
  a provider can be admitted again.
- `Revoke` verifies the current council and exact current envelope, then writes a
  permanent tombstone. Revocation remains possible while the policy is paused.
  The provider identity cannot be re-admitted; recovery uses a newly governed identity.

Each effect has its own Parliament subject: owner bindings, council policy and
provider admissions do not share certificate compare-and-set heads. The normal
Parliament certificate lifecycle binds the exact effect and current subject head;
there is no post-genesis direct admission instruction or local administrative bypass.

Core stores immutable predecessor-linked records and the current head in native
transactional state. Writes include the exact network, committing height/time,
owner, revision and material. The current-head reader checks the retained record
and its predecessor, exact policy claims and provider owner, and requires matching
State/Kura block and certified-frame authentication (the certified-chain reader,
`specs/sumeragi.md` §12.7). A policy pause, owner change/removal, expired
envelope, malformed state or unavailable finality fails closed.
Runtime expiry uses the later of local time and the authenticated current block's
timestamp, so a lagging or rolled-back local clock cannot revive expired admission.
Local time must still satisfy the original issuance lower bound; committed time
does not turn a pre-issuance local clock into a valid request. These checks apply
to both signed-genesis material and subsequent council-signed admission records.

The V1 limits are 4,096 permanent provider identities and 1,024 transitions per
identity, with the last provider transition reserved for revocation. One canonical
admission frame is at most 1 MiB; a terminal revocation is at most 16 KiB. Normal
retained history has a 128 MiB byte budget and terminal revocations have a separate
reserved allowance. Exhaustion rejects the effect before any state write.

## Runtime behavior

Discovery validates against current finalized authority both when preparing and
committing an advert. Pruning reads the same authority, so revocation or renewal
is visible without restarting or editing files. A prepared old advert cannot commit
under a substituted current envelope. Reads remain local to the node's committed
history; this does not promise knowledge of a newer block that the node has not
received.

Advert replay checkpoints retain issuance floors for revoked native identities.
A restart cannot forget the native tombstone or restore admission by presenting an
old envelope. The offline registry constructors remain material-verification tools;
the production Torii constructor never selects them.

`[sorafs.discovery.admission]` has only `enabled`, for consumers that need admission
when discovery is disabled. Enabling discovery automatically enables the native
reader. Retired `envelopes_dir`, `trusted_council_keys` and `signature_threshold`
configuration fields are rejected; council keys and quorum belong in the enacted
policy. Existing development chains need the current governed policy and admission
effects; file-based fixtures do not seed production authority.

## Validation scope

Focused model tests cover canonical action frames, bounded rejection, substituted
provider identities and distinct Parliament subjects. Native tests exercise positive
admission, exact renewal, council rotation, terminal revocation, replay refusal,
owner removal, missing retained history and a missing or invalid commit certificate
on a certified test chain (real signed genesis, blocks executed by the node's
executor, BLS CommitQCs of a four-validator committee). Genesis tests derive the
network from the actual constructed signed Genesis-domain block and reject absent finality,
indirect execution, malformed templates and replay. These fixtures do not qualify live
consensus, multi-gateway deployment or operational failover. Current execution
results belong in the root status and closure ledger.
