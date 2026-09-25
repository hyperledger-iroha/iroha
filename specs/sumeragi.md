# Sumeragi consensus

Sumeragi is the Byzantine-fault-tolerant consensus protocol of Iroha. There is one version of
this protocol; it is called Sumeragi. The sans-IO core crate `crates/iroha_sumeragi` implements
it, with its deterministic simulator; this document is the contract of that core and of the node
driver it relies on. The node still runs the v2 runtime ([`specs/sumeragi_v2.md`](sumeragi_v2.md))
until the cutover goal S4 of [`specs/sumeragi_goals.md`](sumeragi_goals.md) completes.

Wire and storage formats are fresh (Taira is reset); nothing here is compatible with the v2
runtime. Revisions 2, 3, 4 and 4.1 applied adversarial reviews and the decisions taken on them
(review logs: Appendices A–D). Appendix E reconciles the text with the implementation: the rules
the simulator and code review showed to be incomplete are amended in place, and every other
code-level choice is listed.

Normative keywords: MUST, MUST NOT, SHOULD, MAY. "Honest" = follows this spec. Notation:
`h` height, `v` view, `I` instance id, `n` committee size, `B` block, `R` execution result,
`g` genesis height, `H(x)` the chain hash (production: `iroha_crypto::Hash`, 32 bytes), `‖` byte
concatenation, `be64(x)` 8-byte big-endian, `be32(x)` 4-byte big-endian, `be16(x)` 2-byte big-endian.

---

## 1. Model and assumptions

1. **Validators.** Height `h` of instance `I` has a committee `C_h`: an ordered list of `n_h ≥ 1`
   distinct consensus public keys in *canonical order* (ascending by `kb(pk)`, §3.1). `C_h` is a
   pure function of committed state (§10). Every committee member has one equal vote. Signer
   indices in certificates index canonical order, never a topology.
2. **Fault threshold.** `f_h = floor((n_h − 1) / 3)`, quorum `q_h = n_h − f_h`. For `n = 3f+1`
   this is `2f+1`. For other sizes the formula still holds: n=4→(f=1,q=3), n=5→(1,4),
   n=6→(1,5), n=7→(2,5), n=20→(6,14), n=22→(7,15). Two quorums intersect in at least
   `2q − n = n − 2f ≥ f + 1` members, i.e. in at least one honest member. `n = 1` is allowed
   (f = 0, q = 1: a single validator commits alone; used for dev). `n ∈ {2,3}` has f = 0.
   The implementation MUST use `q = n − f` for every quorum and certificate. The only other
   count is the R2 anchoring threshold `2f + 1` (§7.4), computed from `f` and never through
   `quorum`; `2f + 1` MUST NOT appear anywhere else in code.
3. **Adversary.** At most `f_h` members of `C_h` are Byzantine (arbitrary behaviour, colluding,
   adaptive scheduling of the network before GST, full knowledge of the protocol and topology).
   Everything else is honest but may crash and restart any number of times. Durable storage
   survives a crash. Storage that is *lost* (safety record deleted, block-store tail missing, or
   both at once) makes the node abstain from signing wherever it may already have signed (§7.4 R2,
   R6). Storage that is *corrupt* halts that instance (§12.5). A safety record is never rolled back:
   record files are never restored from backups or copied (§7.4 record provenance); a rolled-back
   record is outside the model, exactly like a second Byzantine node.
4. **Network.** Partial synchrony: there is an unknown Global Stabilisation Time (GST) after which
   every message between honest validators is delivered within an unknown bound `Δ`. Before GST
   messages may be dropped, delayed, duplicated, reordered. Any peer (member or not) may relay,
   drop, strip or corrupt any *unsigned* part of a message. Channels are authenticated (the P2P
   layer tells the core the sending public key, which is the sender's consensus key). The core
   never relies on channel authentication for safety: every safety-relevant object is signed or
   bound by a signed hash, including the probe replies that anchor a key whose record was lost
   (signed echoes, §3.3, §7.4 R2). The authenticated sender is used only for per-peer rate
   limits, replies and fetch sources.
5. **Clocks.** Each node has a local monotonic clock (`now`, milliseconds) with bounded drift
   (`|rate − 1| ≤ ρ`, default assumption ρ ≤ 1%). Clocks are never compared across nodes and no
   validity rule depends on a local clock.
6. **Cryptography.** EUF-CMA signatures with aggregation. Production: BLS12-381 in the min-pk
   setting, `iroha_crypto::Algorithm::BlsNormal` (public key = compressed G1 point, 48 bytes;
   signature and aggregate = compressed G2 point, 96 bytes). Proof-of-possession is checked by the
   application when a key enters a committee, so rogue-key attacks are excluded. Collision-resistant
   `H`. The simulator uses a fake scheme with identical interfaces and provenance tracking (§13).
7. **Execution.** `exec(S_{h−1}, B) → Valid(R, S_h) | Invalid` is a deterministic pure function of
   the committed parent state and the block; it terminates within a chain-parameter budget
   (`E_max`, enforced by gas/size limits). `R` is a 32-byte execution commitment (§4.1). A local
   executor *failure* (panic, I/O error, resource exhaustion) is not an outcome of `exec`: it is
   reported as `Failed` and never treated as invalidity (§4.2).
8. **Instances.** One core instance per chain: the global (Nexus) chain and every dataspace /
   lane chain run the *same* core with their own committee and instance id
   `I = H("sumeragi/instance" ‖ genesis_hash ‖ chain_id_bytes ‖ kind:u8 ‖ be32(index))`
   (kind 0 = global, 1 = dataspace, 2 = lane; derivation is the application's; the core only sees
   32 opaque bytes). `I` is in every signing preimage and every block header. Instances never
   wait on each other inside the core, and the driver isolates them (§12.3 O9).

Properties (per instance):
- **Agreement:** no two honest nodes commit different `(block_hash, R)` at the same height.
- **Validity:** a committed block extends the committed parent, was proposed by the leader of its
  origin round, its payload bytes are the ones its header commits to, and its committed `R` equals
  `exec(S_{h−1}, B)`.
- **Liveness:** after GST, if at least `q_h` members of `C_h` are honest and running, every honest
  running node commits height `h` within a bound derived from the configuration (§8.2). A member
  with an unanchored key (§7.4 R2) counts as faulty until it anchors, and a member whose key
  abstains at `h` (R2, R6) counts as faulty at `h`.

---

## 2. Topology per (height, view)

The heart of Sumeragi (after B-Chain): each round `(h, v)` overlays a total order on the committee
and assigns roles by position.

### 2.1 Definitions

Let `C = C_h` (canonical order, length `n`, with `f`, `q` of `C_h`).

1. **Committee permutation.**
   ```text
   committee_digest(C) = H(TAG_COMMITTEE ‖ be32(n) ‖ kb(C[0]) ‖ … ‖ kb(C[n−1]))
   seed_C              = H(TAG_TOPOLOGY ‖ I ‖ committee_digest(C))
   perm_C              = prf_shuffle(seed_C, n)

   prf_shuffle(seed, n):
       slots = [0, 1, …, n−1]; out = []; ctr = 0
       while slots not empty:
           r   = be64_to_u64(H(seed ‖ be64(ctr))[0..8])
           pos = r mod len(slots)
           out.push(slots.swap_remove(pos))   // remove slots[pos], move the last element into pos
           ctr += 1
       return out                             // out[i] = canonical index at permutation position i
   ```
   The permutation depends only on the committee (application state), never on a block that a
   proposer can shape, so nobody can grind it.
2. **Demotion set `D_h` (skipped leaders only).** Window `J_h = [max(g+1, h−1−W), h−2]` of
   committed headers; `W = demotion_window`, a per-instance constant fixed at genesis
   (`Init.demotion_window`; it is not a chain parameter and never changes, so every node uses the
   same window). The core holds every header of `J_h` when it enters `h`: entry requires
   `applied ≥ h − 2`, `BlockApplied` events arrive in height order and carry the applied header
   (§6.13), and `Init` carries the last `W + 2` headers. A member `m ∈ C_h` is a *candidate*
   if `m ∈ skipped_leaders_j` (§3.2: the leaders of views of height `j` that ended without a
   commit) for some `j ∈ J_h`; `last(m)` = the highest such `j`. `D_h` = the candidates sorted by
   (`last` descending, canonical index ascending), truncated to the first `f_h`. Only headers up
   to `h − 2` are used because a node enters `h` before it necessarily holds block `h − 1` (§6.8).
   Nothing else demotes a member: silent or slow set-A members and proxy tails are handled by the
   stage ladder alone (§5.2).
3. **Non-demoted list.** `nd_h` = the members of `C_h \ D_h` in `perm_C` order;
   `a_h = n − |D_h| ≥ q`. `pos(m)` = the permutation position of `m` (`perm_C[pos(m)] = m`).
4. **Order of round `(h, v)` (slot substitution).**
   ```text
   k0(h)         = the index in nd_h of the first non-demoted member at permutation positions
                   h mod n, (h+1) mod n, …        // a demoted member's slot passes to its successor
   L(h, v)       = nd_h[(k0(h) + v) mod a_h]      // views rotate over non-demoted members
   base_{h,v}[i] = perm_C[(pos(L(h, v)) + i) mod n]        for 0 ≤ i < n
   order_{h,v}   = base_{h,v} stably partitioned: members ∉ D_h first, then members of D_h,
                   each group in base order
   ```
   With `D_h = ∅` this is exactly `order_{h,v}[i] = perm_C[(h + v + i) mod n]`. With demoted
   members, the view-0 slot of height `h` is still the permutation slot `h mod n`; only a slot
   whose owner is demoted passes to the next non-demoted member. (Rotating views by permutation
   slots as well, `base_{h,v}[i] = perm_C[(h+v+i) mod n]`, would give the member after a demoted
   slot two consecutive views and break the `f + 1` distinct-leaders property below.)
5. **Roles in `(h, v)`.**
   - leader `L(h,v) = order[0]`;
   - **set A** = `order[0 .. q−1]` (first `q = n − f` positions);
   - **proxy tail** `P(h,v) = order[q − 1]` (last member of set A; equals the leader iff `q = 1`);
   - **set B** = `order[q .. n−1]` (remaining `f` positions; empty if f = 0).

Properties used by §8:
- Demoted members never hold the leader, proxy-tail or any set-A position (`a_h ≥ q`).
- Fairness: with `C_h` and `D_h` fixed, over any `n` consecutive heights every permutation slot
  anchors view 0 exactly once. A non-demoted member `m` is the view-0 leader at its own slot and at
  the slots of the demoted members immediately preceding it in cyclic permutation order, i.e.
  `1 + (that number)` times, and never less than once. The view-0 proxy tail
  (`nd_h[(k0(h) + q − 1) mod a_h]`) likewise visits every non-demoted member at least once.
- Locality: demoting or reinstating `m` changes the view-0 leader only at the heights whose slot
  is (or passes to) `m`: its own slot and those of the demoted members immediately preceding it.
  Within one round it moves `m` into or out of the demoted tail; members before `m` in that
  round's base keep their positions, and members after it move by one position. No height is
  re-indexed (unlike a rotation modulo `a_h`, whose modulus changes with `|D_h|`).
- Within any `f + 1` consecutive views of one height the leaders are `f + 1` consecutive entries
  of `nd_h` (cyclically), hence `f + 1` distinct members because `a_h ≥ q ≥ f + 1`; at least one is
  correct when at most `f` members are faulty.
- The failed leader of view `v` sits at position `a_h − 1` in view `v + 1`: in set B when
  `a_h > q`; when `|D_h| = f` (so `a_h = q`) it is that view's proxy tail, whose failure the stage
  ladder (§5.2) absorbs within about `2·t_retx`.
- Set-A entry: with `D_h` fixed, `k0` advances by one at every height except those anchored at a
  demoted slot, and a non-demoted member's view-0 position `(index − k0) mod a_h` then decreases by
  one. When `a_h > q` it therefore enters set A at position `q − 1` (as proxy tail), passes
  through set A to the leader position 0, then moves to the end of the non-demoted list: it
  enters set A exactly once per `n` consecutive heights. When `a_h = q` every non-demoted member
  is in set A at every height.

### 2.2 Why this shape (B-Chain justification)

B-Chain (Duan, Meling, Peisert, Zhang) runs a chain of `2f+1` active replicas (head … proxy tail)
with `f` standby replicas and re-chains suspected replicas into the standby set. Sumeragi keeps:

- (a) only `q` members (set A) must act in the normal case — `q` is exactly the minimum that can
  form a certificate;
- (b) a designated aggregator (proxy tail) makes the normal case `O(n)` messages, with signature
  verification concentrated at one node;
- (c) a standby set B of size `f` that joins only when set A cannot finish (§5.2);
- (d) re-chaining: a leader whose view failed moves to the demoted tail (set B) for `W` heights and
  its slots pass to its successor. It is computed from committed headers (`skipped_leaders`), so
  every honest node derives the same order without accusation messages. Unlike B-Chain, other
  suspected members are not re-chained: a failed leader costs a whole view timeout, but a silent
  set-A member costs only set B's votes via `P` (stage 1, started at once by the hint of §5.2) and a
  silent proxy tail about `2·t_retx` at each of its proxy-tail turns (about once per `n` heights). Re-chaining those would need absence
  records in headers that a Byzantine proxy tail can bias by omitting honest votes; revision 2
  had them and revision 3 removed them as not worth their complexity (Appendix B).

Changed: B-Chain relays along the chain (`O(n)` hops). Sumeragi uses a star (leader → all,
set A → proxy tail → all). The normal case takes **5 one-way hops** until every member commits
(proposal; votes → P; PrepareQC → all; Commit votes → P; CommitQC → all). P itself commits after 4.
All-to-all voting (stage 2, §5.2) takes 3 hops. The star trades about `2Δ` per height for
`3(n−1) + 2(q−1)` messages instead of `(n−1) + 2n(n−1)`, and for `2(q−1)` signature verifications
at P instead of `2(n−1)` at every node. Under load (`pace = 0`) the next proposal carries the
CommitQC (§3.3), so most members learn finality from the next proposal anyway.

Topology affects only liveness and routing, never safety: every safety rule below is independent of
positions. The schedule is predictable as soon as the committee and recent headers are known (§15).

---

## 3. Data types, signing preimages, certificates

Wire and persisted encodings use Norito derives. **Signing preimages and hash preimages are fixed
byte layouts defined here** (not Norito), so they are exact and codec-independent.

### 3.1 Constants and encodings

```text
TAG_SIG       = ASCII "sumeragi/sig"          TAG_BLOCK    = ASCII "sumeragi/block"
TAG_PAY       = ASCII "sumeragi/payload"      TAG_ATT      = ASCII "sumeragi/attach"
TAG_QC        = ASCII "sumeragi/qc"           TAG_TC       = ASCII "sumeragi/tc"
TAG_COMMITTEE = ASCII "sumeragi/committee"    TAG_TOPOLOGY = ASCII "sumeragi/topology"
KIND_PROPOSAL = 0x01, KIND_PREPARE = 0x02, KIND_COMMIT = 0x03, KIND_TIMEOUT = 0x04, KIND_ECHO = 0x05
Hash32 = [u8; 32]; ValidatorIndex = u32 (canonical index into C_h)

kb(pk)    = be16(len(raw)) ‖ raw    // raw: 48-byte compressed G1 point (production); 32 bytes (simulator)
keys(l)   = be32(len(l)) ‖ kb(l[0]) ‖ … ‖ kb(l[len(l)−1])
enc(None) = 0x00 ; enc(Some(w)) = 0x01 ‖ be64(w)     // optional view
opt(None) = 0x00 ; opt(Some(x)) = 0x01 ‖ x           // optional digest
sig bytes = canonical compressed encoding (96 bytes for BLS)
```

### 3.2 Block

```rust
struct BlockHeader {
    instance: Hash32,                 // I
    height: u64,                      // h
    origin_view: u64,                 // view in which this block was first (freshly) proposed
    parent_hash: Hash32,              // block_hash of the committed block at h−1
    parent_result: Hash32,            // R certified by the CommitQC of h−1
    payload_hash: Hash32,             // H(TAG_PAY ‖ payload)
    payload_len: u32,                 // len(payload) ≤ max_block_bytes(h); 0 if origin_view ≥ empty_after_views(h)
    proposer: ValidatorIndex,         // == idx(L(h, origin_view))
    skipped_leaders: Vec<PublicKey>,  // [L(h, x) for x in 0..min(origin_view, a_h)]
}
struct Block { header: BlockHeader, payload: Vec<u8> }   // payload opaque to the core
```

```text
block_hash = H(TAG_BLOCK ‖ I ‖ be64(h) ‖ be64(origin_view) ‖ parent_hash ‖ parent_result ‖
               payload_hash ‖ be32(payload_len) ‖ be32(proposer) ‖ keys(skipped_leaders))
body_ok(b) := len(b.payload) == b.header.payload_len ∧ H(TAG_PAY ‖ b.payload) == b.header.payload_hash
```

Keys (not indices) are used in headers so that demotion (§2.1) is computable from headers alone,
even across committee changes. At `h = g + 1`: `parent_hash`/`parent_result` are the genesis block
hash and result.

**Body intake rule.** Every block body the core takes from anywhere — a proposal's payload, a
`BlockResponse`, a `SyncResponse` entry, a `BodyAvailable` event — MUST satisfy `body_ok` before
it is stored, executed, committed, served or put into evidence. A failing body is discarded with
no state change, no execution verdict and no evidence; any fetch for it stays outstanding. A body
already held for a block hash is never replaced. Votes and certificates bind `block_hash`, which
binds `payload_hash` and `payload_len`, so a certified block hash determines the payload bytes; `R`
need not commit to them again.

The block's *own* result `R` is **not** in its header; it is bound by the votes and certificates
of that block (§4). The next block binds it again via `parent_result`. The zero-length payload
`EMPTY = []` is the canonical empty block and MUST be accepted by every executor.

### 3.3 Signed messages and preimages

```text
prop_preimage(h, v, bh, ad)      = TAG_SIG ‖ 0x01 ‖ I ‖ be64(h) ‖ be64(v) ‖ bh ‖ ad
vote_preimage(kind, h, v, bh, R) = TAG_SIG ‖ kind ‖ I ‖ be64(h) ‖ be64(v) ‖ bh ‖ R        (kind ∈ {0x02, 0x03})
tmo_preimage(h, v, hq)           = TAG_SIG ‖ 0x04 ‖ I ‖ be64(h) ‖ be64(v) ‖ enc(hq)       (hq = view of the carried PrepareQC)
echo_preimage(nonce, height)     = TAG_SIG ‖ 0x05 ‖ I ‖ be64(nonce) ‖ be64(height)        (probe echo, §3.5, §7.4 R2)

ad = att_digest(justify, parent_qc)
   = H(TAG_ATT ‖ opt(justify.map(tc_digest)) ‖ opt(parent_qc.map(qc_digest)))
qc_digest(c) = H(TAG_QC ‖ c.kind ‖ I ‖ be64(c.height) ‖ be64(c.view) ‖ c.block_hash ‖ c.result ‖
                 be32(len(c.signers)) ‖ c.signers ‖ c.agg_sig)
tc_digest(t) = H(TAG_TC ‖ I ‖ be64(t.height) ‖ be64(t.view) ‖ be32(len(t.entries)) ‖
                 [be32(idx) ‖ enc(hq)] for each entry ‖ t.agg_sig ‖ opt(t.high_pqc.map(qc_digest)))
```

Domain separation: tag, kind, instance, height and view are in every consensus preimage; block
hash and result in every vote. There is no other signed consensus object. The only other signed
object is the probe echo (kind `0x05`): it binds the prober's nonce and the replier's reported
height, carries no sign-once obligation (a node signs one per probe it answers), is never
recorded, and cannot verify as a vote, proposal or timeout because of its kind byte.

The leader's signature covers **everything in a proposal except the payload bytes**, which are
bound through `payload_hash`/`payload_len` in the signed header. A relay can therefore only drop a
proposal, or strip or corrupt its payload. The latter is caught by `body_ok`, cannot be attributed
to anyone, and never causes evidence or an early timeout (§6.2).

```rust
struct Proposal {
    instance: Hash32, height: u64, view: u64,  // round of this proposal (≥ header.origin_view)
    header: BlockHeader,
    justify: Option<TimeoutCert>,  // Some iff view > 0: a TC for (height, view − 1)
    parent_qc: Option<Qc>,         // a CommitQC of height − 1; None iff height == g + 1
    payload: Option<Vec<u8>>,      // unsigned; the leader always includes it; relays may strip it
    sig: Signature,                // by L(height, view) over prop_preimage(height, view, bh, ad)
}
struct Vote {
    kind: VoteKind,                // Prepare | Commit
    instance: Hash32, height: u64, view: u64,
    block_hash: Hash32, result: Hash32,
    signer: ValidatorIndex, sig: Signature,   // over vote_preimage
}
struct TimeoutVote {
    instance: Hash32, height: u64, view: u64,
    high_pqc: Option<Qc>,          // the signer's lock at signing time; view ≤ self.view
    signer: ValidatorIndex, sig: Signature,   // over tmo_preimage(height, view, high_pqc.map(|q| q.view))
}
```

### 3.4 Certificates

```rust
struct Qc {                        // PrepareQC if kind == Prepare, CommitQC if kind == Commit
    kind: VoteKind, instance: Hash32, height: u64, view: u64,
    block_hash: Hash32, result: Hash32,
    signers: Bitmap,               // exactly ceil(n_h/8) bytes, bit i = canonical index i, spare bits 0
    agg_sig: AggregateSignature,
}
struct TimeoutCert {
    instance: Hash32, height: u64, view: u64,
    entries: Vec<(ValidatorIndex, Option<u64>)>, // strictly increasing index; hq view per signer
    agg_sig: AggregateSignature,                 // aggregate of the individual timeout signatures
    high_pqc: Option<Qc>,                        // the PrepareQC with view == max(entries.hq)
}
```

**verify_qc(qc, C_h)**: `qc.instance == I`; bitmap length exact and spare bits zero;
`popcount ≥ q_h`; `AggVerify({pk_i : bit i set}, vote_preimage(kind, h, v, bh, R), agg_sig)`.
Certificates with more than `q_h` signers are valid. The committee used is always
`C_{qc.height}`; a certificate for a height whose committee is not known is buffered (bounded,
§8.4) or dropped, never verified against another committee.

**verify_tc(tc, C_h)**: `tc.instance == I`; `len(entries) ≥ q_h`; indices strictly increasing and
`< n_h`; every `hq ≤ tc.view`; group entries by `hq` value and check
`AggVerifyMulti([(agg_pk(group_g), tmo_preimage(h, tc.view, hq_g))]_g, agg_sig)` (cost: `#groups+1`
pairings); let `m = max(hq)` over entries (None if all None): `high_pqc` MUST be `None` iff
`m = None`, and otherwise `high_pqc.kind == Prepare`, `high_pqc.height == h`,
`high_pqc.view == m`, `verify_qc(high_pqc, C_h)`.

*Uniqueness lemma used throughout:* two valid PrepareQCs with the same `(I, h, v)` certify the same
`(block_hash, R)` (they share ≥ `f+1` signers, one honest, and honest nodes sign one Prepare per
`(I,h,v)`, §7.1). Hence "the PrepareQC of view `m`" is unambiguous and a TC need not sign the hash.

**Formation.** An aggregator (proxy tail, or any member at stage 2) keeps at most one vote per
`(kind, view, signer)` and forms a certificate as soon as it holds `q_h` valid votes with identical
`(kind, h, v, block_hash, R)`, using exactly those `q_h`. A TC is formed from any `q_h` valid
timeout votes for `(h, v)` (honest aggregators pick the `q_h` with the highest `hq`, §6.7); its
`high_pqc` is the PrepareQC carried by the entry with the maximal `hq`. The aggregator's own entry
is its stored signed message (§6.6), never a re-signed one. Verification cost is bounded by the
cheap-reject rules and the verified-certificate cache (§6.1 rule 5).

### 3.5 Unsigned messages

```rust
enum WireMessage {
    // round messages (height-filtered, §6.1)
    Proposal(Proposal), Vote(Vote), Qc(Qc), Timeout(TimeoutVote), Tc(TimeoutCert),
    // service messages (never height-filtered, §6.1)
    Status(Status),
    SyncRequest  { instance: Hash32, from_height: u64, max_count: u16, max_bytes: u32 },
    SyncResponse { instance: Hash32, blocks: Vec<(Block, Qc /*CommitQC*/)> },
                 // consecutive heights starting at from_height, total ≤ max_bytes unless 1 entry;
                 // empty = "I hold nothing at from_height"
    BlockRequest  { instance: Hash32, height: u64, block_hash: Hash32 },
    BlockResponse { instance: Hash32, block: Block },
}
struct Status {                // periodic "state, not custody" summary (§6.11)
    instance: Hash32,
    height: u64, view: u64,                   // sender's current round (while awaiting: tip.height + 1, view 0)
    committed_qc: Option<Qc>,                 // CommitQC of height − 1 (None at the first height)
    high_pqc: Option<Qc>,                     // sender's lock at `height` (None while awaiting)
    high_tc: Option<TimeoutCert>,             // highest TC the sender holds at `height` (None while awaiting)
    proposal_hash: Option<Hash32>,            // block hash of the (height, view) proposal it holds
    want_proposal: bool,                      // holds no proposal of (height, view) and asks for it (§6.11)
    probe: Option<u64>,                       // nonce: "answer me with your Status" (§7.4 R2); set in
                                              //   every Status while one of the sender's keys is unanchored
    echo: Option<Echo>,                       // signed answer to a probe (§7.4 R2)
}
struct Echo {
    nonce: u64,                               // the probe nonce this Status answers
    key: PublicKey,                           // replier's signing key at its height, else its first configured key
    sig: Signature,                           // by `key` over echo_preimage(nonce, Status.height) (§3.3)
}
```
While `awaiting` (§6.8) a node's `Status` has `height = tip.height + 1`, `view = 0`,
`committed_qc = tip.commit_qc`, no `high_pqc`, `high_tc` or `proposal_hash`, and
`want_proposal = false`.

### 3.6 Evidence

```rust
enum Evidence {
    ProposalEquivocation(Proposal, Proposal),   // same (I,h,v), both signed by L(h,v), different (bh, ad)
    VoteEquivocation(Vote, Vote),               // same (kind,I,h,v,signer), different (bh, R)
    TimeoutEquivocation(TimeoutVote, TimeoutVote), // same (I,h,v,signer), different hq
    InvalidProposal { proposal: Proposal, defect: Defect }, // signed-content defect, §6.2 step 7
    ConflictingCertificates(Qc, Qc),            // CommitQCs of the same committed height, different value (§7.6)
}
```

Equivocation evidence is self-verifying. Two certificates of the same `(I, h, v, kind)` with
different values attribute their signer intersection (each signed two values in one view). For
conflicting CommitQCs of one height at different views the core attributes nobody: honest members
can legitimately sign both once more than `f` members are faulty, and forensics (who signed Commit
at `v` and later a timeout at `≥ v` with `hq < v`, who voted for a proposal violating the TC rule)
are the application's. `InvalidProposal` is verifiable from its content by re-checking the signed
fields against the committed chain. **Only signed-content defects produce evidence.** A block that
executes to `Invalid` produces none: the leader does not execute before proposing (§4.2), so an
honest leader can propose a poison transaction without knowing it, and a single divergent executor
would otherwise accuse honest leaders. Penalties are the application's.

Typical sizes (BLS): Vote ≈ 215 B; Qc ≈ 215 B; TimeoutVote ≤ 440 B; TC ≈ 330 B + 13 B × signers;
Proposal without payload ≤ 2 KB at n = 31; Status ≤ 1.4 KB at n = 22 (an `Echo` adds ≈ 160 B).

---

## 4. Execute-before-vote

### 4.1 What is executed and what is bound

- A committee member executes a proposed block `B` at height `h` against `S_{h−1}`: the applied
  state (after `BlockApplied(h−1)`, §12) or, earlier, the executor's cached post-state of the
  committed parent block. The core allows the latter only when this node executed that parent and
  obtained exactly the certified result (`tip.exec_ok`, §6.8), so durable apply of the parent is
  not on the vote path. If the driver no longer holds that post-state (evicted, lost on restart)
  it recomputes it; a missing cache entry is never a verdict (§12.3 O4). Execution is requested by
  `Action::Execute { block, req }` and answered exactly once by
  `Event::Executed { block_hash, req, outcome }`,
  `outcome ∈ {Valid(R), Invalid, Failed, Cancelled}`; `req` is a per-core request counter, so an
  answer to a request the core has since discarded never matches a live entry (§6.3).
- `R` is the application's 32-byte execution commitment. It MUST commit to everything the chain
  must agree on for that block: at least the post-state root, the root of per-transaction outcomes
  (accepted/rejected + reason), the event root, and the height configuration scheduled by this
  block (`C_{h+2}` and chain parameters, §10). The core treats `R` as opaque.
- The Prepare vote, the PrepareQC, the Commit vote and the CommitQC all sign
  `(I, h, v, block_hash, R)`. A CommitQC therefore finalizes order **and** result. The next
  block's header repeats the certified `R` as `parent_result`.

### 4.2 Verdicts

- The leader makes **no** result claim in the proposal and does not execute before proposing (it
  executes after sending, in parallel with everybody else; its Prepare vote carries its own `R`).
  Execution is therefore on the critical path once, not twice.
- Every voter binds the `R` it computed. Votes with different `R` are different values and never
  aggregate together. With deterministic execution all honest voters compute the same `R`; a
  certificate exists only for an `R` computed by ≥ `q − f ≥ f + 1` honest members.
- **Certified blocks.** A block is *certified at `h`* for a node if the node holds a valid
  PrepareQC `Q` at `h` for it (every re-proposal is: its TC names `Q`). Its expected result is
  `Q.result`. If the local outcome is `Invalid`, or `Valid(R)` with `R ≠ Q.result`, the local
  executor disagrees with ≥ `f + 1` honest executions: `Action::LocalFault(ExecutionMismatch)`, no
  Prepare vote in this view, **no evidence and no early timeout**; the node keeps participating
  otherwise (timeouts, Commit votes, aggregation). If a CommitQC for `Q.result` later reaches
  apply, the apply path detects the divergence and halts that instance (§12.5).
- **Deterministic invalidity** of a proposal is either (a) a defect in its signed content
  (§6.2 step 7), or (b) `Executed = Invalid` for a `body_ok`, uncertified, fresh block. Then: no
  Prepare vote and an immediate timeout of the view (§6.6) if it is the current view; in case (a)
  `ReportEvidence(InvalidProposal)`; in case (b) no evidence (§3.6) and `PayloadRejected{height,
  view, block_hash}` to the driver. The payload builder MUST NOT quarantine a transaction merely
  because it was in a rejected block: it re-checks that block's transactions one by one against its
  current state and quarantines only a transaction that makes a block `Invalid` on its own (a
  Byzantine leader can wrap honest transactions in an invalid block). All honest nodes reach the
  same verdict, so the view is abandoned by everyone. Transactions that fail inside a valid block
  are *not* block invalidity; they are recorded as rejected inside `R`.
- **Local failure.** Executor panics, I/O errors and resource exhaustion MUST be reported as
  `Failed`, never `Invalid`. `Failed` → `LocalFault(ExecutorFailed)` and a retry of `Execute`
  after a backoff (100 ms, doubling, capped at `rebroadcast_interval`) while the block is still
  needed; no vote meanwhile, no evidence, no early timeout.
- **Cancelled work.** Every `DiscardExecution` removes the core's execution entries (pending or
  finished) of the blocks it does not keep, and the driver drops their post-states and answers
  their outstanding requests `Cancelled`. A block that comes back later (e.g. re-proposed after a
  PrepareQC this node had not seen) is executed again from scratch; a stale answer is ignored.
- Missing or corrupt payload bytes (`body_ok` fails) are never invalidity: the body is fetched
  (§6.9).

### 4.3 The complete Prepare-vote predicate (no local refusal reasons)

An honest committee member of `C_h` signs `Prepare(h, v, bh, R)` iff **all** hold:
1. it is in round `(h, v)`, has a signing key at `h` (§7.4), and `timeout_view < v` (it has not
   timed out view `v` or later);
2. it has not signed a Prepare at `(h, v)` (sign-once record);
3. it holds a Proposal for `(h, v)` that passed §6.2 (signature, leader, attachments, header, TC
   rule), whose body it holds (`body_ok`) and has handed to `StoreBody`;
4. `Executed(bh) = Valid(R)`, and if `bh` is certified, `R == Q.result`;
5. it is in set A of `(h, v)`, or its stage for `(h, v)` is ≥ 1 (§5.2).

Nothing else may gate a vote: not load, queue state, mempool contents, recovery state, peer
connectivity, disk pressure, clock readings, other instances, or a local lock (there is no
voter-side lock check; §7.3 explains why the TC rule makes it unnecessary for safety and why it
would harm liveness). Slowness only delays conditions 3–4; the vote is still sent when they become
true, as long as condition 1 holds.

### 4.4 Why this does not reintroduce the v2 common-mode stall

The v2 root cause was that votes depended on each node's local conditions, so one bug or one
slow resource stopped every honest node at once. Here:
1. **Value dependence is deterministic.** The only data-dependent input is `exec`, a pure function
   of committed data. A deterministic "Invalid" verdict is shared by all honest nodes and costs
   one view (early timeout, next leader; the leader of that view is recorded as skipped), and
   `PayloadRejected` lets builders drop the culprit transactions. An executor defect that rejects *every* non-empty payload is escaped because
   the `EMPTY` payload must always execute and fresh blocks from view `empty_after_views` on must
   be `EMPTY` (§6.2, §6.10).
2. **Timing dependence is absorbed by the pacemaker.** Execution latency only delays votes. View
   timeouts grow ×1.5 per failed view up to `T_max ≥ T_req` (§8.2 L3; the same formula is the
   config validation rule, §9.4), and the start level adapts across heights (§9), so the view
   eventually outlasts execution. Timers are separate events (`Tick`) that the driver delivers
   ahead of every other event (§12.3 O5); the core never blocks.
3. **Consensus does not wait on local apply/storage.** Entering `h+1`, timing out, Commit-voting
   and aggregating need neither execution nor apply; a Prepare vote needs the block executed, which
   needs the parent's post-state, available from the speculative cache before apply. Consensus runs
   at most two heights ahead of durable apply (§10.2).
4. **Execution never waits on another instance or on external data.** Everything execution reads
   besides `S_{h−1}` (AMX proofs, remote commit proofs, time) is carried in the payload.
5. **Local faults stay local.** A node with a broken, failing or slow executor is one faulty node;
   up to `f` such nodes (in any positions) are masked because set B joins (§5.2). Such a node never
   produces evidence (execution verdicts never do, §3.6); certified-block mismatches and `Failed`
   are local faults only; an `Invalid` it computes for an uncertified block costs one early
   timeout and a local `PayloadRejected`, i.e. the timeout of one faulty node, which the `q` (TC)
   and `f + 1` (join) thresholds already tolerate.

---

## 5. Message flow

### 5.1 Normal case (stage 0, all roles responsive), round (h, v)

| Step | Who | Sends | To |
|---|---|---|---|
| 1 | leader `L` | `Proposal` (StoreBody + persist proposal record first) | members of `C_h` (set A first), plus `C_{h+1} \ C_h` if known |
| 2 | every member | validates (§6.2), stores the body, executes once the parent state is available | — |
| 3 | set A member | `Vote(Prepare, R)` (persist first) | proxy tail `P` (local if self) |
| 4 | `P` | on `q` matching Prepares: `Qc(PrepareQC)` | all other members |
| 5 | every member | on PrepareQC: update lock; set A member sends `Vote(Commit)` (persist first) | `P` |
| 6 | `P` | on `q` matching Commits: `Qc(CommitQC)` (§6.8 step 1a), then commits | all other members (+ `C_{h+1} \ C_h`) |
| 7 | every member | on CommitQC: commit, enter `(h+1, 0)` | — |

Set B members receive the proposal and PrepareQC, execute (so they are ready for stage 1 and for
apply), update their lock, but do not vote at stage 0. Latency: 5 one-way hops until non-P members
commit (4 at `P`), one execution, two record persists (the body write overlaps execution), and the
leader's pacing. Every vote sent to `P` is retransmitted to `P` at `t_retx`, doubling, until that
phase's QC is known (§6.11). A lost QC costs one retransmit period: `P`, when it already holds the
PrepareQC a received Prepare belongs to, answers the voter with it (once per voter per view,
§6.4); a Commit vote retransmitted after `P` committed arrives at a lower height and is answered
with `P`'s `Status`, whose `committed_qc` is that CommitQC (§6.1 rule 3).

### 5.2 Graded fallback (the stage ladder)

Each node has, per round `(h, v)`, a `stage ∈ {0, 1, 2}` that only increases within the round and
is set to the round's *initial stage* on every round entry (height entry and view change).
`t_ready` = the time at which §4.3 conditions 1–4 first held at this node (it could Prepare).
`t_lastvote` = the time of this node's latest vote in the round (for a set-B member that has not
voted: `t_ready + t_retx`). `t_retx` (§9.1) is about three times the locally observed vote-to-QC
latency. The ladder is local routing only; it has no safety impact. Silent or slow set-A members
and proxy tails are handled by this ladder alone; nothing demotes them (§2.1).

- **Initial stage (hint).** 1 if the parent CommitQC `c = tip.commit_qc` has a signer outside
  `setA(h−1, c.view)` (the previous height needed set B, so a set-A member is probably silent and
  set B votes at once), else 0. It is a pure function of the parent CommitQC this node holds
  (normally the one `P` broadcast, so members agree; a disagreement only changes routing): no
  timer, no stored state. Its cost when wrong is `2f` extra vote messages per phase and
  their verification at `P`, never latency. It can stay on in all-honest runs: once set B votes,
  `P` forms each certificate from the first `q` matching votes, and whenever a set-B vote beats a
  set-A vote the next parent CommitQC again has a set-B signer. That is accepted: it happens only
  when a set-B member was faster than a set-A member, so set B's votes shorten the round. A
  Byzantine proxy tail can keep it on only by certifying set-B votes, i.e. while set B is voting
  anyway.
- **Stage 0 (normal).** Set A votes to `P`; set B does not vote.
- **Stage 1 (set B joins through `P`).** Entered at round entry by the hint, or when (a) the QC
  of the current phase is overdue: `now ≥ t_ready + t_retx` and no PrepareQC for `(h, v)` is held,
  or `now ≥ t_pqc + t_retx` and no CommitQC for `h` is held (`t_pqc` = when this node first held
  the PrepareQC of `(h, v)`; this catches a member that Prepares but withholds its Commit), or (b)
  a PrepareQC for `(h, v)` is held whose signers include a set-B member of `(h, v)` (a set-A vote
  was missing in the Prepare phase, so it will be missing in the Commit phase too). Set B votes,
  still only to `P`.
- **Stage 2 (broadcast; every member aggregates).** Entered when no CommitQC for `h` is held and
  (a) `now ≥ t_lastvote + 2·t_retx` without the QC of the phase of that vote, or
  (b) `now ≥ anchor + φ·T(level)` (backstop, also for nodes that never became ready, §9.1), or
  (c) *contagion*: the node is not `P` and holds verified votes of `(h, v)` (either kind) from
  ≥ `f + 1` distinct signers other than itself. Such votes show that at least one honest member
  has voted; they may have been relayed (votes are relayable signed objects), so a Byzantine `P`
  that forwards the honest votes it receives can move every member to stage 2 at its proxy-tail
  turns — a cost it can cause anyway by withholding its QCs (stage 2(a)). `f` Byzantine signers
  cannot trigger contagion alone. Only verified votes exist in pools (§6.4), so no trigger of this
  ladder ever counts an unverified vote.
- **On entering stage 1 or 2** the node re-sends once, with the new routing, each own vote of the
  round whose phase QC it does not hold, and restarts that vote's retransmit schedule from this
  send (§6.11); it then calls `try_prepare()` and `try_commit()` (§6.3, §6.5). There is no other
  path to a vote: §4.3 and S3 (§7.1) stay the only predicates.
- **Routing.** `route(vote)` = `Send(P)` at stages 0–1 (nothing if `P == me`), `Broadcast` to
  `C_h \ {me}` at stage 2. The node's own vote enters its own pool through `pool_insert` exactly
  once, when it is signed (§6.4), never through `route`. Every member aggregates whatever votes it
  receives (§6.4); a member that forms a PrepareQC uses it locally, and a member that forms a
  CommitQC commits. Only `P` broadcasts the QCs it forms (§6.5 2d, §6.8 step 1a); other members
  rely on §5.1's QC answers, on `Status`, and on the next proposal's `parent_qc`. `P` accepts
  votes from anyone at any stage.

Failure coverage:
- silent or slow non-`P` set-A member → stage 1: at the first height where it is missed, set B
  supplies the missing vote to `P` about `t_retx` later (trigger a); at every following height
  whose parent CommitQC needed set B the hint starts stage 1 at once, so set B's votes reach `P`
  together with set A's and the member costs `2f` extra vote messages per phase but no latency. It
  is never demoted for this;
- silent, withholding or selective proxy tail → stage 2 about `2·t_retx` after the members'
  votes. A member is the view-0 proxy tail at least once and at most `1 + |D_h|` times per `n`
  heights (§2.1), so this cost recurs at that rate; it is not demoted for it;
- silent, invalid or equivocating leader → no quorum in the view → view timer (or at once, on a
  signed defect or proven equivocation, §6.6) → TC → next leader
  (a different non-demoted member); the failed leader is recorded in `skipped_leaders` and demoted
  for `W` heights. A crashed member is therefore demoted at its first turn as leader, after at most
  one proxy-tail turn (stage 2) and the set-A turns in between (stage 1; after the first, the hint
  removes the `t_retx` wait). Exception: a re-proposal keeps its original header, whose
  `skipped_leaders` lists only views before its `origin_view` (§3.2), so a leader that fails in a
  view after a block was locked (PrepareQC formed, no commit) is not recorded at that height and is
  demoted at a later turn.

### 5.3 Message complexity (point-to-point messages per height)

| Case | Formula | n = 4 (q=3, f=1) | n = 22 (q=15, f=7) |
|---|---|---|---|
| Stage 0 (normal) | `3(n−1) + 2(q−1)` | 13 | 91 |
| Stage 1, one silent non-P set-A member | `3(n−1) + 2(q−2+f)` | 13 | 103 |
| Stage 1 from round entry (hint), every member responsive (can persist, §5.2) | `5(n−1)` | 15 | 105 |
| Stage 2, silent proxy tail | `(n−1) + (q−1) + 2(n−1)²` | 23 | 917 |
| View change (per failed view) | `n(n−1)` timeouts `+ (n−1)` TC → next leader | 15 | 483 |
| Status while unsettled (per `rebroadcast_interval`, per unsettled node) | `n − 1` | 3 | 21 |
| Status while settled (per `status_keepalive`, all nodes) | `n(n−1)` | 12 | 462 |
| Late entry or lost proposal copy: `Status{want_proposal}` + one proposal re-push | 2 | 2 | 2 |

For comparison, all-to-all two-phase voting costs `(n−1) + 2n(n−1)` = 27 / 945 per height.
Retransmissions happen only on loss. Bytes are dominated by the proposal payload, sent `n−1` times
by the leader (set A first); every other message is ≤ 2 KB.

---
## 6. Handlers

### 6.0 Core state (per instance)

```rust
struct Core {
    local: LocalParams,                  // §9.3 / §12.4
    w: u64,                              // demotion window W (Init.demotion_window, genesis constant, §2.1)
    keys: Vec<LocalKey>,                 // configured and retired keys: {pk, signer: Option<Signer> (None if retired),
                                         //   abstain_below: u64, unanchored: bool, restore: Option<SafetyRecord>} (§7.4)
    nonce: u64,                          // Init.nonce: probe nonce of this process lifetime (§7.4 R2)
    probe: BTreeMap<PublicKey, u64>,     // while a key is unanchored: lowest height reported per member key of
                                         //   C_{tip.height+2} in a fresh, verified echo (§6.11)
    // committed chain (consensus view of it)
    tip: Tip,                            // {height, block_hash, result, commit_qc: Option<Qc>, exec_ok: bool,
                                         //  exec_req: Option<u64>, prev: Option<(Hash32, Hash32)>}
                                         //  (§6.8 step 2, §6.3 step 0; prev = (bh, R) of tip.height − 1, §7.6)
    applied: u64,                        // highest height acknowledged by BlockApplied (durable)
    configs: BTreeMap<u64, HeightConfig>,// {committee, chain params} for tip.height−1 ..= applied+2
    recent_headers: VecDeque<BlockHeader>, // committed headers, the last W + 2 up to `applied`
    pending_apply: VecDeque<PendingApply>, // {height, commit_qc, parent_hash, parent_result}: committed,
                                         //   CommitBlock not yet emitted (≤ 2), strictly increasing height
    awaiting: bool,                      // committed `tip.height`, config of tip.height+1 not yet known
    // current round
    h: u64, view: u64,                   // h == tip.height + 1 (h == tip.height while awaiting)
    t_enter: Millis, t_prop: Option<Millis>, anchor: Millis, // §9.1
    t_body: Option<Millis>,              // when this node first held proposal and its body (§6.2 step 9, §9.2)
    late_entry: bool,                    // this height was entered late (§6.8 step 5); reset on view change
    asked: bool,                         // a Status{want_proposal} was sent to L(h, view) in this round (§6.11)
    signer: Option<usize>,               // index into `keys` of the key that signs at h; None = observer
    timeout_view: Option<u64>,           // highest view this node signed a timeout for at h
    proposal: Option<Proposal>,          // the accepted proposal of (h, view)
    build: Option<(u64, BuildPhase)>,    // outstanding BuildPayload request id (view-0 first request / heartbeat / view > 0)
    stage: u8, t_ready: Option<Millis>, t_pqc: Option<Millis>, t_lastvote: Option<Millis>, // §5.2
    mine: Mine,                          // exact signed messages of this round: proposal, prepare, commit, timeout,
                                         //   each with its t_vote (time of its latest send by route) for §6.11
    blocks: BTreeMap<Hash32, Block>,     // bodies in memory (≤ 6, §8.4); all are also in the driver's body store
    exec: BTreeMap<Hash32, ExecState>,   // blocks of height h: Pending{since, req} | Valid(R) | Invalid | RetryAt{t, attempt}
    next_req: u64,                       // request counter for Execute and BuildPayload (§6.3, §6.10); never reused
    wants: BTreeMap<Hash32, Want>,       // {height, reason, sources, attempt, next_retry}: sole owner of body fetches
    repropose: bool,                     // leader of (h, view) waiting for the body of TC.high_pqc's block
    votes: VotePools,                    // (kind, view ∈ {view−1, view, view+1}) → one vote per signer
    timeouts: Vec<Option<TimeoutVote>>,  // highest-view timeout per signer at h (len n_h)
    high_pqc: Option<Qc>,                // lock: highest-view PrepareQC seen at h
    high_tc: Option<TimeoutCert>,        // highest-view TC seen at h
    safety: SafetyRecord,                // last value handed to PersistSafety for the signing key (§7.4)
    pm: Pacemaker,                       // start_level, fast_streak, last_exec_ms (maximum over h), latency_ewma, qc_lat_ewma (§9)
    sync: SyncState,                     // §6.9 (target, target_verified, sources, buffer, outstanding request)
    peers: PeerTable,                    // bounded LRU (n + max_observers): last Status, rate-limit and re-push stamps
    cert_cache: CertCache,               // LRU of digests of certificates verified at h (capacity 4n)
    reported: BTreeSet<EvidenceKey>,     // evidence already emitted: (kind, view, signer) for pool views, (view) for proposals
    deadlines: Deadlines,                // propose/build, stage, retransmit, view, rebroadcast, status, probe, sync, fetch, exec retry
}
```

`handle(now, event) -> Vec<Action>` runs one handler to completion; `next_wakeup()` returns the
earliest pending deadline. Helpers used below: `C = C_h`; `n, f, q, a` of `C_h`;
`order/L/P/setA/setB` of `(h, view)` (§2); `route(msg)` (§5.2); `recipients` = members of `C_h`
except self, set A of `(h, view)` first, and for a `Proposal` or a CommitQC broadcast also
`C_{h+1} \ C_h` when `C_{h+1}` is known; `sign(..)` = sign with `keys[signer]` (signatures are
deterministic: the same key and preimage always give the same bytes, §12.1);
`persist()` = `safety.lock = high_pqc.clone(); safety.high_tc = high_tc.clone();` push
`PersistSafety(safety.clone())`. Every signed message is created *after* `persist()` in the same
action list, and O2 (§12.3) holds every externally visible action after a `PersistSafety` until
the record is durable. A node whose `signer` is `None` at `h` (not in `C_h`, or its key abstains
or is unanchored, §7.4) runs every rule except those that sign ("observer"); its `Status` to
`C_h` makes members reply with theirs, whose `committed_qc` drives its sync (§6.9).

**Own messages.** The node's own votes and timeouts never go through the network or `route` to
itself (O7). Each is inserted exactly once, right after it is signed, by the same insertion
routine that handles a verified received one: `pool_insert(vote)` (§6.4 step 4: insert,
contagion check counting signers other than `me`, QC formation) and `timeout_insert(t)` (§6.7:
store, TC formation, join). Formation therefore runs on own insertions too
(the node's own vote can be the `q`-th; with `n = 1` every certificate forms this way).

**Evidence dedup.** Before emitting `ReportEvidence`, the core checks `reported` (key: kind, view,
signer, or kind and view for proposals); each key is reported at most once. Keys of views outside
the vote-pool window are pruned with the pools; all are cleared on height entry.

**Names used by the mutations of §13.4.** Handlers: `on_proposal` (§6.2), `on_executed` (§6.3),
`on_vote` (§6.4), `on_qc` (§6.5), `on_timeout` and `on_tc` (§6.7), `commit_height` and
`enter_height` (§6.8), `on_sync_response`, `on_body` and `on_block_request` (§6.9), `on_tick`
(§6.11), `on_status` (§6.11), `on_block_applied` (§6.13), `restore` (the Init procedure, §7.4).
Helpers: `request_exec`, `discard_exec`, `try_prepare`, `pool_insert`, `try_commit`,
`sign_timeout`, `timeout_insert`, `form_tc`, `propose`, `advance_to`, `persist`, `route`,
`request_proposal` (§6.11), `initial_stage`, `quorum`, `verify_qc`, `verify_tc`, `body_ok`,
`demoted_set`, `leader`, `level`, `anchor`. An implementation may structure code differently but MUST keep one function per name so
that each mutation has exactly one site.

### 6.1 Intake filter (all messages)

1. Drop if `msg.instance ≠ I`.
2. **Service messages** (`Status`, `SyncRequest`, `SyncResponse`, `BlockRequest`, `BlockResponse`)
   are never height-filtered; they go to §6.9 / §6.11.
3. **Round messages** with `height < h`: a `Vote` or `Timeout` from a member of the current
   committee proves the sender is behind → `Send` it our `Status` (at most once per peer per
   `rebroadcast_interval`). A `Qc(Commit)` with `height ≤ tip.height` → safety monitor (§7.6).
   Everything else is dropped.
4. Round messages with `height == h + 1`: a `Proposal` → §6.2 step 0 (its `parent_qc` commits
   `h`); a `Qc(Commit)` → §6.9. A `Vote`, `Timeout`, `Qc(Prepare)` or `Tc` from a member of `C_h`
   → `Send` the sender our `Status` (rate-limited as above; its reply carries CommitQC(h)), then
   drop. Heights `> h + 1`: a `Qc(Commit)` or a proposal's `parent_qc` → §6.9 sync hint; else drop.
5. **Cheap rejects before any signature check:** an object byte-identical to one held; a vote
   whose `(kind, view, signer)` slot already holds the same `(bh, R)`; a PrepareQC with view
   `< high_pqc.view` (or equal: same value by the uniqueness lemma); a TC with view
   `≤ high_tc.view` that is not a proposal's `justify`; a CommitQC for a committed height equal in
   value to the committed one; a timeout equal in `(view, hq)` to the stored one of its signer.
   Every object held by the core (pool entries, timeouts, certificates, proposals) was verified or
   formed locally from verified parts, so a cheap reject only ever compares against verified
   entries. These rules apply to top-level objects; a certificate nested in another object (a
   timeout's `high_pqc`, a TC's `high_pqc`, a proposal's `justify`) is verified (or found in
   `cert_cache`) as part of its container and then handled by the container's rules (§6.5 2a
   compares views itself). A certificate whose digest is in `cert_cache` is valid without
   re-verification; every certificate verified at `h` enters the cache (LRU, capacity `4n`;
   cleared on height entry).
6. Signed objects are accepted from any peer (relays are fine); authenticity comes from the
   signature under `C_h[signer]`. Every signature is verified before its object enters any pool
   or changes any state; there is no deferred or optimistic verification.

### 6.2 On `Proposal p` (round `(h, w)`, `w = p.view`)

0. **Height.** If `p.height == h + 1`: if `h` is not yet committed and `p.parent_qc` is a
   CommitQC for `h`, run §6.8 on it (verification included); continue only if the node is now in
   round `h + 1` (not awaiting); otherwise drop (an awaiting node gets the proposal again through
   its late-entry `Status`, §6.11).
   If `p.height > h + 1`: pass `p.parent_qc` to §6.9 as a sync hint and drop.
1. **Authenticate.** `bh = block_hash(p.header)`, `ad = att_digest(p.justify, p.parent_qc)`.
   Verify `p.sig` under `C_h[L(h, w)]` over `prop_preimage(h, w, bh, ad)`. Failure → drop silently
   (a relay may have tampered; nothing is attributable).
2. **Duplicates and equivocation.** If a proposal for `(h, w)` is already held: same `(bh, ad)` →
   if the held one has no body and `p.payload` passes `body_ok`, take the body (step 8); stop.
   Different `(bh, ad)` → `ReportEvidence(ProposalEquivocation(held, p))` (deduplicated, §6.0);
   keep the held one; if `w == view`, `sign_timeout(view)` (early timeout, §6.6: the leader of
   the current view is proven faulty, whether or not this node already Prepared the held
   proposal); stop. At most one proposal per `(h, w)` is ever held, and only in the current view
   (`advance_to` drops it), so twins of a view the node already left are never compared.
   Both proposals passed step 1, so the evidence carries two different preimages of `(h, w)`
   signed by `L(h, w)`: an honest leader never signs two (S1), and bytes a relay can change (the
   payload) do not change `(bh, ad)` (§3.3), so an honest leader cannot be framed (§7.5).
3. **Justify.** `w == 0`: `p.justify` MUST be `None`. `w > 0`: `p.justify` MUST be a valid TC
   (§3.4) for `(h, w − 1)`; run the TC handler (§6.7) on it (it may advance `view` to `w`).
   A violation is a signed defect → step 7.
4. **View.** If `w < view`: if `bh` is wanted (§6.9) and `p.payload` passes `body_ok`, take the
   body; stop. If `w > view`: drop.
5. **Parent.** `p.parent_qc` is `None` iff `h == g + 1`; otherwise it MUST be a valid CommitQC
   (under `C_{h−1}`, cached) for `(tip.block_hash, tip.result)`. A violation is a signed defect →
   step 7.
6. **Header (signed content).** `header.instance == I`, `header.height == h`,
   `parent_hash == tip.block_hash`, `parent_result == tip.result`,
   `payload_len ≤ max_block_bytes(h)`, and:
   - **TC rule** — `w > 0` and `p.justify.high_pqc = Some(Q)`: `bh == Q.block_hash` (re-proposal;
     `expected_R = Q.result`). No other header check: `Q`'s honest signers checked this header
     against the same parent.
   - **Fresh block** — `w == 0`, or `p.justify.high_pqc = None`: `origin_view == w`;
     `proposer == idx(L(h, w))`; `skipped_leaders == [L(h, x) for x in 0..min(w, a_h)]`;
     `payload_len == 0` if `w ≥ empty_after_views(h)`.
7. **Signed defect.** A failure in step 3, 5 or 6 is attributable to `L(h, w)`:
   `ReportEvidence(InvalidProposal{p, defect})` (deduplicated, §6.0), and if `w == view`,
   `sign_timeout(view)` (early timeout, §6.6). Stop.
8. **Accept.** `proposal = p`; `t_prop = now` (recompute `anchor` and deadlines, §9.1). If
   `p.payload` passes `body_ok`: `blocks[bh] = block`; `StoreBody{block}` unless already stored.
   Otherwise (payload absent or corrupt — never a defect): create `want(bh, Proposal)` with sources
   `L(h, w)`, `Q`'s signers for a re-proposal, and members whose `Status` reported `bh`.
9. **Execute.** If the body is held and `t_body` is unset, `t_body = now` (the committing view's
   duration is measured from here, §9.2; this step runs again when the body arrives). If the
   body is held, the parent state is available (`applied ≥ h − 1`, or
   `tip.exec_ok`) and `exec[bh]` is absent: `request_exec(bh)`. If `exec[bh]` already holds an
   outcome, continue at §6.3 step 3. (Otherwise execution is requested when the body arrives,
   when `BlockApplied(h−1)` arrives, or when this node's own execution of the committed parent
   finishes with the certified result, §6.9, §6.13, §6.3 step 0; an entry removed by a
   `DiscardExecution` is absent, so a re-proposed block is executed again, §4.2.)

`request_exec(bh)`: `req = next_req; next_req += 1; exec[bh] = Pending{since: now, req}`; push
`Execute{block: blocks[bh], req}`.

`discard_exec(keep)`: for every `Pending{since}` entry whose block is not in `keep`,
`pm.last_exec_ms = max(pm.last_exec_ms, now − since)` (a lower bound of an execution that
outlasted its view, §9.2; its answer will be ignored); then remove every `exec` entry whose
block is not in `keep`; push `DiscardExecution{height: h, keep}`. It is the only way the core emits `DiscardExecution`
(§6.8 step 4, §6.12). It never touches the committed block's execution tracked in `tip.exec_req`
(that block is of height `tip.height`, and `DiscardExecution` is per height).

### 6.3 On `Executed { block_hash: bh, req, outcome }`

0. **Committed block.** If `bh == tip.block_hash` and `req == tip.exec_req` (the execution of the
   committed block was still pending when it committed, §6.8 step 2): `tip.exec_req = None`;
   `Valid(R)` with `R == tip.result` → `tip.exec_ok = true`, and if the current proposal's body is
   held with no `exec` entry, `request_exec` it (§6.2 step 9); `Valid(R ≠ tip.result)` or
   `Invalid` → `LocalFault(ExecutionMismatch)` (apply will report `ApplyDiverged`, §12.5);
   `Failed`/`Cancelled` → nothing (the driver executes the block when it applies it). Stop.
1. Ignore unless `exec[bh] == Pending{since, req}` with this `req` (a stale answer to a discarded
   or superseded request, including every `Cancelled` for one, is ignored). Record the outcome;
   `pm.last_exec_ms = max(pm.last_exec_ms, now − since)` (the longest execution at `h`, §9.2).
2. `Failed` or `Cancelled` → `exec[bh] = RetryAt{t: now + backoff, attempt}` (§4.2: 100 ms ·
   2^attempt, capped at `rebroadcast_interval`; `attempt` counts consecutive failures of this
   block at `h`), and for `Failed` `LocalFault(ExecutorFailed)`; stop. (The driver cancels only
   work that a `DiscardExecution` left out of `keep`, whose entries the core had already removed,
   so a matching `Cancelled` is a driver anomaly and is retried like a failure.) A due `RetryAt`
   calls `request_exec(bh)` if the block is still needed (§6.11).
3. If `bh` is the current proposal's block (otherwise stop), with `expected_R(bh)` = the result of
   a held PrepareQC at `h` for `bh` (a re-proposal's `Q`, or `high_pqc` if it names `bh`):
   - `Invalid` and `bh` uncertified → `PayloadRejected{h, view, bh}`,
     `sign_timeout(view)` (early timeout); stop. (No evidence, §3.6.)
   - `Invalid` and `bh` certified, or `Valid(R)` with `R ≠ expected_R(bh)` →
     `LocalFault(ExecutionMismatch)`; stop.
   - `Valid(R)`: if §4.3 conditions 1–4 hold and `t_ready` is unset: `t_ready = now`; schedule
     the stage timers (§5.2).
4. `try_prepare()`.

`try_prepare()`: if the §4.3 predicate holds for `proposal` (condition 3 includes
`proposal.view == view`; condition 2 is `safety.prepare` having no entry at `view`):
```text
safety.prepare = Some((view, bh, R)); persist()
vote = sign(Prepare, h, view, bh, R); mine.prepare = vote
route(vote); pool_insert(vote); t_lastvote = now
```
(The body's `StoreBody` was emitted at acceptance; the `PersistSafety` above is durable only after
it, §12.3 O2.)

### 6.4 On `Vote x` (from the wire; own votes enter at step 4 through `pool_insert`)

1. Require `x.height == h`, `x.view ∈ {view−1, view, view+1}`, `x.signer < n`; else drop.
2. If `x.kind == Prepare`, `me == P(h, x.view)` and this node holds a PrepareQC of `x.view`:
   `Send` it to `x.signer` (at most once per signer and view; no signature check is needed to
   send a certificate); the voter evidently lacks it. This step precedes the duplicate check,
   because retransmissions are exact duplicates. (A Commit vote for `h` cannot meet a CommitQC of
   `h` here: the node that holds one has committed `h`, and the vote is then handled by §6.1
   rule 3.)
3. Apply §6.1 rule 5 (an exact duplicate stops here); verify `x.sig` under `C[x.signer]`; failure
   → drop. If the pool `(x.kind, x.view)` already has a vote from `x.signer`: same `(bh, R)` → drop;
   different → `ReportEvidence(VoteEquivocation)` (deduplicated), drop.
4. `pool_insert(x)`: insert (only votes verified in step 3, or the node's own, ever enter a pool).
   If `x.view == view`, `me ≠ P` and the pools of view `view` now hold votes from ≥ `f+1` distinct
   signers other than `me`: enter stage 2 (§5.2 contagion). If the pool now holds `q` votes with
   identical `(kind, view, bh, R)` and no QC of that kind and view is held: form the Qc from
   exactly those `q` (§3.4) and run §6.5 with `formed_locally = true`.

Any member aggregates any votes it receives; the proxy tail is only the *designated* receiver.
Every vote is signature-checked individually before it is pooled (at most `2(n−1)` checks per
round at `P` at stage 0–1, bounded per view by the one-vote-per-slot rule); aggregation never
verifies unverified votes in bulk.

### 6.5 On `Qc c`

1. `c.kind == Commit`: `c.height == h` → §6.8 (passing `formed_locally`); `c.height > h` → §6.9;
   `c.height ≤ tip.height` → safety monitor (§7.6).
2. `c.kind == Prepare`, `c.height == h`: verify (skip if formed locally or cached).
   a. If `high_pqc` is `None` or `c.view > high_pqc.view`: `high_pqc = c` (lock). The new lock is
      written with the next `persist()` (§7.4).
   b. If `c.view > view`: `advance_to(c.view)` (§6.12). (The lock is updated first so that the
      `keep` list of that view change includes `c`'s block and its execution survives.)
   c. If the body of `c.block_hash` is not held: create `want(c.block_hash, Lock)` with sources the
      signers of `c`.
   d. If `formed_locally && me == P`: `Broadcast(Qc(c))` to `recipients`.
   e. If `c.view == view`: set `t_pqc = now` if unset; if `c.signers` includes a member of
      `setB(h, view)`, raise the stage to ≥ 1; `try_commit()`.

   When this step runs on the `high_pqc` of a TC (§6.7, TC handler rule 3), only 2a and 2c apply:
   the TC's own view change follows at once, and `q` members have timed out of
   `tc.view ≥ c.view`, so no CommitQC can form at `c.view`.

`try_commit()`: if in `(h, view)`, `signer` is set, `timeout_view < view`, `mine.commit` is
`None`, `high_pqc.view == view`, and (`me ∈ setA` or `stage ≥ 1`), with `Q = high_pqc`:
```text
persist()                                                   // writes lock = Q (SR25)
vote = sign(Commit, h, view, Q.bh, Q.R); mine.commit = vote
route(vote); pool_insert(vote); t_lastvote = now
```
A Commit vote does not require local execution of the block. There is no separate "commit"
entry in the record: a Commit at `(h, v)` is always for the lock of view `v` (unique per view by
Lemma 1 and never replaced within a view, §6.5 2a), and that lock is durable before the vote
leaves (SR25). After a restart, `try_commit()` may sign it again; the preimage and, with
deterministic signatures, the bytes are identical.

### 6.6 Timing out (local timer, early timeout, join)

`sign_timeout(w)` with `w ≥ view`, `signer` set, and (`timeout_view` is `None` or `< w`); no-op
otherwise:
```text
if w > view: advance_to(w)                // enter w directly in timed-out state
timeout_view = Some(w)
safety.timeout = Some((w, high_pqc.clone())); persist()    // the exact carried PrepareQC is recorded
t = sign TimeoutVote(h, w, high_pqc); mine.timeout = t
Broadcast(t) to C_h \ {me}; timeout_insert(t)
```
No timeout, whatever its caller, changes the start level: §9.2 raises it only for a slow
execution or a slow *committing* view, never for a view that failed.
`mine.timeout` is the only timeout for `w` this node ever sends: every re-send, the node's own
entry in a TC it forms, and the restart path (§7.4 R4, which rebuilds it from the recorded view and
carried PrepareQC) use exactly it. It is never rebuilt from the current `high_pqc`, which may have
risen since.

Callers:
1. **Early timeout** — the current view's leader is proven faulty or its block is invalid: a
   signed defect in the current view's proposal (§6.2 step 7), proven equivocation of
   `L(h, view)` (§6.2 step 2: two different proposals for `(h, view)`, both validly signed by it),
   or `Invalid` for the proposal's uncertified body (§6.3) → `sign_timeout(view)`.
2. **View deadline** `t_view(h, view)` fires and no CommitQC for `h` is known →
   `sign_timeout(view)`.
3. **Join** (§6.7, `timeout_insert`) → `sign_timeout(w*)`.

After `sign_timeout(w)` the node stays in view `w` (voting closed) until a TC for `(h, ≥ w)`, a
PrepareQC for a view `> w`, a CommitQC for `h`, or a join to a higher view. No further view
deadline fires for `w`; the timeout is re-sent at every rebroadcast.

### 6.7 On `TimeoutVote t` and on `TimeoutCert tc`

On `t` (height `h`, from the wire):
1. Apply §6.1 rule 5. `t.signer < n`; `t.high_pqc` (if any) is a valid PrepareQC for `(I, h)`
   with view `≤ t.view`; `t.sig` verifies over `tmo_preimage(h, t.view, t.high_pqc.map(view))`.
   Failure → drop.
2. Let `old = timeouts[t.signer]`. If `old.view > t.view` → drop. If `old.view == t.view`: same
   `hq` → drop; different `hq` → `ReportEvidence(TimeoutEquivocation)` (deduplicated), drop.
3. If `t.high_pqc` is `Some(Q)`: run §6.5 on `Q` (lock update / round sync).
4. `timeout_insert(t)`.

`timeout_insert(t)` (also for the node's own timeout, §6.0):
- store `t` as `timeouts[t.signer]`;
- **TC formation** (`form_tc`). Let `S = {j : timeouts[j].view == t.view}`. If `|S| ≥ q` and no
  TC for `(h, t.view)` is held: form `tc` from the `q` members of `S` with the highest `hq` (ties:
  lower index), `high_pqc` = the PrepareQC of the maximal `hq`; run the TC handler below;
- **Join (f + 1 rule).** Let `w*` = the `(f+1)`-th largest value of `timeouts[j].view` over all
  `j` (undefined if fewer than `f+1` entries). If `w*` is defined, `w* ≥ view`, and
  (`timeout_view` is `None` or `< w*`): `sign_timeout(w*)`. At least one honest node has
  given up on `w*`, so joining can never be forced by `f` Byzantine nodes alone.

On `tc` (height `h`; from wire, from a proposal's `justify`, from `Status`, or formed locally):
1. Verify (§3.4) unless formed locally or cached. Failure → drop.
2. If `high_tc` is `None` or `tc.view > high_tc.view`: `high_tc = tc`.
3. If `tc.high_pqc = Some(Q)`: run §6.5 steps 2a and 2c on `Q` (lock and want only, §6.5).
4. If `tc.view ≥ view`: `advance_to(tc.view + 1)`; then, if `L(h, tc.view + 1) ≠ me` and `tc`
   did not arrive as that leader's own `justify`, `Send(Tc(tc))` to `L(h, tc.view+1)` (one unicast
   per node: the new leader gets its justification even if it missed timeout votes); if
   `L(h, tc.view+1) == me`, propose (§6.10).

### 6.8 On `CommitQC c` for the current height, commit, and height entry

1. Verify `c` under `C_h` (skip if formed locally or cached).
   1a. If `formed_locally` and `me == P(h, c.view)`: `Broadcast(Qc(c))` to the `recipients` of
   `(h, c.view)` (which include `C_{h+1} \ C_h` when known). This happens before step 5 changes
   `h`; O2 holds it until `P`'s own Commit record is durable.
2. `tip = {height: h, block_hash: c.block_hash, result: c.result, commit_qc: c,
   exec_ok: exec[c.block_hash] == Valid(c.result), exec_req, prev: (old tip.block_hash, old
   tip.result)}` where `exec_req = Some(req)` if
   `exec[c.block_hash] == Pending{req, ..}`, else `None`. `exec_ok` holds only while the entry
   exists, i.e. no `DiscardExecution` has dropped that execution since the driver reported it, so
   the driver still holds that post-state unless it evicted it on its own (then it recomputes,
   O4). A pending execution of the committed block is not lost: its answer is handled by §6.3
   step 0 and may set `exec_ok` later.
3. If `pending_apply` is empty and the body `blocks[c.block_hash]` is held:
   `CommitBlock{block, commit_qc: c}` (the driver applies its cached post-state of that block or
   executes it, and compares, §12.3 O3). Otherwise append `{h, c, parent_hash, parent_result}`
   (the previous tip's hash and result) to `pending_apply`, and if the body is not held create
   `want(bh, Apply)` with sources the signers of `c` and members whose `Status` reported `bh`;
   `CommitBlock` is emitted when every earlier entry has been emitted and the body is held
   (§6.9 rule 6). `CommitBlock` actions are thus emitted in strictly increasing height.
4. If `exec[c.block_hash]` is still `Pending{since}` (kept as `tip.exec_req`),
   `pm.last_exec_ms = max(pm.last_exec_ms, now − since)`; `discard_exec([c.block_hash])` (§6.2,
   which does the same for the other pending executions); pacemaker update (§9.2) with `c.view`
   and `d_c = now − t_body` if `c.view == view`, `proposal` is for `c.block_hash` and `t_body` is
   set (otherwise `d_c` is undefined).
5. **Enter `h+1`** if its configuration is known (iff `applied ≥ h − 1`, §10.2); otherwise set
   `awaiting = true`. While awaiting, the round state of the committed height `h` is frozen (no
   round timers, no signing at `h`); round messages for `h` are handled as for a committed height
   (§6.1 rule 3) and those for `h + 1` as in §6.1 rule 4, except that a proposal for `h + 1`
   cannot be verified yet and is dropped. `Status` (content: §3.5) goes out at the unsettled
   cadence; sync, fetch and `pending_apply` continue.
   Entering: `h += 1; view = 0; t_enter = now; t_prop = t_body = None; timeout_view = None;
   proposal = None; build = None; asked = false; stage = initial stage (§5.2 hint, from c);
   t_ready = t_pqc = t_lastvote = None; mine = empty; repropose = false;
   votes, timeouts, high_pqc, high_tc, exec, cert_cache, reported := empty; blocks and wants :=
   only those of pending_apply; signer = the configured (not retired) key in C_h that does not
   abstain at h and is not unanchored (§7.4); safety = SafetyRecord::fresh(key, h, parent_commit_qc = c)` (in
   memory; written by the next `persist()`) unless a restored record for `(key, h)` exists (§7.4
   R6). While a key is unanchored and `C_{tip.height+2}` is known, drop the `probe` entries whose
   key is not in it and re-check anchoring (§7.4 R2); while it is not known yet, keep them. Recompute topology, `anchor` and deadlines
   (§9), and if `me == L(h, 0)` schedule the proposal (§6.10).
   **Late entry.** `late_entry = true` if this entry did not come from a CommitQC that was formed
   locally, arrived as a `Qc` message, or arrived as a proposal's `parent_qc` — i.e. it came from
   `awaiting` (§6.13), from sync (§6.9 rule 3) or from a `Status` — and after a restart (§7.4);
   otherwise `false`. A late entrant without a proposal asks `L(h, view)` for it at the end of the
   `handle` call (or of `Core::new`) through `request_proposal` (§6.11); sync may enter and leave
   several heights in one call, and only the round the call ends in counts.

The consensus height advances on the CommitQC alone; applying (storage) runs behind it, at most
two heights (§10.2).

### 6.9 Catch-up (sync) and block-body fetch

1. **Trigger.** A `CommitQC` (direct, `Status.committed_qc`, or a proposal's `parent_qc`) for height
   `H > h` sets `sync.target = max(sync.target, H)` and remembers the sender as a source. If `C_H`
   is known the QC is verified first (`sync.target_verified`); otherwise the target is only a hint
   (only accepted from members of `C_h`), never makes the node unsettled (§6.11), and is dropped
   after two consecutive empty `SyncResponse`s from different sources.
2. **Request.** While `sync.target ≥ h`: keep exactly one
   `SyncRequest{from_height, max_count: sync_batch, max_bytes: sync_max_bytes}` outstanding, with
   `from_height = max(h, highest buffered height + 1)`, to one source, rotating to the next
   source on *every* request (sources: the senders of target CommitQCs and members whose `Status`
   shows a higher height); deadline `now + sync_retry`. Before a request, buffered entries above
   a gap in the heights from `h` on are dropped (they are fetched again), so `from_height`
   always continues the contiguous buffered prefix. A request unanswered by its deadline counts
   as an empty response for rule 1. The next request is sent as soon as a
   response has been checked, so fetching the next batch overlaps applying the buffered one, but
   not while the buffer holds ≥ `sync_batch` entries.
3. **Response** `SyncResponse{blocks}` answering an unanswered `SyncRequest` of this node to that
   source (processed whenever it arrives, also after that request was retried elsewhere; its
   entries are self-verifying). Each request is answered by at most one response, which starts at
   that request's `from_height` (an empty response answers the source's latest request); other
   responses are dropped. Before it is checked, buffered entries above a gap in the heights from
   `h` on are dropped (rule 2). Entries below `h` are skipped; every
   other entry must satisfy `c.kind == Commit`, `c.height == block.header.height`,
   `block_hash(block.header) == c.block_hash`, `body_ok(block)`, consecutive heights. The entry
   for the current `h` additionally needs `C_h` known, `verify_qc(c, C_h)`,
   `header.parent_hash == tip.block_hash` and `header.parent_result == tip.result`; it then runs
   §6.8 (commit, enter the next height), and buffered entries are processed in the same way in
   height order. Entries for heights `> h` are buffered (≤ `2·sync_batch`) until their turn. The
   first failing entry is dropped together with the rest of that response. A prefix shorter than
   requested is normal; an empty response counts as empty for rule 1.
4. **Serving** (service messages, any height; per-peer rate limits are the driver's). On
   `SyncRequest` → `ServeBlocks{to, from_height, max_count ≤ sync_batch, max_bytes ≤
   sync_max_bytes}`; the driver answers from its block store with consecutive `(block, CommitQC)`
   pairs, total ≤ `max_bytes` unless a single block, or with an empty response if it holds no
   block at `from_height`. On `BlockRequest{height, bh}`: if `blocks[bh]` is held →
   `Send(BlockResponse{block})`; else `ServeBody{to, height, bh}` (the driver answers with a
   `BlockResponse` from its body store or block store, or not at all).
5. **Wants** are the only owner of body-fetch retries. A want exists for: the current proposal's
   body (including this node's own recorded proposal after a restart, §6.10 rule 0); the block of
   `high_pqc`; the block of `high_tc.high_pqc` (while this node is the leader waiting to
   re-propose); each `pending_apply` entry whose body is not held. At creation and at every
   `fetch_retry`: `FetchBody{height, bh, peers}` with the next `min(2^(attempt+1), |sources|)`
   sources, cycling through the source list (wrapping around), so an honest holder among `f + 1`
   or more sources is reached within `⌈log2(f+1)⌉` attempts; the driver first looks in its local
   body store and block store (answering `BodyAvailable`) and otherwise sends `BlockRequest` to
   `peers`. A want is dropped when satisfied or no longer needed (view or height change).
6. **On `BlockResponse{block}` or `BodyAvailable{block}`:** `bh = block_hash(block.header)`; accept
   only if `bh ∈ wants`, `block.header.height == wants[bh].height` and `body_ok(block)`. A body
   already held is never replaced. Accept → `blocks[bh] = block`; `StoreBody{block}` unless it came
   from the local store; remove the want; then flush `pending_apply`: while its front entry's body
   is held, check `header.parent_hash == entry.parent_hash` and
   `header.parent_result == entry.parent_result` (failure → `Halt(SafetyRecordInconsistent)`: with
   at most `f` faults a certified block always extends the committed parent, so this can fail
   only after local storage corruption, e.g. of the block-store tip used by R5), emit
   `CommitBlock{block, commit_qc}` and pop it. If the body is the current proposal's, continue at
   §6.2 step 9; if `repropose`, propose (§6.10).
7. While `sync.target > h + 1` the node still runs the round rules for `h`; nothing waits for sync
   to finish.

### 6.10 Proposing and heartbeat (empty blocks)

A leader can always propose; nothing gates it except being `L(h, v)` in round `(h, v)` with a
signing key, `timeout_view < v`, and no proposal recorded at `(h, v)` (`safety.proposal`).
0. **Never regenerate.** If the record holds a proposal at `(h, view)` (after a restart, §7.4 R4),
   the leader re-sends exactly that proposal — body from its body store (`FetchBody`, local),
   `justify` and `parent_qc` from the record, and the signature re-created by signing the same
   preimage (deterministic, §12.1) — and never builds another for that view. If the body cannot
   be loaded it stays silent in that view.
1. **View 0.** At `t_propose = t_enter + pace` (§9.1) push `BuildPayload{req, height: h, view: 0,
   max_bytes: max_block_bytes(h), exec_budget_ms}` (`req` from `next_req`, remembered in `build`);
   the driver answers `PayloadBuilt{req, payload}`, and a `PayloadBuilt` whose `req` is not the
   one in `build` is ignored. If no answer arrives by `t_propose + build_timeout`, use `EMPTY`. If
   the payload is `EMPTY` and `now < t_enter + idle_block_interval`, wait until
   `t_enter + idle_block_interval` or a `PayloadReady{req}` for that request (but not before
   `t_propose`), request again (new `req`) and propose whatever is returned (`EMPTY` on timeout);
   a `PayloadReady{req}` that arrives before the `EMPTY` answer to `req` is kept, and that answer
   then requests again at once:
   **this is the heartbeat; an idle chain produces one empty block per `idle_block_interval`, and
   a transaction arriving at an idle leader is proposed at once.** `PayloadReady` is used for
   nothing else: no node's timers depend on its own queue (§4.3, §9.1).
2. **View `v > 0`, entered via TC `tc`.** Immediately:
   - `tc.high_pqc = Some(Q)` → re-propose `Q`'s block unchanged (header as originally proposed,
     `origin_view < v`). If the body is not in memory: create `want(Q.block_hash, Repropose)` with
     sources `Q`'s signers (local store first), set `repropose = true`, and propose when the body
     arrives if the node is still eligible in `(h, v)`.
   - otherwise → fresh block; payload = `EMPTY` if `v ≥ empty_after_views(h)` (a validity rule,
     §6.2 step 6), else `BuildPayload` (with a new `req`) and `build_timeout` (no pacing, no idle
     wait).
3. **Construct and send.** Fresh header = `{I, h, origin_view: v, parent_hash: tip.block_hash,
   parent_result: tip.result, payload_hash, payload_len, proposer: idx(me), skipped_leaders:
   [L(h,x) for x in 0..min(v, a_h)]}`; a re-proposal reuses `Q`'s header unchanged. Then
   `StoreBody{block}` (unless stored); `safety.proposal = Some((v, bh, justify.clone())); persist();`
   `ad = att_digest(justify, safety.parent_commit_qc)`; sign `prop_preimage(h, v, bh, ad)`;
   `mine.proposal = Proposal{…, justify, parent_qc: safety.parent_commit_qc, payload: Some(..)}`;
   `Broadcast` it to `recipients` (set A first); then handle it as an accepted proposal (§6.2
   steps 8–9).

The leader does not wait for `BlockApplied(h−1)` or for its own execution to propose.

### 6.11 Tick, retransmission, rebroadcast ("state, not custody") and Status

On `Tick`, fire every due deadline in this order: propose/build (§6.10), stage timers (§5.2), vote
retransmits, view `t_view` (§6.6), rebroadcast, Status, probe, sync retry, fetch retry, execution
retry.

- **Vote retransmit.** Votes are re-sent only by this schedule and by the one re-send at stage
  entry (§5.2). An own vote of the current round whose phase QC is not held (a PrepareQC of
  `view` for the Prepare, a CommitQC of `h` for the Commit) is re-sent via `route()` (to `P` at
  stages 0–1, broadcast at stage 2) at `t_vote + t_retx · (2^k − 1)`, `k = 1, 2, …`, spacing
  capped at `rebroadcast_interval`, where `t_vote` is the time of the vote's first send or of its
  stage-entry re-send, whichever is later.
- **Rebroadcast** (every `rebroadcast_interval`):
  1. Re-send `mine.timeout` by broadcast if `timeout_view == Some(view)`.
  2. Leader of `(h, view)` holding its proposal: re-send it (with payload) once per member per
     view, to each member whose latest `Status` is for `(h, view)`, was received at least
     `rebroadcast_interval` after the proposal was first sent, and does not report
     `proposal_hash == bh`. Members that learn `bh` otherwise (PrepareQC, Status) pull the body
     through their wants.
- **Late-entrant re-push** (on receipt, not on `Tick`; exempt from the `Status` rate limit, below).
  A leader of `(h, view)` holding its proposal that receives, from a peer among its `recipients`
  (§6.0), a `Status` for `(h, view)` with `want_proposal` set and without `proposal_hash == bh`
  re-sends the proposal (with payload) to that member at once — no interval gate — at most once
  per member per view (independently of rule 2 above). Only members that lack the proposal and
  entered late or hold evidence of it set the flag (proposal request, below), and the cap holds
  however many flagged `Status` messages arrive, so this cannot amplify: at most one extra payload
  copy per member per view.
- **Proposal request** (`request_proposal`, at the end of every `handle` call and of
  `Core::new`). If the node is not `awaiting`, `L(h, view) ≠ me`, no proposal for `(h, view)` is
  held, `asked` is false, and either (a) `late_entry` holds, or (b) the node holds evidence that a
  proposal of `(h, view)` exists — a verified vote of view `view` in its pools, a PrepareQC of
  `(h, view)`, or a peer's latest `Status` reporting a `proposal_hash` for `(h, view)` — then
  `Send(Status{want_proposal: true, ..})` to `L(h, view)` and `asked = true`. A late entrant thus
  asks in the call in which it entered (§6.8 step 5), and an on-time member whose copy of the
  proposal was lost asks once per view as soon as it sees such evidence (at the latest when the
  other members' stage-2 votes reach it, §5.2). `want_proposal` is set in every `Status` sent
  while (a) or (b) holds and no proposal for `(h, view)` is held.
- **Status.** `Broadcast(Status)` to `C_h ∪ C_{h+1}` (if known) every `rebroadcast_interval`
  while *unsettled* — timed out in the current view, `view > 0`, `awaiting`, a verified
  `sync.target > h`, `late_entry` with no proposal held for `(h, view)`, or stage 2 in the
  current round — and every `status_keepalive` otherwise. (At stage 2 the members that got a
  PrepareQC from a withholding proxy tail no longer re-send their Prepare, so the others learn
  the lock only from `Status`.)
- **Probe** (§7.4 R2; only while some key is unanchored). Every `rebroadcast_interval`,
  `Send(Status{probe: Some(nonce), ..})` to each member of `C_{tip.height+2}` other than itself
  (not sent while that configuration is unknown). While some key is unanchored, every `Status`
  the node sends — broadcast, reply, proposal request or probe — carries `probe: Some(nonce)`, so
  whichever of them a peer's rate limit admits is answered.

On `Status s` from peer `p` (`p` is the authenticated sender, §1.4), in this order:
1. **Echo** (exempt from the rate limit). If `s.echo = Some(e)`, `e.nonce == nonce`, some key is
   unanchored, `C_{tip.height+2}` is known, `e.key` is a member of it and not one of this node's
   keys, and `probe` holds no entry for `e.key` at or below `s.height`: verify `e.sig` under
   `e.key` over `echo_preimage(nonce, s.height)` (§3.3); on success `probe[e.key] = s.height` and
   check anchoring (§7.4 R2). The reply is fresh (it echoes a nonce drawn at this `Init`) and the
   signature, not the channel, identifies the member, so a relayed copy counts for its signer.
   Nothing else in `s` is exempt from the rate limit.
2. **Proposal request** (exempt from the rate limit). If `s.want_proposal`, `(s.height, s.view)
   == (h, view)` and this node leads `(h, view)`: apply the late-entrant re-push (capped at once
   per member per view).
3. **Rate limit.** The rest of a `Status` is processed for at most one `Status` per peer per
   `rebroadcast_interval / 2`; for any other `Status`, stop here — except that its
   `committed_qc` alone is still used (§6.8 for height `h`, §6.9 above it) if its height is
   `≥ h` and above every `committed_qc` height processed from that peer (at most two extra
   verifications per peer and height): the §6.1 rule-3 reply that carries the CommitQC a
   lagging voter lacks often follows the peer's periodic `Status` within the window.
4. Record `(p, s.height, s.view, s.proposal_hash)` in `peers`. If `s.probe = Some(x)`, the node
   has a configured key, and none of its keys (retired ones included) is unanchored or abstaining
   at the current height (R2, R6): `Send(p, our Status with echo: Some(Echo{nonce: x, key, sig}))`
   (at most once per peer per `rebroadcast_interval`), where `key` is its signing key at `h`
   (`keys[signer]`), else its first configured key, and `sig = sign(echo_preimage(x, height))`
   over the `height` of that `Status` (across a key rotation a reply may name a key that is not
   in the prober's `C_{t'+2}`; it then does not count, and replies after the change height do).
   Then, after §6.1 rule 5: `s.committed_qc` → §6.8 (height `h`), §6.9 (height `> h`), or §7.6
   only if its value differs from the committed one; `s.high_tc` → §6.7 if its view is
   `> high_tc.view`; `s.high_pqc` → §6.5 if its view is `> high_pqc.view`. If `s.height < h`,
   reply with our `Status` (rate-limited, §6.1). If `s.proposal_hash` is a wanted hash, add `p` as
   a source.

No message is ever tracked for delivery: a lost message is replaced by the next re-send of the
sender's current state, and memory holds only the current state.

### 6.12 Round synchronisation (`advance_to(w)`, `w > view`)

`view = w; t_enter = now; t_prop = t_body = None; proposal = None; build = None; late_entry = false; asked = false;
stage = initial stage (§5.2 hint); t_ready = t_pqc = t_lastvote = None; mine.prepare =
mine.commit = None; repropose = false;` (recorded votes are never re-sent once the view changed)
drop vote pools (and their `reported` keys) outside `{w−1, w, w+1}`; drop the proposal want;
`keep = [blocks of high_pqc and high_tc.high_pqc]`; `blocks :=` the bodies of `keep` and of
`pending_apply`; `discard_exec(keep)` (§6.2; the executor cancels the rest and prioritises the
next `Execute`; the current proposal's body and execution are dropped unless it is one of these,
and are fetched and requested again if that block is re-proposed; every body stays in the
driver's body store); recompute `anchor`, reschedule the stage timers and `t_view` with
`level(h, w)` (§9); if entered via `TC(w−1)` and `me == L(h, w)`, propose (§6.10). Triggers (any
valid certificate seen, from any message, including `Status` and a proposal's `justify`):
- TC for `(h, x)` with `x ≥ view` → `advance_to(x + 1)`;
- PrepareQC for `(h, x)` with `x > view` → `advance_to(x)` (the node may Commit-vote in `x`);
  not for a TC's `high_pqc`, whose TC moves the node past `x` at once (§6.5);
- CommitQC for `h` → §6.8;
- `f+1` timeouts at `≥ w*` → join (§6.7 `timeout_insert`) enters `w*` timed-out.
`view` never decreases within a height.

### 6.13 On `BlockApplied{height: a, block_hash, header, config_after_next}`

Require `a == applied + 1`, `header.height == a`, `block_hash(header) == block_hash`, and that
`block_hash` is the block this core committed at `a`: `block_hash == tip.block_hash` if
`a == tip.height`, `block_hash == tip.prev.0` if `a == tip.height − 1` (`tip.prev` is always set
then: that tip was reached by a commit after `Init`); any other `a` (in particular
`a > tip.height`: the driver applies only committed blocks) fails. Otherwise
`Halt(DriverAnomaly)` (§12.5: the driver broke O3, and a wrong header would silently change
`D_h`). Then `applied = a`; `configs[a+2] = config_after_next`; append `header` to
`recent_headers` (keep the last `W + 2`); prune `configs` below `tip.height − 1`. If `awaiting`
and the configuration of `tip.height + 1` is now known, enter it (§6.8 step 5; a late entry). If
`a == h − 1` and the current proposal's body is held with no `exec` entry, `request_exec` it
(§6.2). Retry buffered sync entries.

---

## 7. Safety

### 7.1 Sign-once rules and the complete list of safety rules

Per instance `I`, height `h`, signing key:
- **S1 Proposal.** At most one proposal per `(h, v)` (`safety.proposal`); after a restart only
  the recorded one is re-sent.
- **S2 Prepare.** At most one Prepare per `(h, v)`; only in the current view `v`; only if
  `timeout_view < v`; only for a proposal of that view that passed §6.2 (TC rule included) with
  `Valid(R)` (and `R == Q.result` for a certified block).
- **S3 Commit.** At most one Commit value per `(h, v)`; only in the current view; only if
  `timeout_view < v`; only for `(bh, R)` of the lock, a valid PrepareQC of view `v`, which is
  durable in the record before the vote leaves (the lock is the record of the Commit).
- **S4 Timeout.** At most one timeout per `(h, v)`; only for `v > timeout_view`; it carries the
  signer's lock at signing time (`hq ≤ v`), and only that exact message is ever re-sent.
  **Timeout fence:** after signing a timeout for `w` the node never signs Prepare or Commit at any
  view `≤ w` (S2/S3 conditions).
- **S5 Monotonicity.** `view` never decreases within a height; heights never decrease; the lock
  never decreases within a height.

Every rule the agreement and validity arguments rely on, with the mutation (§13.4) that must be
killed by a named deterministic test:

| Rule | Statement | Where | Mutation |
|---|---|---|---|
| SR1 | ≤ 1 proposal per `(h,v)`; a restarted leader re-sends the recorded proposal or nothing | §6.10 | MS1 |
| SR2 | ≤ 1 Prepare per `(h,v)` (`safety.prepare` checked by `try_prepare`) | §6.3 | MS2 |
| SR3 | Prepare only for the held proposal of the current view | §4.3 | MS3 |
| SR4 | Timeout fence (no Prepare/Commit at a view ≤ `timeout_view`) | §4.3, §6.5 | MS4 |
| SR5 | Commit only on a PrepareQC of the current view (no stale-QC Commit) | §6.5 | MS5 |
| SR6 | ≤ 1 Commit value per `(h,v)`: derived from SR5, SR7 and SR25 (a Commit is signed only over the lock of the current view, which is unique per view and never replaced within it) | §6.5 | — (MS6 removed: equivalent mutant, Appendix C) |
| SR7 | Lock = highest-view PrepareQC, never decreasing within a height | §6.5, §7.2 | MS7 |
| SR8 | A timeout carries the lock held at signing time | §6.6 | MS8 |
| SR9 | ≤ 1 timeout per `(h,v)`; only the stored exact timeout is re-sent or used as own TC entry | §6.6 | MS9 |
| SR10 | Voter TC rule: a TC-justified proposal must be `TC.high_pqc`'s block, and `R == Q.result` | §6.2, §6.3 | MS10a, MS10b |
| SR11 | TC formation: `high_pqc` = PrepareQC of the maximal signed `hq` | §6.7 | MS11 |
| SR12 | TC verification recomputes the maximal `hq` and checks `high_pqc` against it | §3.4 | MS12 |
| SR13 | Quorum `q = n − f` everywhere | §1.2 | MS13 |
| SR14 | A certificate needs ≥ `q` distinct, valid signers | §3.4 | MS14 |
| SR15 | Certificates verified only under `C_{cert.height}` | §3.4 | MS15 |
| SR16 | Instance id in every preimage and header | §3.2–3.3 | MS16 |
| SR17 | Result bound in every vote preimage | §3.3 | MS17 |
| SR18 | Proposal signed by `L(h,v)` over `(h, v, bh, ad)` | §6.2 | MS18 |
| SR19 | Header linkage (`parent_hash`, `parent_result` == committed tip; `parent_qc` verified) | §6.2 | MS19 |
| SR20 | `body_ok` at every body intake | §3.2 | MS20 |
| SR21 | Commit only on a verified CommitQC, on every path (live, Status, `parent_qc`, sync, R5) | §6.8 | MS21 |
| SR22 | Sync entry checks (`block_hash == qc.block_hash`, parent link, consecutive heights) | §6.9 | MS22 |
| SR23 | Persist-before-send for signed messages | §7.4, O2 | MS23 |
| SR24 | O2 total barrier: no externally visible effect (own-signed certificates, `CommitBlock`, served blocks, evidence) before durability | O2 | MS24 |
| SR25 | The lock is written with every record, in particular with the Commit | §6.0 `persist()` | MS25 |
| SR26 | Timeout recorded with its exact carried PrepareQC; `timeout_view` restored | §6.6, R4 | MS26 |
| SR27 | Prepare recorded and restored | §6.3, R4 | MS27 |
| SR28 | (removed in revision 4: the record's `commit` field is gone; SR25 records the Commit through the lock) | — | — |
| SR29 | Proposal (view, bh, justify) recorded and restored | §6.10, R4 | MS29 |
| SR30 | The recorded lock is restored into `high_pqc` | R4 | MS30 |
| SR31 | A missing record makes the key abstain wherever it may have signed: unanchored until fresh probe replies signed by `2f + 1` member keys of `C_{t'+2}` other than its own report committed heights `≤ t'`, then abstaining at heights `≤ t' + 2`; a node with an unanchored or abstaining key answers no probe; only replies that echo the probe nonce under a valid echo signature (§3.3) count, for the key that signed them | R2 | MS31, MS31b, MS31c, MS31d, MS31e |
| SR32 | R5 verifies the recorded parent CommitQC; every `pending_apply` flush checks parent linkage; R6 abstains below the record's height | R5, §6.9, R6 | MS32a, MS32b, MS32c |
| SR33 | One record per (instance, key), never rolled back, copied or dropped (retired keys included), initial record only at the installation event, never over an existing record file, and "generated on this node" trusted only while the installation log's store id matches the record store's; the signing key is chosen by membership in `C_h`; restart composes all keys into one starting round | §7.4 | MS33a, MS33b, MS33c, MS33d |
| SR34 | Apply compares the resulting commitment (cached post-state or re-execution) with `commit_qc.result` | O3 | MS34 |
| SR35 | Evidence only from signed-content defects; early timeouts only from those, from proven equivocation of the view's leader (two different proposals for `(h, view)` both signed by it), or from `Invalid` of an uncertified `body_ok` block | §6.2, §6.3 | MS35 |
| SR36 | Execution mismatch on a certified block (or `Failed`) is a local fault, never an early timeout or `PayloadRejected` | §4.2 | MS36a, MS36b |
| SR37 | Safety monitor halts on a conflicting CommitQC for a committed height | §7.6 | MS37 |
| SR38 | Every vote is signature-verified before it is pooled; locally formed certificates are therefore valid without re-verification | §6.1, §6.4 | MS38 |

### 7.2 Lock

`high_pqc` is the highest-view PrepareQC seen at the current height (from votes, QCs, TCs,
timeouts, proposals, Status). It is carried in every timeout vote, written with every safety
record, restored on restart, and reset only when the height changes. It is *not* a voting filter.

### 7.3 TC rule (Jolteon / HotStuff-2 style) and why there is no voter-side lock check

- A proposal at view `v > 0` MUST carry a valid TC for `(h, v−1)`. If `TC.high_pqc = Some(Q)`, the
  proposal MUST be `Q`'s block and voters bind `Q.result`; if `None`, any fresh valid block.
- `TC.high_pqc` is the PrepareQC with the maximum `hq` over the TC's *signed* entries. Verifiers
  recompute the maximum from the signed `hq` values (§3.4); an aggregator cannot attach a lower
  PrepareQC than its signers declared, and a signer cannot inflate `hq` because the individual
  timeout vote must carry a valid PrepareQC of that view to be aggregated.
- A TC formed from any `q` timeouts is valid; honest aggregators prefer entries with the highest
  `hq` (§6.7), but safety does not depend on that.
- The `justify` TC and the parent CommitQC are covered by the leader's signature (`ad`). Safety does
  not need this (both are self-verifying), but it makes every TC-rule violation attributable to the
  leader and denies relays any way to make an honest leader's proposal look defective.
- **No voter-side lock check.** Safety follows from the TC rule alone (§7.5). A lock check
  ("refuse unless the proposal matches my lock or the TC shows a higher QC") would make the vote
  depend on local state: when a Byzantine node or a race composes TCs from `q` timeouts that exclude
  the honest lock holders (legal: any `q` suffice), the lock holders refuse, the Byzantine withhold,
  and the view fails; this can repeat indefinitely before and after GST. That is precisely a local
  refusal reason (v2 needed "late TC upgrade" machinery for it). Sumeragi votes on the TC's
  verdict; the lock's job is to be *carried* into timeouts.

### 7.4 Persisted safety record, body durability, persist-before-send, restart

```rust
struct SafetyRecord {
    instance: Hash32,
    key: PublicKey,                                  // the consensus key this record belongs to
    height: u64,                                     // round height the record describes
    parent_commit_qc: Option<Qc>,                    // CommitQC of height − 1 (None at g + 1)
    proposal: Option<(u64, Hash32, Option<TimeoutCert>)>, // (view, bh, justify) of the last proposal signed
    prepare: Option<(u64, Hash32, Hash32)>,          // (view, bh, R) of the last Prepare signed
    timeout: Option<(u64, Option<Qc>)>,              // (view, exact PrepareQC carried) of the last timeout signed
    lock: Option<Qc>,                                // high_pqc at write time; also the record of every Commit (§6.5)
    high_tc: Option<TimeoutCert>,                    // high_tc at write time (the certificate that justified the view)
}
```
Norito-encoded, followed by `H(encoding)`; the driver keeps one file per `(instance, key)` and
writes it atomically (temp file, fsync, rename, fsync dir). Size ≤ ~3 KB at n = 31. Nothing else is
persisted by the core; the block bodies it relies on are written by the driver on `StoreBody`.
Votes, pools, pacemaker level and routing stage are re-derived or re-fetched.

**Body durability.** Every body the core accepts is handed to `StoreBody`, and a `PersistSafety`
is durable only after every earlier `StoreBody` (§12.3 O2). Hence the body of every block this node
proposed or Prepare-voted is durable before that signature leaves the node. The driver keeps stored
bodies of height `x` until `x` is applied (the committed body then lives in the block store).
Consequence (liveness): the block of every PrepareQC is durably held by ≥ `q − f ≥ f + 1` honest
members until its height is applied, and every committed block is held by them until its height is
durably in their block stores. Because fresh blocks from view `empty_after_views` on are `EMPTY`,
a height accumulates at most `empty_after_views` non-empty stored bodies plus header-sized ones.

**Persist-before-send (O2).** No externally visible effect of anything a node does after a
`PersistSafety` may exist outside the node until that record is durable: no message, no
certificate containing its signature (including one it formed itself), no block written to the
block store and served from there, no evidence. A crash before durability therefore takes every
such effect with it. Invariant (oracle O-PBS): whenever a signed message, or any object containing
this node's signature, leaves a node, the node's *durable* record covers each such signature — for
a proposal, Prepare or timeout: the same height and entry, a later entry of the same kind at a
higher view, or a higher height; for a Commit at `(h, v)`: a record at `h` whose lock has view
`≥ v`, or a higher height. Probe echo signatures (§3.3) are exempt: they carry no sign-once
obligation and are never recorded.

**Record provenance (normative for the driver and operators).** R3–R6 below trust a `Present`
record to describe the key's latest signatures for `I`. Therefore:
1. A record file is written only by `PersistSafety` and by the installation event below. It MUST
   NOT be restored from a backup, copied to another disk or node, or moved together with a key;
   backups MUST exclude record files. (A rolled-back record would let R3/R4 sign again below the
   key's real history; like a second Byzantine node, this is outside the model, §1.3.)
2. **Installation event.** The driver keeps, outside the record files (e.g. in the node's key
   store), a durable *installation log*: one entry per key stating whether the key was generated
   on this node or imported (from a KMS, a backup or another node), and one entry per
   `(instance, key)` ever started on this node. When the node starts instance `I` with key `K` and
   the log has no `(I, K)` entry, the driver — only if `K` counts as generated on this node
   (rule 3), or the operator asserts that `K` has never signed for `I` (e.g. at `I`'s genesis) —
   first writes and syncs the initial record `{instance: I, key: K, height: g, everything else
   None}`, unless a record file for `(I, K)` already exists (the installation event never replaces
   a record file: a rolled-back log can lack the entry of a record in use); then it writes and
   syncs the `(I, K)` entry; only then does it start the core. A key generated on this node and
   never exported cannot have signed for `I` anywhere else, so a dataspace or lane created after
   the key was installed gets its initial record automatically.
3. **Store id.** "Generated on this node" is bound to the record store. Every log entry is written
   with a freshly drawn random 128-bit *store id*: the driver writes and syncs the id in a file
   next to the record files (part of the record store: never backed up or restored, like the
   records), then writes and syncs the entry carrying the same id. At start, before any
   installation event, the driver compares the id next to the record files with the id in the
   log's newest entry. If they differ, or one of them is missing while the log has entries, the
   log does not describe these record files (the key store was restored from a backup older than
   some installation event, e.g. older than a dataspace instance, or the record store is new or
   was replaced): before anything else the driver durably marks every key in the log as imported
   (with a fresh id, like every entry). Every key is then treated as imported, so no initial record
   is written without an operator assertion and every missing record is `Absent`. (A crash
   between the two writes of an entry also shows a mismatch; that is safe and only costs operator
   assertions for instances created later.)
4. Once the `(I, K)` entry exists, a missing record file is `Absent`; the driver never re-creates
   it. Without an initial record the core also sees `Absent`. Reinstalling a key (new disk, lost
   key store, import from a KMS) or restoring the key store from a backup therefore yields
   `Absent` for every missing record unless the operator asserts, per instance, that the key has
   never signed there.

**Restart.** `Init` carries, for every configured or retired key (**Keys** below),
`RecordState = Present(bytes) | Absent`,
plus the block-store tip `t` (header, result, CommitQC), the configurations of `t` (needed to
verify proposals' `parent_qc` and the stage hint at `t + 1`; omitted when `t = g`), `t+1` and
`t+2`, the last `W + 2` committed headers, `demotion_window` and `nonce` (a fresh random 64-bit
value drawn by the driver for every `Init`; the simulator draws it from its seeded PRNG).
`restore` composes all keys into one starting round:
1. **R1** For every key: `Present` but checksum/decode fails, or `instance`/`key` mismatch →
   `Halt(SafetyRecordCorrupt)`.
2. **R5** If some `Present` record has `height == t + 2` (the block store lost the last commit: a
   crash after entering `t+2`, before the apply of `t+1` was durable): verify its
   `parent_commit_qc` under `C_{t+1}` as a CommitQC for `t + 1`; failure →
   `Halt(SafetyRecordInconsistent)`. On success commit it (§6.8 steps 2–3: `tip` becomes `t + 1`;
   the `pending_apply` entry carries the old tip's hash and result as parent, and its want looks in
   the local body store first; that the block extends the old tip is checked when the body
   arrives, §6.9 rule 6), and continue with `t := t + 1`. At most one key has a record at any
   height (two configured keys in one `C_h` sign with neither, **Keys** below).
3. Every key is then classified against `t`:
   - **R2** `Absent` → the key is *unanchored* (below).
   - **R3** `record.height ≤ t` → nothing to restore: those heights are committed and never
     signed again.
   - **R4** `record.height == t + 1` → this record restores the round (step 4).
   - **R6** `record.height ≥ t + 2` (the block store lost its tail) → the key abstains at heights
     `< record.height` (`abstain_below = record.height`), `LocalFault(StoreBehindRecord)`; the
     node syncs, and on reaching `record.height` the round is restored from this record as in R4
     (§6.8 step 5).
4. **Round.** The node starts in round `t + 1` (a late entry, §6.8 step 5). If a key has an R4
   record whose `parent_commit_qc` is not a valid CommitQC of `t` for `(tip.block_hash,
   tip.result)` (or is present at `g + 1`, absent above it), the record contradicts the block
   store: `Halt(SafetyRecordInconsistent)` (its parent CommitQC would be the `parent_qc` of a
   re-sent proposal). Otherwise restore from it: `proposal`, `prepare`, `timeout`, `lock`, `high_tc` into `safety`;
   `high_pqc = lock`; `high_tc = record.high_tc`; `timeout_view = timeout.view`; `mine.timeout` =
   the timeout rebuilt from the recorded `(view, carried QC)` (re-signing an identical preimage
   gives identical bytes, §12.1); `view` = the maximum of `proposal.view`, `prepare.view`,
   `timeout.view`, `lock.view` and `high_tc.view + 1` over the entries present (0 if none; the
   views of `parent_commit_qc` and of certificates nested inside entries are not used); timers
   start at `now`. The recorded timeout is re-sent at the first rebroadcast and a recorded Prepare
   of the current view on the retransmit schedule with `t_vote = now`; `try_commit()` is called
   (it re-signs the identical Commit if the lock is of the current view); a recorded proposal is
   re-sent only as recorded (§6.10 rule 0). If `me == L(h, view)`, no proposal is recorded at
   `view` and `timeout_view < view`: at `view > 0` with `high_tc` = TC(`view − 1`) propose as
   §6.10 rule 2 with it; at `view = 0` schedule the proposal as §6.10 rule 1. Without an R4
   record the round starts fresh at `t + 1`.

**R2 (lost record).** An unanchored key signs nothing; `LocalFault(RecordMissing)`. The node
probes (§6.11): every `rebroadcast_interval` it sends `Status{probe: Some(nonce)}` to the members
of `C_{tip.height+2}` other than itself, and every other `Status` it sends carries the probe too.
It keeps, per member key of `C_{tip.height+2}`, the lowest `height` reported in a *fresh* reply —
a `Status` whose `echo` carries this node's `nonce` and a valid signature of that key over
`echo_preimage(nonce, height)` (§3.3). The signature, not the channel, identifies the member and
binds the reported height, so relayed replies count and a relay can neither forge nor alter one.
A node answers probes only while none of its keys (retired ones included) is unanchored or
abstaining at its current height. Let `t' = tip.height` (the node's committed height; its round
is `t' + 1`). On every fresh reply, every height entry and every `BlockApplied` that makes
`C_{t'+2}` known (at a height entry it usually is not known yet, and later replies report
higher heights, which never lower an entry) the node checks: if `C_{t'+2}` is known
and fresh replies signed by `2f + 1` distinct member keys of `C_{t'+2}` other than the node's own
keys (`f` of `C_{t'+2}`) each report `height ≤ t' + 1` (a committed height `H_m ≤ t'`), the key becomes
*anchored*: it abstains at every height `≤ max(t', H) + 2 = t' + 2` (`H` = the highest reported
committed height; `abstain_below = t' + 3`) and signs normally, with a fresh record, from
`t' + 3`. Lemma 0 (§7.5) proves that no signature the key made before the loss is at a height
above `t' + 2`, whatever else was lost (the block-store tail included) and however the committee
changed. The threshold is `2f + 1`, not `q`: it is the smallest count that meets the honest
signers of every CommitQC of `C_{t'+2}` (Lemma 0), and with the node that lost its record counted
as one of the `f` faults, `n − f = q ≥ 2f + 1` honest other members can answer. Reported heights
need no verification beyond the echo signature: only an honest reply can make the check fail, and
a Byzantine member can at most count as one of the `2f + 1`. The probe waits until the node has
caught up with `2f + 1` members of the next height's committee; until then the node is an
observer, and for liveness it counts as faulty until it anchors (§1, §8.1). With `n = 1` there is
no other member, so a lost record can never be anchored (re-signing without the record could
commit two blocks at one height); with `n ∈ {2, 3}` (`f = 0`) one fresh reply anchors, because
every certificate contains every member.

**Keys.** The core signs at `h` with the unique configured key that is in `C_h`, is anchored and
does not abstain at `h`; if two configured keys are in `C_h` it signs with neither
(`LocalFault(KeyConflict)`). The application MUST NOT schedule a committee that contains two keys
of one validator: a key rotation replaces the old key by the new one at a single height (§10.3)
(a two-step, make-before-break rotation would make that node sign with neither key while both
are members). A key that is no longer configured for signing but has an installation-log entry
for `I` is *retired*: the driver keeps its record and still passes it in `Init`, and `restore`
treats it like any other key (R1–R6, probing and the probe-answer rule) except that it never
signs. The driver never drops a retired key's record or its log entries: with the `(I, K)` entry
in the log, a dropped record would be `Absent` at every later restart (rule 4 above), leaving the
key unanchored and the node answering no probe (§6.11) until it anchors again; nodes restarting
together in that state could not anchor one another. Size bound: the record store holds one
record (≤ ~3 KB at n = 31) per `(instance, key)` ever installed on the node, i.e. per instance
≤ ~3 KB × (1 + number of key rotations), and `Init.records` and `keys` have one entry per such
key. Key rotation therefore needs no special handling: configure both keys across the change
height.

### 7.5 Agreement proof sketch

*Lemma 0 (anchoring after a lost record).* Let key `K` of node `X` lose its record, let `h_r` be
the highest height at which `K` signed before the loss, and let `K` be anchored with committed
height `t'` (R2). Then `h_r ≤ t' + 2`, so abstaining at every height `≤ t' + 2` covers every
height at which `K` may have signed.
*Proof.* Suppose `h_r ≥ t' + 3`. `X` was in round `h_r`, which it can enter only after committing
every lower height through a valid CommitQC, so a valid CommitQC `c` for height `t' + 2` existed
before the loss. Let `S` be its honest signers other than `X`: `|S| ≥ q − f − 1` (`n`, `f`, `q` of
`C_{t'+2}`). The anchoring replies are signed by a set `Q` of `2f + 1` member keys of `C_{t'+2}`
other than `X`'s, so `|Q ∩ S| ≥ (2f + 1) + (q − f − 1) − (n − 1) = q + f − n + 1 = 1` (`q = n − f`;
`2f + 1` is the smallest size for which this holds for every `n`). Take `y ∈ Q ∩ S`. `y` signed
its Commit at `t' + 2` in round `t' + 2`, before the loss, hence before `X`'s probe. Its reply's
echo is signed by `y` over `X`'s nonce, drawn at `X`'s `Init`, and over the reported height
(EUF-CMA: no relay or other node can produce or alter it), so `y`'s node signed it after that
Commit, reporting its own height. When it answered, `y`'s node had no unanchored or abstaining
key, retired keys included (§6.11, §7.4 Keys), so its current round
was `≥ t' + 2`: without a restart since its Commit, its round never decreased; after a restart
with its record present (never rolled back, record provenance), R3–R5 resume at a round `≥` the
record's height `≥ t' + 2`, and under R6 it does not answer below that height; after a restart
with its record lost, it answers only once anchored and past its abstention, at a round
`> t'_y + 2 ≥ t' + 2` by this lemma applied to `y`'s own earlier anchoring (its Commit at
`t' + 2` preceded its loss; induction on the time of anchoring). So `y`'s signed echo reported
`height ≥ t' + 2`, contradicting the anchoring condition `height ≤ t' + 1` for every member of
`Q`. ∎ The lemma needs no assumption about `X`'s block store (it may have lost any tail), about
committee changes (only `C_{t'+2}` is used, and it is the committee of `c`), or about the channel
a reply arrived on (the echo is signed).

*Lemma 1 (one value per view).* Two valid PrepareQCs (or two CommitQCs) of the same `(I, h, v)`
certify the same `(bh, R)`: they share ≥ `2q − n = n − 2f ≥ f + 1` signers, one of them honest,
and S2/S3 allow one value per kind per view.

*Lemma 2 (committers carry the lock).* Let `c` be a CommitQC for `(B, R)` at `(h, v)` that exists
anywhere other than in the volatile memory of a node that then crashes (by O2 this covers every
CommitQC an honest node can commit on, relay or serve). Let `S` be its honest signers,
`|S| ≥ q − f`. Each `s ∈ S` held a PrepareQC of view `v` as lock when it signed Commit (S3), and
its record containing that lock was durable before its vote, or any certificate containing it,
left `s` (persist-before-send, SR25; O2's total barrier when `s` formed `c` itself). The record is
never rolled back (record provenance) and is restored on restart (R4–R6), and a lost record keeps
`s` from signing at `h` again (R2, Lemma 0: `h ≤ h_r ≤ t' + 2`), so `s` never signs at `h` with a
lower lock. By the timeout fence, `s` signed no timeout for a view `≥ v`
before that Commit. Since the lock never decreases, every timeout `s` signs at `h` for a view `≥ v`
carries `hq ≥ v`.

*Lemma 3 (TCs see committers).* A valid TC for `(h, x)`, `x ≥ v`, has ≥ `q` signers of timeouts for
view `x`; `|TC ∩ S| ≥ q + (q − f) − n = n − 3f ≥ 1`. By Lemma 2 its maximal `hq` is `≥ v`, and
`≤ x` by validity, so `TC.high_pqc` has view in `[v, x]`.

*Theorem.* If a CommitQC for `(B, R)` exists at `(h, v)` (in the sense of Lemma 2), every valid
PrepareQC at `(h, v')`, `v' ≥ v`, certifies `(B, R)`. Induction on `v'`. `v' = v`: the CommitQC has
an honest signer holding a PrepareQC of `v` for `(B, R)` (S3); Lemma 1. `v' > v`: the PrepareQC has
≥ `q − f ≥ 1` honest signer, who accepted a proposal for `(h, v')` justified by a valid TC for
`(h, v'−1)` and applied the TC rule. By Lemma 3 that TC's `high_pqc` has view in `[v, v'−1]` and by
induction certifies `(B, R)`; so the proposal is `B` and the honest signer bound `R` (it votes only
if its own result equals `Q.result`). Lemma 1 gives the whole PrepareQC.

*Corollary (agreement).* Take CommitQCs at `(h, v)` and `(h, v')`, `v ≤ v'`. The one at `v'` has an
honest signer holding a PrepareQC of `v'`, which by the theorem certifies `(B, R)`; S3 binds that
value. Honest nodes commit only on a valid CommitQC (SR21), so all commit `(B, R)`. Chain agreement
follows by induction on height: each header binds `parent_hash` and `parent_result`, `C_h` is a
function of the agreed prefix (§10), and certificates are verified only against `C_h`.

*Views and late certificates.* The argument does not depend on arrival order: a CommitQC of an
old view that surfaces after later TCs is still consistent with everything certified later.
*Timeouts are always safe.* Nothing above uses why or when a node signs a timeout, only S4 (one
per view, carrying the lock, then the fence) and lock durability, so any trigger of
`sign_timeout` — the deadline, a join, or an early timeout on a signed defect, an `Invalid`
block or proven leader equivocation (§6.6) — preserves agreement; a Prepare already signed in
that view stays as it is, and the fence forbids a Commit after it. Early timeouts can only cost
liveness, and only in views whose leader is faulty or whose block is deterministically invalid:
`ProposalEquivocation` consists of two different preimages of `(h, v)` both signed by `L(h, v)`,
which an honest leader never produces (S1: at most one proposal per view, re-sent byte-identical
after a restart), no one else can forge (EUF-CMA), and a relay cannot create by changing unsigned
bytes (the payload is bound through the signed header, so `(bh, ad)` does not change, §3.3).
*Restarts.* The proof uses only S1–S5 and lock durability; R4–R6 restore exactly what was durable
(a `Present` record is the latest one, record provenance), R2 (Lemma 0) and R6 make a key abstain
wherever its record cannot prove what it signed, and O2 guarantees no signature exists outside a
node without its record (a crash before durability only loses messages). R3 is safe because a node
never signs at a height `≤ t` again. Restart composition gives one starting round: R5 commits
`t + 1` before any key is classified, and at most one key has a record at a height, so the round
state comes from a single record and the tip never moves backwards. *Instances.* `I` is in
every preimage and header, so a vote, QC or TC of one instance never verifies in another, even with
identical committees.
*Validity.* A PrepareQC has ≥ `f + 1` honest signers, each of which executed a `body_ok` body of
`B` on the agreed parent state and obtained `R`; `bh` fixes the payload bytes (§3.2), so with
deterministic execution `R = exec(S_{h−1}, B)`. Apply re-checks it (O3, SR34).

### 7.6 Safety monitor

- A valid CommitQC for a committed height `x ∈ {tip.height − 1, tip.height}` whose configuration
  is retained (after a restart `C_{t−1}` is not, so only `tip.height` until the next commit) with
  `(bh, R)` different from the committed value (the core keeps the committed `(bh, R)` of both
  heights) →
  `ReportEvidence(ConflictingCertificates)` and `Halt(SafetyViolation)`: agreement is broken, so
  more than `f` members of `C_x` are faulty. Values are compared first; a signature check happens
  only on a mismatch. CommitQCs for older heights are dropped (§10.7).
- Nothing else is monitored. Conflicting certificates of the current, uncommitted height are
  never even verified (a second PrepareQC of a view already locked is cheap-rejected, §6.1 rule
  5); forensics for them are the application's (§14.6).

---
## 8. Liveness

### 8.1 Assumptions after GST

At least `q_h` members of `C_h` are honest and running, where a member with an unanchored key
(§7.4 R2) counts as faulty until it anchors and a member whose key abstains at `h` (R2, R6)
counts as faulty at `h`; honest-to-honest delay `≤ Δ` for every
message up to `max_block_bytes`, including the last of the `n − 1` payload copies a leader sends
in sequence (the driver's traffic classes, §12.3 O8, keep control traffic independent of bulk
load); local signing/verification/persist per message `≤ δ`; parent apply `≤ A_max`; execution
`≤ E_max`; the driver delivers `Tick` within `δ` of `next_wakeup()` ahead of every other event
(O5); `σ = rebroadcast_interval + Δ`; `F = ⌈log2(f+1)⌉ · fetch_retry` (body fetch, §6.9 rule 5).
`Δ` is unknown to the nodes, but view timers stop growing at `T_max_eff` (§9.1), so liveness
needs the post-GST `Δ` (and `A_max`, `E_max`) to satisfy `T_req ≤ T_max_eff` (§8.2 L3); the
simulator's post-heal delays respect this, and §9.4 validates the nominal case.

### 8.2 Argument

- **L1 View synchronisation (including restarts).** Let `w` be the highest view at `h` that an
  honest node is in (in memory, or durably after a restart). Consider the first honest node to
  reach `w`. It did not get there by joining: a join needs a timeout at `≥ w` from an honest node,
  which would have reached `w` earlier. So it entered via `TC(w−1)` or a PrepareQC of view `≥ w`.
  If it has signed anything in `w`, that certificate is in its durable record (`persist()` writes
  `high_tc` and the lock every time) and is restored on restart. If it has not, a restart takes it
  back to a lower view and it no longer holds `w`. The node is unsettled (`view > 0`), so its
  `Status` carries the certificate every `rebroadcast_interval`, and every honest node reaches a
  view `≥ w` within `σ`. Joins spread through broadcast and re-sent timeouts.
- **L2 Failed views end.** If honest nodes are in `w` and no CommitQC forms, each times out by
  `t_view`, the `≥ q` honest timeouts are broadcast (and re-sent), and every honest node forms or
  receives `TC(w)` within `Δ` (after GST) of the last honest timeout; the `f+1` join pulls
  laggards out of `w` within `Δ` of the `(f+1)`-th honest timeout. A view whose leader is proven
  faulty ends earlier: every honest node that holds both of its twin proposals, or a signed
  defect, times out at once (§6.6 caller 1); once `f + 1` honest nodes have, every honest node
  joins within `Δ` and forms or receives `TC(w)` within `Δ` more; otherwise the view ends by
  `t_view` as above. An early timeout never delays a good view: the
  proof needs a faulty leader (§7.5).
- **L3 Timers become long enough.** `level(h, v) = min(level_cap, start(h) + v)` grows by one per
  failed view, so after at most `k* = min{L : T(L) ≥ T_req}` failed views
  `T ≥ T_req = 2·(σ + build_timeout + 3Δ + F + A_max + E_max) + 4δ` (the `3Δ + F` covers a body
  fetch for a re-proposal). Config validation (§9.4) enforces `T_max ≥ T_req(nominal)` with this
  exact formula, and the effective `T_max` is never below it. This growth within a height is
  independent of the start level: `start(h) ≥ 0` only shortens the path to `k*`, so liveness does
  not depend on the §9.2 adaptation (which only prices failed views at later heights; payload
  progress under executions longer than the payload views' timers does, §9.2).
- **L4 A good view commits.** Consider a view `w` whose leader is honest, entered by all honest
  within `σ`, with `T(level) ≥ T_req`. The leader proposes immediately (TC entry) or at `pace`
  (view 0). A fresh honest block is valid. A re-proposed block has a PrepareQC, so ≥ 1 honest node
  computed `Valid(Q.result)` and, by determinism, all honest do; its body is durably held by
  ≥ `f + 1` honest members (§7.4), so the leader and the voters fetch it within `3Δ + F`. A node
  that enters `h` late (after apply, sync or a restart) gets the proposal from the leader within
  `2Δ` of entering, through its `want_proposal` Status (§6.11), which the leader acts on even
  when the rest of that `Status` is over its rate limit; a member whose copy was lost asks the
  same way as soon as it holds a vote, PrepareQC or `Status` showing the proposal exists, at the
  latest when stage-2 votes reach it. All honest nodes execute within
  `σ + build_timeout + 3Δ + F + A_max + E_max ≤ φ·T`. If every set-A
  member and `P` are honest, stage 0 forms both certificates within 4 hops. With faulty non-P
  set-A members (at most `f`, and set B supplies the missing votes), stage 1 does so within
  `t_retx` more, or with no delay when the hint started the round at stage 1. With a faulty `P`,
  stage 2 starts within `2·t_retx` of the members' votes (at the latest at `anchor + φ·T`), and
  broadcast voting forms
  the PrepareQC and CommitQC within `2Δ + 2δ ≤ (1−φ)·T`. No honest node times out first, so the
  CommitQC exists and reaches every honest node within `σ`.
- **L5 Honest leaders recur.** The leaders of any `f + 1` consecutive views of a height are
  `f + 1` consecutive entries of `nd_h`, hence distinct (§2.1, slot substitution rotates views
  over non-demoted members only), so one is correct.
- **L6 Committed bodies are obtainable.** A node that commits without the body fetches it with
  `BlockRequest`, which any holder answers at any height (§6.9); ≥ `f + 1` honest members hold it
  durably (§7.4). So `pending_apply` drains, `applied` advances, and the next configuration becomes
  known.

**Bound (used by O-LIVE).** After heal/GST at time `t_g`, every honest running node commits a new
height by `t_g + B_live` with
`B_view = T_max_eff + idle_block_interval + 2·build_timeout + 2·rebroadcast_interval + 4Δ + F`
and
`B_live = (f + 2 + level_cap) · B_view + ⌈lag / sync_batch⌉ · (sync_retry + 2Δ + sync_batch · (E_max + A_max))`,
where `lag` is how many heights the node is behind (synced blocks have no cached post-state, so
the driver executes each before applying it). This bound is deliberately loose. It only catches
stalls; performance regressions are caught by the tighter bounds below.

**Performance bounds (used by O-PERF, §13.2).** Let `G_norm = X + E + 5Δ + 4δ`, where `X` is
`block_time` while transactions are pending and `idle_block_interval + build_timeout` while idle
(the heartbeat's second build request may take up to `build_timeout`), and let
`t̂ = φ·T(level(h, 0))/2`, the upper end of the `t_retx` clamp (§9.1). The bounds use `t̂`, not the
measured `t_retx`, because a Byzantine proxy tail can inflate the measured value up to the clamp.
- **P1** All honest, loss ≤ 1 %: p99 of commit gaps ≤ `G_norm`, and every gap ≤
  `G_norm + 2·rebroadcast_interval + Δ` (a proposal copy lost to two members the quorum needs is
  recovered by their proposal request once they see evidence of it, at worst the stage-2 votes,
  or by the leader's re-push). This includes heights at which one member enters late (apply
  up to `A_max`, or just synced): its re-pushed proposal adds about `2Δ + E` for that member only.
- **P2** One member that withholds its votes (in either phase or both) but proposes and
  aggregates correctly, lossless links: every gap ≤ `G_norm + t̂`, and with `D` fixed at most one
  gap per `n` consecutive heights exceeds `G_norm` (its entry into set A when the hint is off;
  while it stays in set A the parent CommitQC needs set B, so the hint starts every round at
  stage 1, §5.2). It is never demoted, so this recurs indefinitely.
- **P3** Silent or withholding proxy tail: each gap ≤ `G_norm + 2·t̂ + Δ`. A given member
  causes such a gap only at its view-0 proxy-tail turns: at most `1 + |D_h|` heights per `n`
  consecutive heights (§2.1 fairness; demoted slots pass to successors).
- **P4** Crashed member: per `W` consecutive heights, at most one gap up to
  `P(0) + T(start) + G_norm`, plus `2·t̂ + Δ` when `a_h = q` (then the crashed member is view 1's
  proxy tail, §2.1) — its view-0 leader turn; it is demoted two heights later — plus one more
  such gap for every honest leader recorded in `skipped_leaders` inside the window (an honest
  skip, e.g. through a poison transaction, can displace it from `D_h` because `|D_h| ≤ f`); at most
  two P3 gaps (its proxy-tail turn before that, and, when `a_h = q`, the height right after it)
  and at most one P2 gap (its first set-A height with the hint off); every other gap meets P1. A
  failed view followed by a re-proposal does not record its leader (§5.2); scenarios that check
  P4 do not lock blocks without committing them.

Throughput consequences (revision 3, no absence demotion). Under load the pace (§9.1) makes the
cycle time `max(block_time, commit latency)`, so a gap above `block_time` costs throughput one for
one. A vote-withholding member costs at most one `t_retx` per `n` heights (P2) plus `2f` vote
messages at every hinted height; with `f` withholding members at `n = 22` the hint is on almost
always, set B votes at every height (105 instead of 91 messages, up to 21 instead of 14 vote
verifications per phase at `P`), and gaps meet P1. A crashed or withholding proxy tail costs
`2·t_retx + Δ` per turn (P3); at `n = 22` with the default `t_retx ≈ 750 ms` and one faulty member
that is ≈ 1.6 s extra once per 22 heights (≈ 6 % throughput loss at a 1 s block time); `f = 7`
Byzantine members that withhold as proxy tail but lead correctly (never demoted) cost it at
roughly one height in three (≈ 30 % throughput loss). Crashed members stop costing after their
first leader turn (demotion) until they are reinstated `W` heights later. Revision 2 also removed
silent members from set A after `f + 1` heights; revision 3 keeps paying P3 at their proxy-tail
turns in exchange for no absence records in headers.
- **P5** After heal at `t_g`, in scenarios where every member is honest and none restarted
  (F9–F12 with heal): first commit at every honest node by
  `t_g + P(0) + Σ_{L=ℓ..max(ℓ, k*)} (T(L) + Δ) + 2σ + F + E + 5Δ`, where `ℓ` is the lowest level of
  an honest node's current view at `t_g` and `k*` is as in L3 (views may fail until
  `T(L) ≥ T_req`; every leader is honest, so the first such view commits).
- **P6** Under the F29 flood, every `Tick` is handled within `ε_tick` of `next_wakeup()`.

### 8.3 Scenario checklist

| Fault | Mechanism | Typical cost |
|---|---|---|
| Silent/crashed leader | view timer → TC → next leader; `skipped_leaders` demotes it for `W` heights and its slots pass to its successor; the start level is unchanged (§9.2) | `P(0) + T(start)` (7.2 s at `n = 4` defaults, idle or loaded) once per `W` heights |
| Silent / slow non-P set-A member | stage 1 (set B → `P`); from the next height the hint (parent CommitQC needed set B) starts stage 1 at once; never demoted | `t_retx` once per `n` heights, then `2f` extra messages per height |
| Silent / withholding / selective proxy tail | stage 2 (readiness timer or contagion), every member aggregates; never demoted | `≈ 2·t_retx + Δ` per proxy-tail turn (1 to `1 + |D|` turns per `n` heights) |
| Member that Prepares but withholds its Commit | stage 1 on the overdue CommitQC (`t_pqc + t_retx`) | `t_retx` |
| Late entrant (slow apply, joiner, just synced, restarted) | `Status{want_proposal}` to the leader → immediate re-push, even if the rest of that `Status` is rate-limited | `≈ 2Δ` for that member |
| Lost proposal copy at an on-time member | a vote, PrepareQC or `Status` of the round shows the proposal exists → one `Status{want_proposal}` per view to the leader → immediate re-push | `≈ 2Δ` after that evidence (at worst the stage-2 votes, `≈ 2·t_retx`) |
| Equivocating leader | ≤ 1 PrepareQC per view (S2); evidence; every honest node holding both twins times out early (§6.2 step 2), `f + 1` of them pull the rest (join); a failed view never raises the start level (§9.2) | `TC` `≈ 2Δ` after both twins reach `f + 1` honest nodes; one view `T(level)` if fewer see both |
| Adversarial TC composition | no voter lock check; TC rule only | none |
| Relay tampering (payload, attachments) | signature covers attachments; `body_ok`; silent drop + fetch; no evidence | none |
| Byzantine body / sync responder | `body_ok`, wanted-only acceptance, rotation over sources | one fetch retry |
| Lossy links (10–30 %) | vote retransmit to `P`, QC answers (Prepare) and Status replies (Commit), state rebroadcast, proposal re-push, body pull | a few `t_retx` |
| Slow executors (≤ f) | they fall into the `f` that stage 1/2 tolerates | `t_retx` |
| Slow executors (all) | level grows ×1.5 per failed view; start level adapts (an execution — also one discarded unfinished, as a lower bound — or the committing view `> T(start)/2`, §9.2); `exec_budget` caps block cost at `φ·T_base/2`; `EMPTY` from `empty_after_views` | converges in `k*` views; payload resumes once the start level covers `E` |
| Late but valid leader (proposal or body sent as late as the view still commits) | the view commits; `d_c` is measured from `t_body`, so the start level is unchanged (§9.2); never skipped, no evidence | its own delay, `< P(v) + T(start)` per turn |
| Poison transaction | `Invalid` → early timeout + `PayloadRejected` → builder quarantines the culpable transaction only; no evidence; start level unchanged | one or two views; that view's (honest) leader is demoted once |
| Lost safety record (with or without block-store loss) | key unanchored until signed probe echoes from `2f + 1` members of `C_{t'+2}` show it caught up; abstains through `t' + 2` | that key signs nothing until then; the node counts as one of the `f` faults until it anchors |
| Lagging node | `Status.committed_qc` / `parent_qc` → sync (one request outstanding, fetch overlaps apply); others never wait | none for others |
| Whole-cluster restart | records (lock, `high_tc`, exact timeout) and stored bodies restored; Status re-synchronises views | ≤ one view |

### 8.4 Bounded memory and storage (what is retained, when it is discarded)

| Item | Bound | Discarded |
|---|---|---|
| vote pools | `3 views × 2 kinds × n` votes | on view advance (outside `{v−1, v, v+1}`), all on commit |
| timeouts | `n` (highest-view per signer) | on commit |
| certificates | `high_pqc`, `high_tc`, `tip.commit_qc`; `cert_cache` ≤ `4n` digests (LRU) | replaced monotonically / LRU / on commit |
| block bodies in memory | ≤ 6: current proposal, `high_pqc`'s, `high_tc.high_pqc`'s, 2 `pending_apply`, 1 in flight | `advance_to` keeps only `high_pqc`'s, `high_tc.high_pqc`'s and `pending_apply`'s; height entry keeps only `pending_apply`'s (all reloadable from the body store) |
| body store (driver, disk) | per height ≤ `empty_after_views × max_block_bytes` + one header-sized body (≤ ~3 KB) per view with a proposal | when the height is applied |
| `exec` entries | one per body in memory, plus `tip.exec_req` | at every `DiscardExecution` not keeping them (view change, commit); all on height entry except `tip.exec_req` (cleared by its answer) |
| probe table | ≤ `n` member keys of `C_{tip.height+2}` | entries of other keys at height entry; all when the key is anchored |
| keys (configured and retired) and their records | one per key with an installation-log entry for `I`; records ≤ ~3 KB each on disk (§7.4 Keys) | never: retired records are kept |
| `wants` | ≤ 5 | when satisfied / no longer needed |
| `pending_apply` | ≤ 2 (consensus ≤ 2 heights ahead of apply) | when `CommitBlock` is emitted |
| sync buffer | ≤ `2·sync_batch` entries and ≤ `2·sync_max_bytes` bytes (one outstanding request) | when processed or on failure |
| peer table | ≤ `n + max_observers` (LRU) | LRU eviction |
| recent headers / configs | `W + 2` / ≤ 4 | sliding window |
| evidence dedup set (`reported`) | ≤ `3 views × (3n + 1)` keys | with the vote pools' window; all on height entry (evidence itself is emitted immediately) |

No in-memory structure grows with the number of views, with time, or with message volume; the
only on-disk structure that does is the body store, by one header-sized body per failed view until
the height is applied. Messages for
heights `> h` (other than those listed in §6.1) and views outside the window are dropped, not
queued; rebroadcast re-delivers the current state later.

---

## 9. Pacemaker

### 9.1 Formulas

```text
T(L)         = min(T_max_eff, T_base · 1.5^L)
level(h,v)   = min(level_cap, start(h) + v)
level_cap    = ceil(log_1.5(T_max_eff / T_base))
P(0)         = idle_block_interval + build_timeout ;   P(v > 0) = build_timeout
anchor(h,v)  = min(t_enter + P(v),  t_prop)
               // t_prop: time the (h,v) proposal was accepted (if any)
t_view(h,v)  = anchor(h,v) + T(level(h,v))                       // view timeout
t_retx       = clamp(3 · qc_lat_ewma, 50 ms, φ · T(level(h,v)) / 2)
t_s1         = min(t_ready + t_retx  [no PrepareQC],  t_pqc + t_retx  [no CommitQC])  // stage 1 (§5.2)
t_s2         = min(t_lastvote + 2 · t_retx,  anchor + φ · T(level(h,v)))   // stage 2, φ = 0.5
t_propose    = t_enter(h,0) + pace,  pace = max(0, block_time − latency_ewma)
exec_budget  = min(e_max, φ · T_base / 2)                         // hint to the payload builder
```
`latency_ewma` = EWMA (weight 1/8) of `t_commit(h) − t_proposal_received(h)` measured locally.
`qc_lat_ewma` = EWMA (weight 1/8, initial 250 ms) of the time from sending an own vote to
receiving that phase's QC, sampled for every own vote whose phase QC arrives, each sample capped
at `2 × qc_lat_ewma` (so a persistent change, such as execution skew in set A, is learned within a
few rounds, and one slow or adversarially delayed round moves the estimate by at most 1/8).
`exec_budget` depends only on `e_max` and `T_base`, never on the leader's level: a leader at a
high level must not build blocks that voters at level 0 cannot execute within their timers. It is
the builder's estimate of execution time in milliseconds (e.g. gas at a calibrated rate); the
core only passes it. Deadlines are recomputed whenever `anchor` moves; `anchor` only moves
earlier within a round. All timers are local; nodes need not agree on them. No timer depends on
the node's own transaction queue: a local queue affects only when this node proposes (§6.10).

### 9.2 Start level adaptation (on commit of `h`)

Let `v_c` be the view of the CommitQC with which this node commits `h`, and
`d_c = t_commit(h) − t_body(h, v_c)`, where `t_body` is when this node first held the proposal of
`(h, v_c)` together with its body (§6.2 step 9), if this node is in round `(h, v_c)` when it
commits and holds that view's proposal of the committed block with its body. Otherwise `d_c` is
undefined: this node left `v_c` for a higher view, never reached it, or holds a twin or no body,
so it never measured that view. `pm.last_exec_ms` is the longest execution at `h`: the maximum of
`now − since` over every answered execution at `h` (§6.3 step 1), every execution still pending
when a view change discarded it (§6.2 `discard_exec`), and the committed block's execution if it
is still pending at the commit (§6.8 step 4). The last two are lower bounds of executions that
outlasted their view.
- **Raise:** if `pm.last_exec_ms > T(start(h)) / 2` (an execution at `h` took more than half the
  base timer here) or `d_c > T(start(h)) / 2` (the committing view was slow here):
  `start(h+1) = min(start_cap, start(h) + 1)`, `fast_streak = 0`.
- **Decay:** else if `v_c == 0`: `fast_streak += 1`; when
  `fast_streak ≥ decay_after`: `start(h+1) = max(0, start(h) − 1)`, `fast_streak = 0`.
- **Otherwise** unchanged. No failed view raises the level by itself, whatever ended it
  (deadline, join or early timeout, §6.6) and whether or not this node held its proposal; a
  crashed leader costs `T(start)`, not more. Only an execution can: a failed view whose execution
  was still running when the view ended counts that execution's elapsed time.
- After restart `start = 0`, `fast_streak = 0` (safe; only costs time).

*Why executions that outlast their view count.* Payload may be proposed only in views below
`empty_after_views`, at levels `start(h)` to `start(h) + empty_after_views − 1`; the `EMPTY`
block of view `empty_after_views` always commits (§8.2 L3) and executes fast. If every executor
takes `E ≥ T(start + empty_after_views − 1)` for a non-empty block — for example
`E ∈ [T(1), e_max]` = 3–4 s at the `n = 4` defaults (the window exists whenever
`e_max ≥ 1.5·T_base`) — every payload view times out while executing, the driver may cancel the
work at the view change (O4), and the committing `EMPTY` view is fast. Without the lower bounds
nothing records the slowness and the chain commits only `EMPTY` blocks forever (in the
simulator, F15 variant 5 with the lower bounds removed: no non-empty block among 19 heights in
120 s). The maximum, not the last value, keeps the fast `EMPTY` execution from hiding the slow
ones. A lower bound never exceeds the real duration of the execution, so a leader gains nothing
through it that the execution clause (item 4 below) does not already give it.

*Why the proposal and its body, not the view entry or the anchor.* At view 0 the entry is
followed by the deliberate wait for the proposal — about `block_time` under load (the pace) and
`idle_block_interval` while idle (the heartbeat) — which alone exceeds `T(0)/2` at every idle
height and, at `n = 4` (`T(0)/2 = 1 s`), at most loaded ones: measured from the entry, the
simulator's healthy idle chain sat at `start_cap` and its loaded `n = 4` chain at level 1. The
anchor `min(t_enter + P(v), t_prop)` removes that wait but still counts a leader's lateness. A
leader whose valid proposal arrives `T(start)/2 + ε` after `t_enter + P(v)`, or which sends the
proposal at once without its payload and serves the body `T(start)/2 + ε` later, makes
`d_c > T(start)/2` at every honest node while the view still commits before its deadline. The
view succeeds, so that leader is never skipped or demoted and leaves no evidence. Measured from
the anchor (or from `t_prop`), such leaders raised every honest node to `start_cap` in the
simulator (F36), and every crashed leader then cost `P(0) + T(4)` — 15.3 s at `n = 4`, 20.4 s at
`n = 22` — instead of `P(0) + T(0)`. From `t_body` the leader controls neither: an honest
proposal carries its payload, so `t_body = t_prop` for an honest leader. The wait for a body that
a relay stripped is not counted either (the start level is not the remedy for lost payloads).

*Why this resists Byzantine members.* (1) A failed view never raises by itself. A faulty leader
that equivocates (early timeout, §6.2 step 2), sends a valid proposal to too few members
(deadline or join timeout while holding it), signs a defect or stays silent costs at most its
view (§8.2 L2) and leaves every honest node's start level unchanged. Revision 4.1 raised whenever
a view timed out while this node held its proposal, which every faulty leader could trigger at
each of its turns; at `n = 22` successive equivocating leaders held honest nodes at levels 3–4
and one height took 26 s (F5 seed 279). (2) Faulty members cannot stretch `d_c` through the
proposal or its body (above), nor through a twin (`d_c` is undefined at a node that holds the
other twin). They can only delay certificates, and only until the stage ladder bypasses them:
each phase at the latest by stage 2 at `t_lastvote + 2·t_retx` (§5.2), so a committing view they
stretch lasts at most about `E + 4·t_retx + 4Δ` from `t_body`. `t_retx ≈ 3·qc_lat_ewma` does not
depend on the level until its clamp, and a delayed round moves `qc_lat_ewma` by at most 1/8
(§9.1), while `T(L)/2` grows ×1.5 per level; so this exceeds `T(L)/2` only at the lowest levels
(level 1 in F2/F3). That bound, not decay, is what limits the level they can hold: faulty members
lead or collect at about `f` of every `n` heights (every 3–4 heights at `n = 3f + 1`), more often
than `decay_after = 8` heights, so a lever that raised the level at every faulty turn would hold
it at `start_cap` however `decay_after` is chosen. (3) Only a view this node measured counts: a
CommitQC of a view it has left, or for a twin of the proposal it holds — for example the first
twin of an equivocating leader, committed through members that voted before the second arrived —
does not. (4) The execution clause: a valid block that takes longer than `T(start)/2` to execute
raises the level whoever proposed it; execution is bounded by `e_max` (§9.4), and a persistently
slow executor is what the level exists for (§4.4 item 2). A discarded execution counts only its
elapsed time, never more than it would once finished.

*Why liveness does not depend on it.* Within a height the level still grows by one per failed
view (`level(h, v) = start(h) + v`), so the view timer reaches `T_req` after at most `k*` failed
views whatever `start(h)` is (§8.2 L3). The start level only shortens that path at later heights
when the symptom persists — an execution, or a committing view, longer than half the base timer —
and prices failed views; it rises by one per such height until `T(start)/2` covers it (F15).
Payload progress does depend on it when non-empty blocks take longer than
`T(start + empty_after_views − 1)`: such a height commits `EMPTY` (L3), and its discarded
executions raise the start level by one until a payload view outlasts them (F15 variant 5).

### 9.3 Defaults (1 s target block time)

| Parameter | Kind | n = 4 | n ≈ 20 (≤ 31) |
|---|---|---|---|
| `block_time` | chain | 1000 ms | 1000 ms |
| `idle_block_interval` | chain (per instance; DS/lane SHOULD use ≥ 30 000 ms) | 5000 ms | 5000 ms |
| `empty_after_views` | chain | 2 | 2 |
| `demotion_window W` | genesis constant per instance (SHOULD be ≥ `4n`) | 128 heights | 128 heights |
| `e_max` / `a_max` | chain | 4000 / 1000 ms | 4000 / 1000 ms |
| `max_block_bytes` | chain | 4 MiB | 4 MiB |
| `build_timeout` | local | 200 ms | 200 ms |
| `T_base` | local | 2000 ms | 3000 ms |
| growth | fixed | 1.5 | 1.5 |
| `T_max` | local (effective ≥ `T_req`, §9.4) | 30 000 ms | 30 000 ms |
| `level_cap` | derived | 7 | 6 |
| `start_cap` / `decay_after` | local | 4 / 8 heights | 4 / 8 heights |
| `φ` | fixed | 0.5 | 0.5 |
| `rebroadcast_interval` (≤ `T_base/2`) | local | 500 ms | 1000 ms |
| `status_keepalive` | local | 5000 ms | 5000 ms |
| `fetch_retry` | local | 250 ms | 500 ms |
| `sync_batch` / `sync_retry` | local | 64 / 1000 ms | 64 / 1000 ms |
| `sync_max_bytes` (≥ `max_block_bytes` + 64 KiB) | local | 16 MiB | 16 MiB |
| `max_observers` | local | 64 | 64 |

View-0 timelines at n = 4, start level 0. **Under load:** the leader proposes about
`block_time − latency_ewma` after the parent commit; stage 1/2 fire a few `t_retx` after a member
became ready; the view times out at `t_prop + 2.0 s`. **Idle:** the heartbeat is proposed at
`t_enter + 5 s`. A crashed leader is detected at `t_enter + 5.2 s + 2.0 s`, idle or under load
(§15.2). `T(L)` for n = 4: 2.0, 3.0, 4.5,
6.75, 10.1, 15.2, 22.8, 30 s. Chain parameters MUST be identical on all validators (they come from
committed state, §10); local ones only change performance.

### 9.4 Config validation

`T_req(nominal)` is the §8.2 L3 formula with `Δ = Δ_nom = 500 ms`, `δ = δ_nom = 50 ms`,
`σ = rebroadcast_interval + Δ_nom`, `F = ⌈log2(f+1)⌉ · fetch_retry` for the committee size, and the
chain's `a_max`, `e_max`. Defaults: n = 4 → 16.1 s; n = 22 → 19.6 s; both ≤ 30 s.
`Core::new` rejects a local config if `T_max < T_req(nominal)` for the initial configurations,
`rebroadcast_interval > T_base / 2`, `sync_batch < 1`,
`sync_max_bytes < max_block_bytes + 64 KiB`, `fetch_retry > rebroadcast_interval`, or
`demotion_window < 1`. The application MUST reject chain parameters with
`block_time > idle_block_interval`, `empty_after_views < 1`, or `max_block_bytes` above the
transport limit (§12.3 O10). `Δ_nom` presumes that a leader sends its `n − 1` payload copies of
`max_block_bytes` within about `Δ_nom` (at `n = 22` and 4 MiB: ≈ 1.4 Gbit/s of uplink); the
application SHOULD choose `max_block_bytes ≤ Δ_nom · uplink / (n − 1)` for the validators' real
uplink, or operators raise `T_max`. When a
later committed configuration raises `T_req`, the core uses `T_max_eff = max(T_max, T_req(nominal))`
and reports `LocalFault(ConfigTooTight)` instead of halting.

---

## 10. Validator set changes, chain parameters and epochs

1. **Rule.** A *height configuration* is `HeightConfig { committee: C_h, params: ChainParams }`.
   For the first two consensus heights after genesis height `g`, the configuration is the genesis
   one. For `h > g + 2`, it is what the application state `S_{h−2}` schedules for height `h`
   (lag = 2, a protocol constant). The application reports it in
   `BlockApplied{height: h−2, config_after_next}`, and it is committed in `R_{h−2}` (§4.1), so all
   honest nodes agree on it. Chain parameters (block time, idle interval, `e_max`, `a_max`,
   `max_block_bytes`, `empty_after_views`, epoch length) change only this way. The demotion window
   `W` is not a chain parameter: it is fixed at genesis (§2.1), because nodes that pruned headers
   under an old `W` could not compute `D_h` under a new one.
2. **Why lag 2, and the apply bound.** The core enters `h+1` on the CommitQC of `h` without waiting
   for `B_h` to be applied. It needs the configuration of `h+1` = f(`S_{h−1}`), known once
   `applied ≥ h−1`. Entering `h+1` therefore requires `applied ≥ h − 1`: consensus runs at most two
   heights ahead of *durable* apply (`BlockApplied` is reported only after the block and its
   CommitQC are durable, O3). Consequently a safety record never describes a height above the
   durable block-store tip `+ 2` unless the block store lost data (§7.4 R5, R6; R2 does not rely
   on this, Lemma 0). Execution of `h+1`
   does not wait for apply (it uses the certified speculative post-state of `h`, §4.1), so storage
   latency is off the vote path.
3. **Epochs** are an application policy: e.g. NPoS elects at epoch `e` and schedules the new
   committee for the first height of epoch `e+1`. The core accepts a change at any height; instances
   whose certificates other instances verify change committees only at epoch starts (§11.7).
   **Key rotation** replaces the old key by the new one at a single height (the new key's proof
   of possession is checked by the application before scheduling). The application MUST NOT
   schedule a committee containing two keys of one validator (a make-before-break rotation would
   make the core sign with neither, §7.4 Keys). The node configures both keys across the change
   height; each key has its own safety record and the signing key at `h` is chosen by membership
   in `C_h` (§7.4), so no record is deleted and nothing halts; afterwards the old key is retired
   and its record is kept for good (§7.4 Keys).
4. **First round of a new committee.** Height `h_new` (first height of `C_new`) is ordinary: its
   parent CommitQC was produced and is verified by `C_{h_new − 1}` (old committee); its topology
   uses `perm` of `C_new` and `D_{h_new}` (skipped leaders of the window that are members of
   `C_new`); view 0 needs no justification. Members of `C_new` must
   have state `≥ h_new − 2` (they sync as observers before activation). Proposals and CommitQCs of
   `h_new − 1` are also sent to `C_new \ C_old` (`recipients`, §6.0), so joiners have the body and
   certificate of their parent. The application SHOULD schedule changes ≥ 1 epoch ahead. Removed
   members become observers from `h_new` (they never sign for heights where they are not in
   `C_h`) and keep serving sync and bodies.
5. **Sync across boundaries** verifies each CommitQC against its own height's committee, learned
   from applying the blocks two heights earlier; sync entries wait (bounded) until that is known.
6. **Other instances.** A dataspace or lane committee chosen by the global chain is still a
   function of *that instance's* committed state: the DS/lane application carries the global
   decision (with a global CommitQC proof) into its own block, and it takes effect two heights
   later (at an epoch start, §11.7). The core never reads another instance.
7. **Long-range note.** Keys of removed members are useless for heights where they are not in
   `C_h`, but a node syncing from far behind can be fed a fork signed by a *later-compromised old*
   committee. Weak-subjectivity checkpoints (a trusted recent `(height, block_hash)` in node
   config) are the application's mitigation; the core only guarantees per-height verification.
8. **Quorum loss.** If more than `f` members of `C_h` are permanently gone, the instance cannot
   commit `h`, and no block can install a replacement committee. Recovery is outside the core: it
   needs an application-defined, governance-certified handover (freeze the instance at a committed
   height `x`; the new committee starts from `x`'s CommitQC; verifiers reject signatures of the old
   committee above `x`). Until then the instance is halted and the global chain aborts its AMX
   transactions at their deadlines. Implementations MUST NOT add a core-level committee override.

---

## Governed NPoS reconfiguration

**Application-layer governance.** This section is not a core rule. The NPoS application applies
these on-chain parameters when it elects validators and schedules committees and chain parameters
for the core through §10.

Evidence retention and penalty scheduling are governed on-chain rather than
read from local node config. The first-release defaults are:

- `SumeragiNposParameters.reconfig.evidence_horizon_blocks = 7200`;
- `SumeragiNposParameters.reconfig.activation_lag_blocks = 1`;
- `SumeragiNposParameters.reconfig.slashing_delay_blocks = 3600`.

The evidence horizon, slashing delay, and epoch length are
immutable after the initial signed installation. The
horizon plus delay may span at most three epochs, matching the fixed
four-roster committed-evidence capacity. Other admitted fields may be governed
through the on-chain parameter path; validators and executor upgrades must
never replace consensus-owned values with local TOML or executor defaults.

A staged mode transition preserves the joint-consensus rule that the outgoing
set authenticates the boundary: `mode_activation_height requires next_mode to be set in the same block`.

---

## 11. Dataspace-local finality and AMX via two-phase commit (application layer)

The core is not involved beyond the exports listed at the end. Participants: the global chain `G`
and dataspaces `D1..Dk`, each a separate core instance with its own committee.

1. **Finality.** A DS block is final when its DS CommitQC exists (DS-local finality). A global
   block is final when its global CommitQC exists. Neither depends on the other.
2. **Begin (G).** An AMX transaction `X` (`x = H(X)`, participant set, global deadline height `d`)
   is first committed in `G` as `Begin{x, participants, d}` at a global height `b < d`. From then on
   `G`'s state knows `x`; a second `Begin` for `x` is rejected.
3. **Prepare (DS).** Each participant `Di` includes `X` with a `G` proof of `Begin` (§11.7) in a
   DS block. DS execution checks the proof and that `x ∉ prepared_Di`, adds `x` to `prepared_Di`,
   and either escrows (locks) the touched state and records `Prepared{x, Di, Yes, effects_hash}`,
   or records `Prepared{x, Di, No}`; the record is committed in `R_Di`. A second inclusion of `x` is
   a rejected transaction. If `Di` already holds a proof of `G`'s decision for `x`, it records `No`
   and escrows nothing.
4. **Relay.** Any node submits to `G` a proof: DS header, DS CommitQC, Merkle proof of the record
   in `R_Di`. `G`'s executor verifies it with the foreign-committee tracker (§11.7). Relaying
   affects liveness only; the validators of each participant SHOULD relay their own `Prepared`
   records (their payload builders include pending proofs).
5. **Decide (G).** `G`'s execution records `Decision{x, Commit}` in the first global block at
   height `≤ d` that holds `Yes` proofs from every participant; `Decision{x, Abort}` on the first
   valid `No` proof, or deterministically in the first global block with height `> d` if no decision
   exists yet (`G` knows `x` from `Begin`, so abort needs no proof). One decision per `x`,
   immutable; later proofs for `x` are ignored.
6. **Settle (DS).** Each `Di` includes the decision proof (global header, global CommitQC, Merkle
   proof of the decision) in a later DS block; DS execution verifies it, checks
   `x ∈ prepared_Di \ settled_Di`, applies or releases the escrow, and adds `x` to `settled_Di`.
   **A DS MUST NOT release a `Yes` escrow by any other means** (no local timeout, no operator
   release). Escrow is bounded in global heights (`G` decides by `d + 1`) but not in wall-clock
   time: it lasts while `G` is halted or while no decision proof is relayed.
7. **Foreign-committee tracking (light client).** An instance `J` whose certificates another
   instance verifies (`G`, and every DS taking part in AMX) changes committees only at epoch starts:
   heights `s` with `(s − g_J − 1) mod E_J == 0`, `E_J = epoch_length` (a chain parameter of `J`).
   `R_{s−2}` commits to `C_{J,s}` (§4.1). A verifier tracks, per foreign instance, the latest epoch
   `e` it has verified with `C_{J,e}`, and also `C_{J,e−1}`. It accepts a foreign CommitQC for height
   `x` only if `epoch(x) ∈ {e − 1, e}` and `verify_commit_qc(I_J, C_{J,epoch(x)}, qc)` holds. To
   advance to `e + 1` it needs a *handoff proof*: the header of height `s − 2` (`s` = first height of
   epoch `e + 1`), its CommitQC verified under `C_{J,e}`, and a Merkle proof of `C_{J,e+1}` in
   `R_{s−2}`. Handoffs are relayed in sequence, one per epoch even if the committee is unchanged.
   Members removed at an epoch start therefore cannot certify any height of a later epoch. A proof
   for an epoch beyond `e` waits for the handoffs; if they do not arrive before `d`, `X` aborts.
8. **Properties.** Atomicity: all participants apply `X` iff `G` recorded `Commit`, `G` records
   `Commit` only with `Yes` from all, and no participant releases a `Yes` escrow without `G`'s
   `Abort` proof. Idempotence: per-`x` prepared and settled sets. Non-blocking: no DS block and no
   global block ever waits for another instance; a slow DS only makes its AMX transactions abort.

**What the core exposes for this (and nothing else):** (a) the instance id in every signing
domain and header; (b) the committed-block output (`CommitBlock{block, commit_qc}`) from which
proofs are built; (c) pure functions `block_hash(&BlockHeader)`, `committee_digest(&Committee)` and
`verify_commit_qc(instance, &Committee, &Qc) -> bool` for foreign-instance light verification.

---

## 12. Sans-IO node interface

### 12.1 API

```rust
pub struct Core { /* §6.0 */ }
impl Core {
    pub fn new(local: LocalParams, init: Init, signers: Vec<Box<dyn Signer>>, crypto: Box<dyn Crypto>,
               now: Millis) -> Result<(Core, Vec<Action>), ConfigError>;  // applies R1–R6 (§7.4)
    pub fn handle(&mut self, now: Millis, event: Event) -> Vec<Action>;
    pub fn next_wakeup(&self) -> Millis;                   // earliest deadline; driver sends Tick
    pub fn status(&self) -> CoreStatus;                    // read-only diagnostics (height, view, level, stage, t_retx, footprint)
}
pub struct Init {
    records: Vec<(PublicKey, RecordState, bool)>, // one per configured or retired key: Present(bytes) | Absent; retired flag
    genesis_height: u64,
    demotion_window: u64,                     // W, fixed at genesis (§2.1)
    nonce: u64,                               // fresh random value for this Init (R2 probe)
    tip: CommittedTip,                        // height t, header, result, CommitQC (None at genesis)
    configs: Vec<(u64, HeightConfig)>,        // for t (unless t = g), t+1 and t+2
    recent_headers: Vec<BlockHeader>,         // last W + 2 committed headers (≤ t)
}
pub struct HeightConfig { committee: Committee, params: ChainParams }
pub enum Event {
    Tick,
    Message { from: PublicKey, msg: WireMessage },           // `from`: authenticated sender (§1.4)
    PayloadBuilt { req: u64, payload: Vec<u8> },             // answer to BuildPayload{req}
    PayloadReady { req: u64 },                               // after answering BuildPayload{req} with EMPTY: an includable transaction arrived (at most once per req)
    Executed { block_hash: Hash32, req: u64, outcome: ExecOutcome }, // Valid(R) | Invalid | Failed(reason) | Cancelled; exactly one per Execute
    BodyAvailable { block: Block },                          // FetchBody satisfied from local storage
    BlockApplied { height: u64, block_hash: Hash32, header: BlockHeader, config_after_next: HeightConfig },
    ApplyDiverged { height: u64, block_hash: Hash32, local_result: Hash32 },
}
pub enum Action {
    PersistSafety(SafetyRecord),                    // durability barrier (O2)
    StoreBody { block: Block },                     // durable body store keyed by (height, block_hash) (O2)
    Send { to: PublicKey, msg: WireMessage },
    Broadcast { to: Vec<PublicKey>, msg: WireMessage }, // recipients resolved by the core, sent in list order
    BuildPayload { req: u64, height: u64, view: u64, max_bytes: u32, exec_budget_ms: u32 }, // the builder peeks; it never removes transactions
    Execute { block: Block, req: u64 },             // against the applied state or the (re)computed post-state of block.parent_hash (O4)
    DiscardExecution { height: u64, keep: Vec<Hash32> }, // cancel (answer `Cancelled`) and drop post-states of other blocks at `height`
    CommitBlock { block: Block, commit_qc: Qc },
    FetchBody { height: u64, block_hash: Hash32, peers: Vec<PublicKey> }, // local stores first, then BlockRequest
    ServeBody { to: PublicKey, height: u64, block_hash: Hash32 },        // answer with BlockResponse if held
    ServeBlocks { to: PublicKey, from_height: u64, max_count: u16, max_bytes: u32 },
    PayloadRejected { height: u64, view: u64, block_hash: Hash32 },     // builder quarantines those transactions
    ReportEvidence(Evidence),
    LocalFault(LocalFault),                         // ExecutionMismatch, ExecutorFailed, RecordMissing, ... (telemetry)
    Halt(HaltReason),
}
pub trait Signer { fn public_key(&self) -> &PublicKey; fn sign(&self, preimage: &[u8]) -> Signature; } // deterministic
pub trait Crypto {                                  // pure; BLS in production, fake in the simulator
    fn hash(&self, bytes: &[u8]) -> Hash32;
    fn verify(&self, pk: &PublicKey, msg: &[u8], sig: &Signature) -> bool;
    fn aggregate(&self, sigs: &[Signature]) -> AggregateSignature;
    fn verify_aggregate(&self, pks: &[&PublicKey], msg: &[u8], agg: &AggregateSignature) -> bool;
    fn verify_aggregate_multi(&self, groups: &[(Vec<&PublicKey>, Vec<u8>)], agg: &AggregateSignature) -> bool;
}
```
`Signer::sign` runs inside `handle` and MUST be local and non-blocking (an in-memory key or a
local device with bounded latency); a remote signer would block the core and its timers. It MUST
be deterministic (same key and preimage → same bytes; BLS signatures are): the core re-creates
recorded messages after a restart by signing their recorded preimages again (§6.10 rule 0, §7.4).
The enums above are written unboxed for readability; to satisfy the workspace lint
`variant_size_differences = deny`, variants carrying a `Block`, `Proposal`, `TimeoutCert`, `Qc`,
`Status`, `SafetyRecord`, `HeightConfig`, `BlockHeader` or `Evidence` are boxed (`Box<…>`) in
code; the wire and record encodings are unchanged by boxing.

### 12.2 Driver responsibilities

Transport (authenticated P2P, routing by instance id, per-peer rate limits, traffic classes),
timer (`Tick` at `next_wakeup()`), durable safety records (one file per instance and key,
retired keys' included, never dropped) with the record-provenance rules, installation log and
store id of §7.4, a fresh random `Init.nonce` per start,
durable body store, block store (Kura), executor with a speculative-state cache keyed by block
hash (chained on certified parents), payload builder (queue; `BuildPayload` only peeks,
transactions leave the queue when a block containing them is applied; `PayloadReady{req}` at most
once after an `EMPTY` answer to `req`; on `PayloadRejected` it quarantines only transactions that
make a block `Invalid` on their own, §4.2), serving `ServeBlocks`/`ServeBody`/`FetchBody`
(`ServeBlocks` may answer with an empty `SyncResponse`), reporting executor panics and I/O errors
as `Failed`, and the applied header in `BlockApplied`. One core instance per consensus instance; instances share nothing inside the core and
are isolated by the driver (O9).

### 12.3 Ordering and scheduling guarantees the driver MUST honour

- **O1** Actions of one `handle` call are executed in order.
- **O2 Persist before effect.** After a `PersistSafety`, every later action (this or any later
  call) except `Execute`, `DiscardExecution`, `BuildPayload`, `PayloadRejected`, `LocalFault` and
  `StoreBody` takes effect only after that record is durable. In particular `Send`, `Broadcast`,
  `CommitBlock` (and anything later served from the block store), `ServeBlocks`, `ServeBody`,
  `FetchBody` and `ReportEvidence` wait. A `PersistSafety` counts as durable only after every
  `StoreBody` emitted before it is durable. Records replace each other atomically; a failed write
  is retried (the instance is silent meanwhile), never skipped.
- **O3 Commit order.** `CommitBlock` actions are applied strictly in height order. The block and
  its CommitQC are durable in the block store before `BlockApplied` is reported, and
  `BlockApplied` events are reported in height order, one per height, each with the header of the
  block applied. To apply, the driver uses its cached post-state of exactly that block hash if it
  holds one whose commitment equals `commit_qc.result`; otherwise it executes the block, reusing
  an execution of that block still in flight instead of starting another; if the resulting
  commitment differs from `commit_qc.result` it reports `ApplyDiverged` instead. A missing cache
  entry is never reported as divergence.
- **O4 Execution.** Execution runs off the core thread; results may arrive late or out of order;
  the most recent `Execute` is served first. The driver answers **every** `Execute` exactly once
  with its `req`: `Valid(R)`, `Invalid`, `Failed`, or `Cancelled` when a later `DiscardExecution`
  of that height left the block out of `keep` before the work finished. On a `DiscardExecution` it
  also drops the post-states of the blocks not kept (a finished execution's `Executed` may already
  be in flight; the core ignores it). It keeps the post-state of every other `Valid` execution
  until that block is applied or discarded. A post-state it needs but no longer holds (evicted
  under memory pressure, lost on restart) is recomputed: for an `Execute` whose parent is
  certified but not yet applied, by re-executing the parent from its stored body or by waiting
  for the parent's apply (bounded by `A_max`, which §8.2 L3 already budgets); for `CommitBlock`,
  as in O3. A missing cache entry is never answered `Failed` or `Invalid`.
- **O5 Event priority.** The driver delivers events to `handle` in this priority: `Tick`, then
  `Executed`/`BlockApplied`/`ApplyDiverged`/`PayloadBuilt`/`PayloadReady`/`BodyAvailable`, then
  control-class messages, then other messages. `Tick` is delivered within ε of `next_wakeup()`
  regardless of execution, storage or network load; the core never waits for I/O.
- **O6** Network messages may be dropped under load (lowest class first). Events other than
  `Message` MUST NOT be dropped. Ingress is bounded per `(peer, class)`, keeping the latest
  `Status` per peer and dropping the oldest message otherwise. Within a class the driver SHOULD
  serve peers round-robin, so one flooding peer cannot starve the others' control traffic.
- **O7** The driver never delivers the node's own messages back to it.
- **O8 Traffic classes.** *Control*: `Vote`, `Qc`, `Timeout`, `Tc`, `Status`, `SyncRequest`,
  `BlockRequest`, and `Proposal` without payload. *Proposal*: `Proposal` with payload, and
  `BlockResponse` for the current height. *Bulk*: `SyncResponse` and other `BlockResponse`s.
  Egress and ingress use strict priority control > proposal > bulk, with a guaranteed minimum
  share (e.g. 10 %) for bulk so sync always progresses.
- **O9 Instance isolation.** Each instance has its own event loop, its own record files and O2
  barrier, its own ingress queues and network quotas. A stalled or flooding instance never delays
  another instance's events, persistence or messages.
- **O10 Frame limit.** The transport accepts messages up to `max_block_bytes + 64 KiB` and
  `sync_max_bytes`; the driver checks this at startup against the committed chain parameters.

### 12.4 Config

`LocalParams { t_base, t_max, start_cap, decay_after, rebroadcast_interval, status_keepalive,
build_timeout, fetch_retry, sync_batch, sync_retry, sync_max_bytes, max_observers }`.
`ChainParams { block_time, idle_block_interval, e_max, a_max, max_block_bytes,
empty_after_views, epoch_length }` arrive in `HeightConfig` (§10.1); `demotion_window` arrives in
`Init` (genesis constant). Validation: §9.4.

### 12.5 Error policy

The core halts an instance (`Action::Halt`, then emits nothing but serving) **only** on:
1. a corrupt local safety record, a recorded parent CommitQC that does not verify (R1, R5), or a
   committed body that does not extend the committed parent (§6.9 rule 6, local corruption);
2. a detected safety violation (§7.6: a valid CommitQC conflicting with a committed block);
3. `ApplyDiverged` — the local state or executor disagrees with a certified result, so the node
   cannot follow the agreed chain; it needs an operator restore (state snapshot or re-sync);
4. `DriverAnomaly` — a driver contract violation the core detects (a `BlockApplied` out of order,
   whose header does not hash to its `block_hash`, or whose `block_hash` is not the block the core
   committed at that height, §6.13).

Everything else is drop / evidence / abstain / backoff: malformed or badly signed messages are
dropped; signed misbehaviour becomes `ReportEvidence`; proposals with signed-content defects, and
a leader's twin proposals in the current view, cause evidence and an early timeout; blocks that
execute to `Invalid` cause an early timeout and
`PayloadRejected`, never evidence; unattributable defects (bad payload bytes, tampered copies) are
dropped silently; executor failures and certified-block mismatches are local faults; a missing
record or a block store behind the record makes the key abstain (R2, R6); storage write failures are retried by the
driver with backoff (the node is merely slow); overload drops network messages (rebroadcast
recovers). The core never panics on any input (the decoder and every handler are fuzzed); other
instances keep running when one halts.

### 12.6 Size budget

The core crate (everything except `sim/` and tests) MUST stay at or below 8 000 lines of Rust
(non-blank, non-comment); CI prints the count per module and fails above the budget. Planned
split (a budget, not an estimate):

| Module | Content | Budget |
|---|---|---|
| `types` | ids, keys, committee, `HeightConfig`, Norito derives | 600 |
| `topology` | `perm`, `D_h`, slot substitution, roles, stage hint | 300 |
| `message` | preimages, digests, `verify_qc`/`verify_tc`, formation, evidence types | 800 |
| `safety` | record, `persist()`, restart R1–R6 and composition, probe/anchoring, key selection | 700 |
| `pacemaker` | levels, anchor, deadlines, start adaptation, EWMAs | 400 |
| `core` | handlers §6.1–§6.8, §6.10–§6.13 | 3 000 |
| `sync` | sync, wants, serving (§6.9) | 700 |
| `api` | `Event`, `Action`, traits, config validation, `status()` | 600 |
| total | | 7 100 |

A new mechanism that would exceed the budget replaces something or goes to §14.

---
## 13. Verification plan (deterministic simulator, scripted tests, driver conformance)

### 13.1 Harness

- Single-threaded discrete-event scheduler in virtual milliseconds; every random choice comes
  from one seeded PRNG; a failing run is reproduced from `(scenario, seed)`. Seed count via
  `SUMERAGI_SIM_SEEDS` (CI default 200 per scenario; nightly 10 000).
- Runs the **unmodified** `Core` (no simulation branches in protocol code) with a fake driver per
  node honouring §12.3: network model (per-link delay distribution, bandwidth, loss, duplication,
  reordering, asymmetric partitions, delay spikes); per-node clock offset and drift; per-node
  single-threaded event queue with the O5 priority lanes and O8 classes, and a virtual CPU cost
  per pairing/verification; per-node safety-record files, body store, block store and executor
  cache whose writes become durable after a latency and are **all** lost if the node crashes first;
  a deterministic executor `R = H(parent_R ‖ payload)` with per-node latency, optional poison
  payloads, injected `Failed` outcomes and an optional divergent node, which honours O4 exactly
  (answers every `Execute` once, `Cancelled` for discarded work, possibly after a `Valid` for the
  same block is already in flight; a discarded running job either finishes first or, per node,
  is aborted and answered at once) and, in F15, evicts cached post-states at random and slows
  non-empty payloads beyond the builder's estimate; a payload
  builder with a per-node transaction queue (clients may submit to any subset of nodes), request
  ids, peek semantics, `PayloadReady{req}` and per-transaction quarantine; a record store with the
  installation log and store id of §7.4 (initial record only at the installation event and never
  over an existing file, `Absent` otherwise; the key store, with its log, can be restored from an
  older snapshot, and the record store can be replaced by an empty one);
  `Init.nonce` drawn from the seeded PRNG; deterministic fake aggregate crypto that records the
  provenance `(node, key, preimage)` of every honest signature.
- Crash points: before/after every action and at every I/O completion (in particular between
  `PersistSafety` and its durability, between local QC formation and durability, and inside apply).
  Restart rebuilds `Init` from the fake stores.
- Byzantine nodes run scripted strategies with their own keys only; the network adversary is
  adaptive (sees all traffic, delays/drops before heal) and may relay, strip or corrupt unsigned
  parts of messages. The simulator MAY pick which identities are Byzantine after computing the
  deterministic topology of a target height (static corruption with hindsight), which is what
  exposes collector- and leader-position attacks.
- Several instances can run in one world (global + DS) with overlapping committee keys.
- **Scripted deterministic tests** drive 1–4 cores by hand (explicit event sequences, harness-held
  keys when a test needs certificates that more than `f` members signed). Every mutation in §13.4
  has one; the randomized scenarios are an additional layer, not the acceptance criterion.

### 13.2 Oracles (checked after every event; failure prints seed and trace)

| Id | Oracle |
|---|---|
| O-AGR | Agreement and prefix: per instance and height, all honest committed `(bh, R)` are equal; each honest chain is contiguous. |
| O-VAL | Validity: every honest-committed block links to the committed parent, its payload satisfies `body_ok` and is the one the proposer built, its `R` equals the reference execution, its proposer is `L(h, origin_view)` by ground-truth topology. |
| O-SIGN | Honest sign-once across restarts and record loss, per key (provenance log; probe echoes, which have no sign-once rule, are excluded): no two different preimages of one kind at `(I, h, v)`; no Prepare/Commit at a view `≤` a timeout view the key already signed at `h`; no timeout at `(h, w)` carrying `hq < v` after the key signed a Commit at `(h, v)`, `v ≤ w` (Lemma 2). |
| O-PBS | Persist-before-effect: whenever an honest signature other than a probe echo leaves a node in any form (message, certificate, block served from the block store, evidence), the node's *durable* record covers it (§7.4). |
| O-CERT | Certificate integrity: every QC/TC an honest core accepts is ground-truth valid: `≥ q_h` distinct members of `C_h`, every signature genuinely produced for exactly that preimage and instance, TC `high_pqc` equal to the true maximum. |
| O-LIVE | After heal time `t_g` (partitions healed, loss at the scenario's post-heal level, `≥ q` honest running, where a node with an unanchored key counts as faulty until it anchors and a node whose key abstains at a height counts as faulty at that height, §7.4 R2, R6), every honest running node commits a new height by `t_g + B_live` (§8.2) and again within every later window of `B_live`. |
| O-PERF | The scenario's performance bounds P1–P6 (§8.2) hold after `t_g`. |
| O-CQ | Chain quality: in scenarios without crashes, over every window of `4n` committed heights that starts at least `W` heights after both `t_g` and the last height whose `skipped_leaders` names an honest member (honest demotions pass slots to successors, which may be Byzantine), the share of blocks proposed by honest members is ≥ `(n − f)/n − 0.05`. |
| O-TXP | Transaction progress: with a poison transaction present, every non-poison transaction submitted after `t_g` commits within `B_live`. |
| O-MEM | Bounded memory and storage: after every event, `status().footprint` of each core and each fake body store is within the §8.4 bounds computed from `n` and the config. |
| O-HALT | No honest halt, except at a node where the scenario injected record corruption or a divergent executor. |
| O-EVID | Evidence soundness: honest nodes never report evidence against honest nodes. |
| O-FAULT | Local-fault soundness: an honest node emits `LocalFault(ExecutorFailed)` or `LocalFault(ExecutionMismatch)` only where the scenario injected executor failures or divergence; cancelled, discarded or evicted executions never surface as faults, `Failed` or `ApplyDiverged`. |
| O-AMX | (F31) Atomicity: a toy AMX transaction is applied by all participants or by none; an escrow is released only with a `G` decision proof; settlement follows within `B_live` of `G` and the DSes being live. |

### 13.3 Fault scenarios (each over `n ∈ {1, 4, 5, 7, 22}` where meaningful)

F1 crashed leaders (≤ f, permanent) · F2 silent proxy tail · F3 withholding proxy tail (forms QCs,
delivers to a chosen subset / one node / nobody) · F4 silent or slow set-A members · F5
equivocating leader (two blocks to two halves) · F6 `f` Byzantine members withholding their votes
(in set A, as proxy tails, or both) while leading correctly · F7 adversarial TC composition
(Byzantine aggregators build TCs from the lowest-`hq` timeouts; network delays lock holders'
timeouts) · F8 split-brain (selective CommitQC delivery + partition of the committer) · F9 loss
10/20/30 %, 5 % duplication, reordering · F10 delay spikes (10× Δ windows, heavy tails) · F11
asymmetric and minority/majority partitions with heal · F12 clock skew ±10 s, drift ±1 % · F13
crash-restart at every action boundary and I/O completion, random and targeted · F14
whole-cluster simultaneous crash/restart with lost non-durable writes · F15 slow executors (one
node 10×; all nodes with `E > T_base`; injected `Failed`; random eviction of cached post-states;
all nodes with `E ∈ [T(1), e_max)` for non-empty blocks only, unforeseen by the builder, and an
executor that aborts discarded work at once — non-empty blocks must commit)
· F16 validator-set change at an epoch
boundary (add, remove, replace a majority) under load and crashes, including joiners that must
fetch their parent's body · F17 node joining 10 000 heights behind; Byzantine sync responders
(forged blocks, invalid QCs, withholding) · F18 floods of votes/timeouts for huge views and
heights, oversize messages · F19 poison payload (executor rejects every block holding a given tx)
→ early timeout, quarantine, `EMPTY` escape · F20 cross-instance replay with shared keys · F21
nondeterministic executor at one honest node (only that node may halt) · F22 idle chain for 10⁵
heights (heartbeat cadence, flat memory) · F23 `n ∉ {3f+1}` (5, 6, 8) with `f` Byzantine and
partitions · F24 injected safety-record corruption (that node halts, others continue); record
deletion (that key is unanchored until its probe succeeds); record deletion together with Kura
tail loss of up to 40 heights (whole-volume restore that excludes records) under the F8 / MS10a
adversary; key reinstallation (all instances `Absent`); a dataspace instance created after key
installation, whose record is later deleted; the key store (with its installation log) restored
from a backup older than a dataspace instance, once with that instance's record deleted and once
onto a new, empty record store (both → `Absent` for that instance, never an initial record, and
the existing records of other instances stay `Present`); probe echoes forged, replayed or
relayed by Byzantine peers. A node whose record was deleted counts as one of the `f` faults until
it anchors: no F24 script pairs a deletion with `f` other members that ignore probes (Byzantine,
crashed or partitioned from the prober; the Byzantine members of the F8 / MS10a adversary answer
probes), and O-LIVE counts that node as faulty until it anchors · F25 relay tampering: payload stripped or corrupted, copies from
non-members, duplicates with the same `bh`, delivered before and after the genuine proposal · F26
Byzantine body responders: forged payloads under genuine headers in `BlockResponse` (solicited and
unsolicited) and `SyncResponse`, answering re-proposal fetches, `pending_apply` fetches and R5 ·
F27 benign storage faults: ENOSPC/EIO on records, bodies and Kura (retried), Kura tail loss, Kura
restore from backup (records are never restored) · F28 key rotation across `h_new` with restarts on
both sides of the change, including one key at R4 and the other at R5 · F29
CPU flood: `Status` with maximal-group TCs, votes for huge views, all at the per-peer rate limit ·
F30 maximum-size blocks with the transport frame limit and sync byte caps · F31 toy AMX
application with `G` and a DS stalled in turn · F32 whole-cluster restart after a PrepareQC was
locked everywhere but before any CommitQC formed, and after a CommitQC formed but before any block
store made it durable · F33 hidden PrepareQC: a Byzantine proxy tail forms PQC(B, v) and delivers
it to one honest node only; the others time out with `hq < v`, discard B's execution on entering
`v + 1`, and must execute B again when TC(v+1) carries the hidden PQC and B is re-proposed ·
F34 apply-bound late entrants: one member's `BlockApplied` delayed by up to `A_max` at every
height, with `f` members crashed; joiners and just-synced nodes entering at the frontier;
executions still pending at commit · F35 local-queue asymmetry: transactions submitted only to
`f + 1` non-leaders of an idle chain, with and without one crashed member (no timer may move) ·
F36 late leaders: up to `f` members send their view-0 proposal `T(start)/2 + 100 ms` after the
honest anchor `t_enter + P(0)`, or the proposal at once without its payload and the body that much
later (never served on request), so that every such view still commits; later one honest member
crashes → no honest start level ever rises, every gap within the P4 leader-turn bound.

### 13.4 Mutations that MUST be detected

Each mutation exists only in a test build (`cfg(sumeragi_mutation = "Mxx")`) and is exactly the
code change given in the table: a named function (§6.0 names; "fake driver" means the
simulator's driver, and the production driver in §13.5) and the change made to it. CI runs a
**meta-check**: for every mutation it builds the mutated crate, runs the named deterministic
test(s) of that row, and fails unless at least one of them fails; the unmutated build must pass
them all. CI additionally runs the listed randomized scenario (≤ 200 seeds) as a second line.
Every named test was checked against the other guards of this specification so that the mutated
rule is the only thing standing between the scenario and the failure; where an in-process guard
masks a rule (noted in the table), the test uses a restart or a nested certificate instead.
Detection needs targeted adversaries: random faults alone did not kill the TC-rule,
`tc-min-highqc` or `forget-timeout` mutations in the previous attempt.

**Safety rules (one mutation per rule of §7.1).**

| Id | Site — change | Named deterministic test (setup → expected) | Randomized scenario | Oracle |
|---|---|---|---|---|
| MS1 | `propose` — rule 0 skipped: after a restart the leader builds a new block although `safety.proposal` holds one at `(h, view)` | `det_s1_leader_restart_no_second_proposal`: n=4, X = L(h,0) proposes B; crash after its `PersistSafety` is durable and the proposal is signed, before the `Broadcast` executes; restart; the builder now returns C → X sends B (identical bytes) or nothing, never C | F13 targeted at leaders | O-SIGN |
| MS2 | `try_prepare` — condition 2 (`safety.prepare` has no entry at `view`) deleted | `det_s2_s27_prepare_once_across_restart`: n=4, harness-signed twins B, B′ of L(h,v); X Prepares B, record durable; crash; restart (R4, record intact); deliver B′ and `Executed(Valid)` for it → no Prepare for B′. (In-process the mutation is masked by §6.2 step 2, which holds only the first twin; hence the restart.) | F5 + F13 | O-SIGN |
| MS3 | `advance_to` keeps `proposal` **and** `try_prepare` checks only that a proposal is held, not `proposal.view == view` (SR3 has two guards; the mutation removes both) | `det_s3_no_prepare_for_old_view_proposal`: n=4, X accepts proposal(B, v), `Execute{B, r1}` pending; harness TC(v) with `high_pqc = PQC(B, v)` (so B's entry survives the discard); X enters v+1; `Executed(r1, Valid(R))` → no Prepare until a proposal of v+1 is accepted, then exactly one Prepare(h, v+1, B, R) | F10 + F33 | direct assertion (the premature Prepare has the legitimate preimage, so O-SIGN cannot see it) |
| MS4 | `try_prepare` and `try_commit` — `timeout_view < view` deleted | `det_s4_fence_across_restart`: X signs timeout(h, w, hq = None), durable; crash; restart (view = w, timeout_view = w); deliver proposal(w) with `Executed(Valid)`, then PQC(w) → no Prepare and no Commit at w | F7+F13: delay PQC(B,v) past honest timeouts, release it while v+1 commits C | O-SIGN, O-AGR |
| MS5 | `try_commit` — `high_pqc.view == view` deleted | `det_s5_no_commit_stale_qc`: X in (h, v+1) via TC(v) with no PrepareQC, no proposal at v+1; deliver PQC(B, v) (lock rises; §6.5 2e does not fire); advance time past `anchor + φ·T` so stage 2 is entered and `try_commit()` runs → no Commit | F8 | O-AGR, direct assertion |
| MS7 | `on_qc` step 2a — `high_pqc = c` unconditionally | `det_s7_lock_monotone`: X locks PQC(B1, v1); deliver W's harness-signed TimeoutVote at view v1 carrying PQC(B0, v0), v0 < v1 (a top-level `Qc` would be cheap-rejected, §6.1 rule 5); X times out → its timeout carries hq = v1 | committer receives an older PQC nested in a timeout | O-SIGN, O-AGR |
| MS8 | `sign_timeout` — carries `None` instead of `high_pqc` | `det_s8_timeout_carries_lock`: lock PQC(v), time out at v → hq = v | MS10a scenario, committer inside the TC | O-SIGN, O-AGR |
| MS9 | `on_tick`, rebroadcast rule 1 — re-signs `TimeoutVote(h, w, high_pqc)` instead of re-sending `mine.timeout` | `det_s9_timeout_resend_exact`: timeout(w, None); then PQC(w−1) arrives (lock rises, no view change); rebroadcast tick → the re-sent timeout is byte-identical (hq = None); a TC X forms then has X's entry with hq = None | F9 + F7 | O-SIGN, O-EVID |
| MS10a | `propose` rule 2 always builds a fresh block **and** `on_proposal` step 6 checks every TC-justified proposal as fresh | `det_s10a_tc_rule_forces_reproposal`: X = L(h, v+1) gets TC(v) with `high_pqc = PQC(B, v)` → X re-proposes B's header unchanged; voter Y gets a harness-signed fresh C justified by that TC → no Prepare, evidence | F8 n=4: Byzantine P(h,v) forms PQC+CQC(B), delivers the CQC only to X, partitions X until TC(v) forms; the leader proposes C | O-AGR |
| MS10b | `on_proposal` step 6 — the TC-rule branch deleted (a TC with `high_pqc = Some(Q)` is checked like a fresh block) | `det_s10b_voter_rejects_tc_violation`: harness-signed fresh proposal C at v whose TC names Q with `C ≠ Q.bh` → no Prepare, `InvalidProposal` evidence, early timeout | n=7, f=2: Byzantine P(h,v) and L(h,v+1) | O-AGR |
| MS11 | `form_tc` — chooses the `q` entries with the lowest `hq` and sets `high_pqc` to the PrepareQC of the lowest non-`None` `hq` | `det_s11_tc_max_hq`: n=4, X receives timeouts for (h, v2) with hq = None, v0, v1 in that order → the TC formed at the third has `high_pqc` of view v1 | B0 locked by Y only, B1 committed by X only, TC(v1) holds Y and Z | O-AGR, O-CERT |
| MS12 | `verify_tc` — the check `high_pqc.view == max(entries.hq)` deleted | `det_s12_tc_verify_rejects_low_high_pqc`: harness TC with max hq = v1 and `high_pqc` = PQC(v0) → rejected, no view change | Byzantine L(h,v+1) attaches an older PQC to honest timeouts | O-CERT, O-AGR |
| MS13 | `quorum(n)` returns `2·f + 1` | `det_s13_quorum_n5`: n=5 (q=4): a QC with 3 valid signers is rejected, and 3 matching votes form no QC | F23 n=5 split | O-CERT, O-AGR |
| MS14 | `verify_qc` — accepts `popcount ≥ q − 1` | `det_s14_qc_popcount`: QC with q−1 valid signers → rejected | Byzantine P + partition | O-CERT, O-AGR |
| MS15 | every caller of `verify_qc` passes the configuration of `tip.height` instead of `C_{qc.height}` | `det_s15_committee_of_height`: one member replaced at h_new; X in round h_new; harness CommitQC(h_new) signed by `q` members of `C_{h_new−1}` including the removed one → rejected, no commit | F16 | O-CERT, O-AGR |
| MS16 | `vote_preimage`, `prop_preimage`, `tmo_preimage` omit `I` | `det_s16_cross_instance_replay`: instances I1, I2 with one committee; a vote and a QC of I2 with the unsigned `instance` field rewritten to I1 (else §6.1 rule 1 drops them) → rejected by I1 | F20 | O-CERT, O-AGR |
| MS17 | `vote_preimage` omits `R` | `det_s17_result_bound`: a genuine PQC(B, R) with `result` rewritten to R′ → rejected | Byzantine P rewrites `result` in its QCs | O-VAL, O-HALT |
| MS18 | `on_proposal` step 1 — verifies `p.sig` under any member of `C_h` instead of `C_h[L(h, w)]` | `det_s18_non_leader_proposal_dropped`: proposal signed by non-leader W with `proposer = idx(L(h, w))`, otherwise valid → dropped silently: no Prepare, no evidence, no timeout | Byzantine non-leaders race proposals every view | O-VAL, O-LIVE |
| MS19 | `on_proposal` steps 5–6 — `parent_qc`, `parent_hash` and `parent_result` checks deleted | `det_s19_wrong_parent_rejected`: signed proposal on a stale parent → evidence, early timeout, no Prepare | Byzantine leader builds on a stale parent | O-VAL, O-AGR |
| MS20 | `body_ok` always returns `true` | `det_s20_forged_body_under_real_header`: n=4, PQC(B,m) exists, Z lacks B, re-proposal without payload, Byzantine W answers Z's fetch with a forged payload → rejected, no execution verdict, no `LocalFault`; genuine body later → Z Prepares | F26 | O-VAL, O-HALT, O-LIVE |
| MS21 | `commit_height` step 1 — no verification when the CommitQC came from a `Status` or a proposal's `parent_qc` | `det_s21_forged_commitqc_via_status`, `det_s21_forged_commitqc_via_parent_qc`: forged CommitQC for h → no commit | Byzantine Status/proposal with a forged CommitQC | O-CERT, O-AGR |
| MS22 | `on_sync_response` — `block_hash(header) == c.block_hash` and the parent checks deleted | `det_s22_sync_forged_block`: a genuine CommitQC paired with a forged block → rejected, no commit | F17 Byzantine responder | O-VAL, O-AGR |
| MS23 | `try_prepare` — `route(vote)` emitted before `persist()` | `det_s23_crash_between_persist_and_durable`: equivocating leader (twins B, B′); X Prepares B; crash before the record is durable; restart; B′ delivered → the network never carries Prepares of X for both B and B′ | F13 targeted | O-PBS, O-SIGN |
| MS24 | fake driver — the O2 barrier holds only `Send`/`Broadcast` | `det_s24_local_cqc_not_exposed_before_durable`: X = P forms CommitQC(B,v) including its undurable Commit, W sync-requests, X crashes → nothing carrying X's signature left X; the two-block commit of the revision-2 review scenario is impossible | F13 at every I/O completion | O-PBS, O-AGR |
| MS25 | `persist` — does not copy `high_pqc` into `safety.lock` | `det_s25_lock_persisted_with_commit`: X Commits at (h,v); the CommitQC reaches only W; X crashes before timing out; restart; X times out → hq = v | F8 + F13 (Commit, CommitQC to one node, crash before timing out) | O-SIGN, O-AGR |
| MS26 | `sign_timeout` — `safety.timeout` not set (forget-timeout) | `det_s26_forget_timeout`: durable timeout at v (hq = None), crash, restart, deliver PQC(B, v) → no Commit at v | targeted restart after timeout while v+1 commits C | O-SIGN, O-AGR |
| MS27 | `try_prepare` — `safety.prepare` not set (forget-prepare) | `det_s2_s27_prepare_once_across_restart` (as MS2) | F5 + F13 churn | O-SIGN |
| MS29 | `propose` — `safety.proposal` not set | `det_s29_forget_proposal`: MS1's scenario → X sends B or nothing, never C | F13 at leaders | O-SIGN |
| MS30 | `restore` step 4 — `high_pqc` not set from the recorded lock | `det_s30_lock_restored`: record with lock PQC(v); restart; time out → hq = v | MS10a scenario with committer restart | O-SIGN, O-AGR |
| MS31 | `restore` — `Absent` classified as R3 (fresh at `t + 1`) | `det_s31_deleted_record_abstains`: n=4, X Commits at (t+1, 0), record deleted, block store intact; restart → X signs nothing at t+1 or t+2; after W, Y, Z (`2f + 1 = 3`) report heights ≤ t'+1 in signed echoes it signs from t'+3 | F24 deletion under the MS10a adversary | O-SIGN, O-AGR |
| MS31b | `restore`, R2 — anchors at once with `abstain_below = t + 3` from the local block-store tip (the revision-3 rule), no probe | `det_s31b_record_and_store_lost`: n=4 (W, X, Y honest, Z harness-Byzantine); at (100, 0) X, Y, Z Prepare B; Z gives PQC(B,0) to X and Y only; X and Y Commit; CommitQC(B,0) reaches Y only; Y's messages are delayed; X's record is deleted and its Kura truncated to 60; restart; X syncs 61..99 from W and enters 100 → X signs nothing at 100 while Y's fresh reply (height 101) is missing; no second block commits at 100 | F24 deletion + Kura tail loss under F8 | O-SIGN, O-AGR |
| MS31c | `on_status` — answers probes while a configured key is unanchored or abstaining | `det_s31c_abstaining_node_does_not_answer`: Y has an `Absent` record (unanchored); X's probe → Y sends no `Status` with `echo`; after Y is anchored and past `abstain_below` → it answers | F24 with two simultaneous deletions | O-SIGN, O-AGR |
| MS31d | `on_status` — counts every `Status` from a member of `C_{t'+2}` as a probe reply for its sender (`echo` not required) | `det_s31d_stale_status_is_not_a_reply`: X's record deleted; a `Status` Y sent before X's crash (height ≤ t'+1) arrives late, Y's fresh reply reports t'+2 → X stays unanchored | F24 + F10 | O-SIGN, O-AGR |
| MS31e | `on_status` step 1 — the echo signature is not verified (a reply counts for the key its `Echo` names) | `det_s31e_echo_signature`: n=4 (W, X, Y honest; Z harness-Byzantine, leader of (t+3, 0)); X Prepared B at (t+3, 0), then lost its record and its Kura above t; W and Y are in round t+3; X restarts at tip t; Z sends X, with X's nonce, echoes that name W and Y, report height t+1 and carry Z's signature, plus its own valid echo → X stays unanchored; X syncs to t+2; Y's valid echo (height t+3) reaches X only relayed by Z, W's directly → X anchors at t' = t+2 (a relayed signed echo counts) and never Prepares Z's twin B′ at (t+3, 0) | F24 with forged, replayed and relayed echoes | O-SIGN, O-AGR |
| MS32a | `restore` step 2 — R5 commits `parent_commit_qc` without verifying it | `det_s32a_r5_forged_parent_qc`: record at t+2 (valid checksum) whose `parent_commit_qc` does not verify under `C_{t+1}` → `Halt(SafetyRecordInconsistent)`, no commit | F26 at R5 | expects `Halt` |
| MS32b | `restore` step 3 — R6 classified as R3 | `det_s32b_store_behind_record`: record at t+5 with a Commit (lock) at (t+5, v); Kura at t; restart → X signs nothing below t+5 and resumes at t+5 with the lock | F27 Kura tail loss | O-SIGN, O-AGR |
| MS32c | `on_body` (§6.9 rule 6) — the parent-link check of the `pending_apply` flush deleted | `det_s32c_r5_block_not_extending_tip`: R5 with a harness-valid CommitQC(t+1) whose block's parent is not the local tip (simulated block-store corruption) → `Halt(SafetyRecordInconsistent)` when the body arrives, no `CommitBlock` | — | expects `Halt` |
| MS33a | fake driver record store — one file per instance for all keys | `det_s33_key_rotation_restart` (a): rotation K1 → K2 at h_new, restarts at h_new − 1 and h_new → each key keeps its own record, no halt, no double signature | F28 | O-SIGN, O-HALT |
| MS33b | `enter_height` — `signer` = the first configured key, regardless of membership in `C_h` | `det_s33_key_rotation_restart` (a): X signs at h_new with K2, never with K1 | F28 | O-SIGN, O-LIVE |
| MS33c | `restore` — keys processed one by one in configuration order, each R4/R5 overwriting the round (no composition) | `det_s33_key_rotation_restart` (b): store tip t, K2 (listed first) with a record at t+2, K1 with a record at t+1 → start in round t+2 from K2's record, exactly one `CommitBlock` for t+1, the tip never moves back | F28 with R4/R5 splits | O-AGR, O-SIGN |
| MS33d | fake driver record store — the store-id comparison deleted (a rolled-back log is trusted: a key it lists as generated on this node gets an initial record for every instance the log lacks) | `det_s33d_installation_log_rollback`: key K generated on X; dataspace instance I created later (initial record, `(I, K)` entry, new store id); X Prepares B at (t+1, 0) for I, block store at t; X's key store is restored from a snapshot taken before I existed and X's record for I is deleted; restart → the driver marks K imported, `Init` carries `Absent` for (I, K), and X does not Prepare the twin B′ at (t+1, 0); variant: the same old key store onto a new, empty record store → `Absent` likewise | F24 (key store restored from an older backup) | O-SIGN, O-AGR, direct assertion on `Init` |
| MS34 | fake driver apply — the commitment comparison skipped | `det_s34_apply_divergence_halts`: X's executor diverges deterministically (cached and re-executed commitments differ from `commit_qc.result`) → `ApplyDiverged` → `Halt` | F21 | O-VAL, O-HALT |
| MS35 | `on_proposal` step 8 — a payload failing `body_ok` is treated as a signed defect (evidence + early timeout) | `det_s35_relay_tamper_no_evidence`: tampered copy before and after the genuine one → no evidence, no timeout, Prepare on the genuine body | F25 | O-EVID, O-PERF |
| MS36a | `on_executed` step 3 — the certified branch (`Invalid` for a certified block, or `Valid(R ≠ expected_R)`) handled like uncertified `Invalid` | `det_s36_certified_mismatch_is_local` (a): re-proposal of certified B; X's executor returns `Invalid` → `LocalFault(ExecutionMismatch)`, no early timeout, no `PayloadRejected`; X still Commit-votes on PQC | F21 | O-FAULT, O-LIVE |
| MS36b | `on_executed` step 2 — `Failed` handled like `Invalid` | `det_s36_certified_mismatch_is_local` (b): uncertified block, executor returns `Failed` → `LocalFault(ExecutorFailed)`, retry, no early timeout, no `PayloadRejected` | F15 with injected `Failed` | O-FAULT, O-LIVE |
| MS37 | `on_qc` step 1 — the §7.6 monitor branch deleted | `det_s37_conflicting_commitqc_halts` (harness keys): conflicting CommitQC for `tip.height` → `Halt(SafetyViolation)` | — | expects `Halt` |
| MS38 | `on_vote` — `pool_insert` before the signature check | `det_s38_forged_votes_never_pooled`: n=4, `q − 1` forged Commit votes under honest signer indices plus one genuine one reach `P` → no CommitQC, no commit, no stage change; the genuine votes still form a QC later | F18 with forged-signature votes at every node | O-CERT, O-AGR |

Removed as equivalent mutants (revision 4, Appendix C): **MS6** (Commit sign-once check deleted)
and **MS28** (Commit not recorded). A second, different Commit in one view is unreachable
whatever the check: `try_commit` signs only over `high_pqc` of the current view, a second
PrepareQC of a locked view is cheap-rejected (§6.1 rule 5) and never replaces the lock (§6.5 2a
strict `>`), and the lock is durable before the Commit leaves and restored after a restart (SR25,
SR30). Revision 4 therefore removed the redundant `commit` field; SR6 is covered by MS5, MS7, MS25
and MS30.

**Liveness rules.**

| Id | Site — change | Named deterministic test (setup → expected) | Randomized scenario | Oracle |
|---|---|---|---|---|
| ML1 | `level` — returns `start(h)` (ignores `v`) | `det_l1_levels_grow`: n=4, every execution takes `1.5·T_base` → the height commits within `k* + 1` views | F15: all executors `1.5·T_base` | O-LIVE |
| ML2 | `on_tick`, rebroadcast rule 1 deleted | `det_l2_lost_timeout_resent`: the first broadcast of every timeout is dropped → TC forms after the re-sends | F9 20 % | O-LIVE |
| ML3 | `try_prepare`/`try_commit` — condition "in set A or stage ≥ 1" replaced by "in set A" | `det_l3_setb_joins_stage1` (a): n=4, a non-P set-A member silent → view 0 commits through set B | n=7: two Byzantine set-A members withhold votes but lead correctly | O-PERF P2, O-LIVE |
| ML4 | stage-2 triggers (a) and (c) deleted (only `anchor + φ·T` remains) | `det_l4_stage2_timing`: silent P → stage 2 at `t_lastvote + 2·t_retx` (asserted time) | F3 | O-PERF P3 |
| ML5a | `on_tick` — vote retransmit deleted | `det_l5_lost_vote_retransmitted` (a): X's Prepare to P dropped once → re-sent at `t_vote + t_retx` | F9 5 % at n=22 | O-PERF P1 |
| ML5b | `on_vote` step 2 (PrepareQC answer) deleted | `det_l5_lost_vote_retransmitted` (b): P holds PQC(v); a retransmitted Prepare of v arrives → P sends the PQC to that voter | F9 5 % at n=22 | O-PERF P1 |
| ML6 | §9.2 decay branch deleted | `det_l6_level_decays`: after `decay_after` fast commits the start level drops by one | F15 transient | O-PERF P1 |
| ML7 | `demoted_set` returns ∅ | `det_l7_demotion_golden`: leader of (h,0) silent, commit at view 1 → from h + 2 it is in `D`, its slot's view-0 leader is its successor, other heights' leaders unchanged | F1 | O-PERF P4 |
| ML8 | `timeout_insert` — join rule deleted | `det_l8_join_f_plus_1`: `f + 1` other members' timeouts at w ≥ view → X signs a timeout at w | F11 | O-LIVE |
| ML9 | `persist` — `high_tc` not copied | `det_l9_restart_after_tc_entry`: n=4, D silent, X alone forms TC(6), enters 7, times out, restarts → all reach 7 and commit | F13 + F11 | O-LIVE |
| ML10 | `on_proposal` step 8 — `StoreBody` omitted | `det_l10_cluster_restart_lock_no_cqc`: PQC(B) locked everywhere, no CQC, whole-cluster restart → B is re-proposed and commits | F32 | O-LIVE |
| ML11 | `on_block_request` — ignores requests for heights ≤ `tip.height` | `det_l11_pending_apply_after_peers_moved_on`: X committed h without the body while peers moved on → X's `BlockRequest` is answered, `pending_apply` drains | F16 joiners; equivocating leader | O-LIVE |
| ML12 | fake driver scheduler — FIFO ingress without the O5 priority lanes | `det_l12_tick_ahead_of_flood`: a flood of `Status` at the rate limit → every `Tick` handled within `ε_tick` | F29 | O-PERF P6, O-LIVE |
| ML13 | `propose` rule 2 never forces `EMPTY` **and** `on_proposal` step 6 omits the `payload_len == 0` rule | `det_l13_empty_after_views`: executor rejects every non-empty payload → view `empty_after_views` commits `EMPTY` | F19 | O-LIVE |
| ML14 | fake builder — ignores `PayloadRejected` | `det_l14_poison_quarantined`: poison transaction proposed once, then never again by that builder; other transactions commit | F19 | O-TXP |
| ML15 | `on_tick`, Status — keepalive cadence only | `det_l15_unsettled_status_rate`: X at view > 0 broadcasts `Status` every `rebroadcast_interval` | F13 + F11 | O-LIVE |
| ML16 | `initial_stage` returns 0 | `det_l16_hint_from_parent_commitqc`: n=7, parent CommitQC with a set-B signer → a set-B member sends its Prepare to `P` as soon as it is ready, before `t_ready + t_retx`; parent CommitQC of set A only → it does not | F6 at n=22, one withholding member, lossless | O-PERF P2 |
| ML17 | `discard_exec` — emits the action but leaves the `exec` entries | `det_l17_hidden_pqc_reexecutes` (below) | F33 | O-LIVE, O-FAULT |
| ML18 | `leader` — `base_{h,v}[i] = perm_C[(h+v+i) mod n]` (views over permutation slots) | `det_l18_f_plus_1_distinct_leaders`: n=4, `D = {B}`, `perm = [A, B, C, D]`, height anchored at B's slot → leaders of views 0 and 1 differ (C, then D) | F1 + F6 with the Byzantine member right after a demoted slot | O-LIVE, O-PERF P4 |
| ML19a | `request_proposal` — clause (a) deleted (no `Status{want_proposal}` after a late entry) | `det_l19_late_entrant_repush`: n=4, D crashed; C's `BlockApplied(h−1)` delayed until 100 ms after proposal(h+1, 0) reached it; E = `exec_budget`; C's `Status` timer phase pinned so that its last awaiting-cadence `Status` reaches the leader A 10 ms before its `want_proposal` `Status` (inside A's per-peer rate-limit window); run with the n=4 and with the n=22 default timings (`T_base`, `rebroadcast_interval`) → C gets the proposal within `2Δ` of entering h+1, view 0 commits, A ∉ `skipped_leaders` | F34 | O-PERF P1, P4 |
| ML19b | `on_status` — the late-entrant re-push deleted (only the interval-gated rebroadcast re-push remains) | `det_l19_late_entrant_repush` | F34 | O-PERF P1, P4 |
| ML19c | `on_status` — the proposal-request step runs after the rate limit (a rate-limited `Status` with `want_proposal` is dropped whole) | `det_l19_late_entrant_repush` (the pinned `Status` phase makes C's request arrive inside the rate-limit window) | F34 | O-PERF P1, P4 |
| ML20 | `commit_height` step 1a deleted (P does not broadcast its CommitQC) | `det_l20_p_broadcasts_commitqc`: n=4, P forms CommitQC(h) → in the same `handle` call it emits `Broadcast(Qc)` to the other members and `C_{h+1} \ C_h`, before `CommitBlock`; L(h+1, 0) enters h+1 one hop later | F9 1 % at n=22 | O-PERF P1 |
| ML21 | `anchor` — the revision-3 `t_tx` term restored (any `PayloadReady` sets `anchor = min(anchor, now + block_time + build_timeout)` in view 0) | `det_l21_local_queue_moves_no_timer`: idle chain, n=4; `PayloadReady` delivered only at the `f + 1` non-leaders B, C (with and without D crashed) → nobody times out before `t_enter + P(0) + T`; view 0 commits the heartbeat; `skipped_leaders` stays empty | F35 | O-PERF P4, O-CQ |
| ML22 | stage 1 trigger (a) — the Commit-phase clause (`t_pqc + t_retx` without CommitQC) deleted | `det_l3_setb_joins_stage1` (b): a set-A member Prepares but withholds its Commit, hint off → set B's Commits reach P by `t_pqc + t_retx + Δ`; commit before stage 2 | F6 with Commit-only withholders | O-PERF P2 |
| ML23 | `commit_height`/`enter_height` — the committed block's pending execution is dropped (`tip.exec_req` not kept, §6.3 step 0 absent) | `det_l23_commit_before_own_execution`: X commits h while `Execute{B_h}` is pending; proposal(h+1) arrives; `Executed(Valid(R_h))` arrives before `BlockApplied(h)` → X emits `Execute{B_{h+1}}` at once | F34 | O-PERF P1 |
| ML24 | `request_proposal` — clause (b) deleted (only late entrants ask for the proposal) | `det_l24_lost_proposal_copy`: n=4, `order_{h,0} = [A, C, B, D]` (A leads, B = P, C in set A), D crashed, hint off; A's proposal copy to C is dropped once; C's keepalive `Status` phase pinned so that none is sent during view 0 → C receives A's and B's stage-2 Prepare broadcast, sends one `Status{want_proposal}` to A, gets the proposal at once and Prepares; view 0 commits | F9 (lost proposal copies) | O-PERF P1, O-LIVE |
| ML25 | `sign_timeout` / `on_commit` — the revision-4.1 raise restored: a timeout signed while the view's proposal is held raises the start level at the next commit | `det_l26_raise_only_on_slow_commit_or_exec`: n=4 (the join case n=7), X in set A; (a) the leader's twin reaches X after X executed and Prepared the first → early timeout, TC, view 1 commits at once → `start = 0`; (b) the valid proposal reaches X but no quorum (2f members) → X times out on its deadline holding it → `start = 0`; (c) as (b) ending by the `f + 1` join → `start = 0`; (d) CommitQC of view 0 arrives after X left it → `start = 0`; (e) view-0 commit exactly `T(0)/2` after the proposal → 0, then `T(0)/2 + 1` → 1; (f) the same at view 1 → 1; (g) heartbeat proposal `idle_block_interval` after the entry, committed 100 ms later → 0; (h) execution `T(0)/2 + 1` in a failed view, view 1's block executed at once and committed at once → 1 | F5 | direct assertion (start level), O-LIVE |
| ML27 | `discard_exec` / `commit_height` — a `Pending` execution that is discarded, or still pending at the commit, records nothing | `det_l27_pending_execution_counts`: n=4, X in set A; (a) views 0 and 1 time out with their executions pending, view 2 commits `EMPTY` executed in 0 ms → `start = 1`; (b) X enters view 1 through a TC carrying PQC(A) with A's execution still pending (kept), CommitQC(0, A) arrives `T(0)/2 + 1` after the request → `start = 1`; (c) control: the execution finished at once, the view failed, view 1 commits → 0 | F15 variant 5 | direct assertion (start level, non-empty blocks) |
| ML28 | `record_exec` — keeps the last execution of the height, not the maximum | `det_l28_exec_duration_is_the_height_maximum` (3.3 s, then 10 ms → raised), `det_l26_raise_only_on_slow_commit_or_exec` (h), `det_l27_pending_execution_counts` (a) | F15 variant 5 | direct assertion (start level, non-empty blocks) |
| ML29 | `commit_height` — `d_c` measured from the anchor `min(t_enter + P(v), t_prop)` (the E40 rule) | `det_l29_late_leader_does_not_raise`: n=4, X in set B; (a) the view-0 proposal arrives `T(0)/2 + 100` after `t_enter + P(0)`, commit 50 ms later → `start = 0`; (b) the same at view 1 → 0; (c) a payload-less proposal at once, its body `T(0)/2 + 100` later → 0; (d) X holds A from the start, CommitQC of its twin B `T(0)/2 + 100` later → 0; (e) control: body at once, commit `T(0)/2 + 1` later → 1 | F36 | direct assertion (start level), O-PERF P4 per gap |
| ML30 | `commit_height` — `d_c` measured from `t_prop` of any held proposal (a late body or a twin counts) | `det_l29_late_leader_does_not_raise` (c), (d) | F36 (body variant) | direct assertion (start level) |
| ML26 | `on_proposal` step 2 — no early timeout on proven leader equivocation (evidence only) | `det_l25_equivocating_leader_early_timeout`: n=4, X in set A; twin B of the held A (both signed by `L(h, 0)`), with and without X's Prepare of A first → `ProposalEquivocation` evidence and exactly one timeout at view 0 (`hq = None`); no further vote at view 0, even with PQC(A); the twin again → nothing; `q − 1` other timeouts → view 1 at once; not equivocation (no evidence, no timeout): a payload-stripped copy of A, B signed by another member, B carrying A's signature, twins of view 0 after X entered view 1 | F5 | direct assertion, O-LIVE |

Shared tests: `det_s2_s27_prepare_once_across_restart` kills MS2 and MS27;
`det_s33_key_rotation_restart` (a) kills MS33a and MS33b, (b) kills MS33c;
`det_s36_certified_mismatch_is_local` (a) kills MS36a, (b) MS36b; `det_l3_setb_joins_stage1` (a)
kills ML3, (b) ML22; `det_l5_lost_vote_retransmitted` (a) kills ML5a, (b) ML5b;
`det_l19_late_entrant_repush` kills ML19a, ML19b and ML19c. `det_l26_raise_only_on_slow_commit_or_exec`
kills ML25 and `det_l25_equivocating_leader_early_timeout` ML26 (the test numbers follow the
order in which the rules were added); `det_l26` (h) and `det_l27` (a) also kill ML28, and
`det_l29` kills ML29 and ML30. Every other test kills exactly the mutation in its row.

`det_l17_hidden_pqc_reexecutes` (n = 4: W, X, Y, Z; X under test; harness keys): (1) X accepts
proposal B of `(h, v)` and emits `Execute{B, req = r1}`; the harness withholds the answer.
(2) The harness forms PQC(B, v) from W, Y, Z and hides it from X; X, W, Y time out with
`hq = None`; X forms TC(v) and enters `v + 1` → `DiscardExecution` without B. (3) The harness
delivers the answer to `r1` — variant a `Cancelled`, variant b `Valid(R)` (finished before the
discard) — and X ignores it (no vote, no `LocalFault`). (4) View `v + 1` fails; TC(v+1) includes
Z's timeout carrying PQC(B, v), so `L(h, v+2)` re-proposes B. Expected: in the `handle` call that
accepts the re-proposal X emits `Execute{B, req = r2}` with `r2 ≠ r1`; on `Valid(R)` for `r2` it
Prepares `(h, v+2, B, R)`; no `LocalFault` at any point. Variant c delivers the answer to `r1`
only after the re-proposal: X still re-executes at once and ignores the stale answer.

Also required: golden vectors for every preimage (including `echo_preimage`, §3.3), `block_hash`, `att_digest`, `committee_digest`,
the permutation, `D_h`, slot substitution (order and leaders for `|D| ∈ {0, 1, f}`, with demoted
slots at the anchor, between views and adjacent to each other) at views 0, 1, 2, and the stage
hint; TC
verification vectors; Norito round-trips of every wire and record type; fuzzing of the decoder and
of `handle` with arbitrary events (no panic, O-MEM holds).

### 13.5 Driver conformance and soak (the runtime, not only the reducer)

- The production driver in `iroha_core` is generic over `Net`, `RecordStore`, `BodyStore`,
  `BlockStore`, `Clock` and `Executor` traits, so the **same driver code** runs in the simulator with
  fake backends (F9, F13, F14, F27, F29, F32 at minimum).
- Conformance oracles on the real driver: O2 (kill the process image at every I/O completion and
  check that nothing externally visible preceded durability), O3 (commit and `BlockApplied` order;
  the applied header in every `BlockApplied`, and its `block_hash` equal to that of the height's
  `CommitBlock` block — the core halts with `DriverAnomaly` otherwise, §6.13, checked with a
  driver fault that reports another block of the same height; `ApplyDiverged` on a forced mismatch, and none when
  the cached post-state was evicted before `CommitBlock`; one execution, not two, when
  `CommitBlock` arrives while that block's execution is in flight), O4 (every `Execute` answered exactly once, `Cancelled` included;
  an `Execute` whose certified parent post-state was evicted is answered with the correct `Valid`,
  never `Failed`), O5/O8 (Tick lateness and control-class latency under bulk and CPU floods), O9
  (a stalled DS instance does not delay global-instance events).
- Before the Taira cutover: a multi-process soak test (n = 4 and n = 22; netem loss 10–30 %,
  delay spikes, random `kill -9`, disk-full injection; 24 h) with O-AGR, O-SIGN, O-LIVE and O-PERF
  computed from node logs. It is a release gate.

---

## 14. Out of scope for the core (tracked as goals) and future optimisations

1. Node integration: driver in `iroha_core` wiring P2P (routing by instance id, traffic classes),
   Kura (blocks + CommitQCs), body store, State (speculative execution chained on certified
   parents, apply, height-config schedule, `R` definition), Queue (payload builder, `PayloadReady`,
   quarantine), safety-record files per key, status endpoint, config surface, §13.5 conformance.
2. Deleting the v2 runtime, its specs, formal models, configs and tests; no "v2" naming remains.
3. Hosting dataspace/lane instances (several cores per node, instance-id derivation, committees,
   O9 isolation).
4. AMX 2PC in executor/Nexus (Begin, prepare/escrow, relay, decision, settle, foreign-committee
   tracker and handoff relaying).
5. Taira reset: fresh genesis with the new parameters and an operator runbook: key generation
   on the node, the installation log, store id and initial records (§7.4 record provenance), the
   per-instance "never signed" assertion for imported keys at genesis, backups that exclude record
   files and the store id (a restored key store makes its keys imported), retired records kept for
   good, and single-height key rotation (§10.3).
6. Evidence → penalties; NPoS election and epoch scheduling of committees; forensics for
   conflicting certificates (cross-view, and same-view ones at an uncommitted height).
7. Payload dissemination optimisations (relay through set A, erasure coding) if blocks outgrow the
   leader's uplink.
8. Weak-subjectivity checkpoints for long-range sync.
9. Push delivery of committed blocks to observers (RPC/indexer nodes) by subscription; `Status`
   pull stays the backstop.
10. Governance-certified committee handover for an instance that lost its quorum (§10.8).
11. **Future optimisations** removed from the first release by the revision-3 simplicity pass
    (Appendix B). None is needed for safety or for P1–P6; each may come back only with its own
    mutation test and within the §12.6 budget:
    a. optimistic (batch) vote verification at the aggregator;
    b. re-chaining of silent set-A members and proxy tails (absence records in headers, with
       attribution to `f + 1` distinct proxy tails), which would remove the recurring P3 cost of a
       silent proxy tail (§8.2);
    c. several outstanding `SyncRequest`s for consecutive ranges (faster catch-up when the
       round trip, not apply, dominates).

---

## 15. Open questions

1. **Committee lag = 2.** Chosen so apply/storage is off the critical path; it means a change
   decided in block `h` binds at `h+2`. Acceptable for NPoS epochs?
2. **Idle cadence and crashed-leader cost.** Default `idle_block_interval` is 5 s for the global
   chain (≈ 17 280 blocks/day when idle), and `PayloadReady` makes the first transaction after an
   idle period propose at once at an idle leader. Since revision 4 no timer depends on a node's own
   queue, so a crashed view-0 leader costs `idle_block_interval + build_timeout + T(start)`
   (7.2 s at the defaults) under load as well. A deterministic alternative that would restore
   ≈ 3.2 s under load: use `P(0) = block_time + build_timeout` (and no idle wait at the leader)
   whenever the parent block is non-empty — a function of committed data, not of any queue. Not
   adopted (not needed for any bound); adopt it if crashed leaders under load matter more than one
   extra empty block after each busy period. Should dataspace and lane instances default to 30 s
   or more?
3. **Predictable schedule.** Leaders and proxy tails are a round-robin over a per-committee
   permutation, so they are known ahead of time. This removes seed grinding and gives slot
   fairness (§2.1), but lets an attacker aim a DoS at upcoming leaders; the cost of that is bounded by
   view change and demotion. An unpredictable alternative (seed = unique BLS signature of the
   previous leader, carried in the header) is possible. Accept predictability?
4. **Demotion parameters.** Only leaders of failed views are demoted, for `W = 128` heights,
   `|D| ≤ f`, with slot substitution. Silent set-A members and proxy tails are not demoted (their
   cost is in §8.2 P2/P3). Confirm.
5. **Block time semantics.** Timestamps are application payload; the core never checks clocks.
   The application must define time monotonicity without local-clock comparisons.
6. **Divergence recovery.** `ApplyDiverged` halts the instance; recovery needs a state-snapshot
   path (not in scope here).
7. **Payload size.** The leader sends the full payload to `n − 1` peers (set A first); at n = 22
   this caps block size by leader bandwidth. Enough for launch?
8. **Epoch length** of AMX-participating instances (handoff cadence for light clients, §11.7).

---

## Appendices A–D. Review logs

The review logs of revisions 2, 3, 4 and 4.1 (Appendices A–D) are kept verbatim in
[`docs/history/2026-09-25/sumeragi-spec-review.md`](../docs/history/2026-09-25/sumeragi-spec-review.md).

---

## Appendix E. As-built reconciliation

This appendix reconciles revision 4.1 with the implementation in `crates/iroha_sumeragi` and its
simulator. Every `// SPEC:` marker in the crate refers to one row below. Rows E1–E8, E36, E40
and E41 changed the normative text above (minimal edits, marked by their section); rows E9–E30,
E37 and E38 record code-level choices where the text leaves room or adds a defensive check,
without changing the protocol; E31–E35, E39 and E42 record simulator and verification
deviations. This file, `specs/sumeragi.md`, is the normative specification that the crate's `§`
references and `// SPEC:` markers resolve against (checked by `crates/iroha_sumeragi/tests/spec.rs`).

### E.1 Rules amended in place

| # | Found by | Rule and change | Where | Code site |
|---|---|---|---|---|
| E1 | simulator, F3 seed 12 (build report bug 1) | `Status` rate limit: a `Status` over the per-peer limit is still used for its `committed_qc` alone if that is at height `≥ h` and above every `committed_qc` height processed from the peer (the §6.1 rule-3 reply carrying the CommitQC a lagging voter lacks often follows the peer's periodic `Status` inside the window; at most two extra verifications per peer and height). Not covered by 4.1, which exempts only echoes and proposal requests. | §6.11 on-`Status` step 3 | `machine::intake::on_status` |
| E2 | simulator, F24 forged record, seed 11 (bug 2) | R4 requires the record's `parent_commit_qc` to be a valid CommitQC of the tip `t` for `(tip.block_hash, tip.result)` (absent only at `g + 1`); otherwise `Halt(SafetyRecordInconsistent)`. A checksum-valid record that contradicts the block store would otherwise make a restarted leader re-send a different proposal (its `parent_qc` comes from the record). 4.1 checks only R5's parent CommitQC. The check runs for the record of every key (configured or retired, signing or not) when the node reaches the record's height (R4 at the restart, R6 later), as step 4 says ("if a key has an R4 record"); the first implementation checked the signing key's record only (found by review). | §7.4 step 4 | `machine::restart::record_matches_tip` (from `machine::round::take_restore`) |
| E3 | simulator, F17 seed 106 (bug 3) | Sync: before a request, buffered entries above a gap in the heights from `h` on are dropped and fetched again (a dropped invalid entry otherwise left a gap that `from_height = max(h, highest buffered + 1)` never requested again, and a full buffer blocked all requests). A request unanswered by `sync_retry` counts as an empty response for dropping an unverified target (a silent source would keep a bogus hint and its requests alive). | §6.9 rule 2 | `machine::sync::keep_buffer_contiguous`, `sync_tick` |
| E4 | simulator, F2 seed 115 (bug 4) | Heartbeat: a `PayloadReady{req}` that reaches the core before the `EMPTY` answer to `BuildPayload{req}` (the builder answered, then a transaction arrived) is kept, and the `EMPTY` answer then requests again at once instead of idling until `idle_block_interval`. Revision 4's request ids narrow the case but do not order the two events. | §6.10 rule 1 | `machine::propose::on_payload_ready` |
| E5 | simulator, F3 seed 4 (this reconciliation) | Unsettled also at stage 2 of the current round. A withholding proxy tail can deliver its PrepareQC to some honest members only; at stage 2 those no longer re-send their Prepare (its phase QC is held), so the others cannot form the PrepareQC from broadcast votes and learn the lock only from `Status` — at the keepalive cadence after revision 4 removed "no commit in `2·rebroadcast_interval`", which broke P3. | §6.11 Status | `machine::timers::unsettled` |
| E6 | simulator, F24 seed 10 (this reconciliation) | R2: anchoring is also checked when `BlockApplied` makes `C_{tip.height+2}` known. At a height entry that configuration is normally not known yet, and later echoes report higher heights (they never lower an entry, so they trigger no check): a node whose lowest replies were recorded while it was behind stayed unanchored forever. | §7.4 R2 | `machine::round::on_block_applied` |
| E7 | test `det_s31e_echo_signature` (this reconciliation) | Probe-table pruning at height entry happens only while `C_{tip.height+2}` is known; otherwise the entries are kept (a lowest reported height stays valid as the tip rises). Pruning against an unknown configuration discarded valid replies. | §6.8 step 5 | `machine::round::prune_probe` |
| E8 | simulator, F9 seeds 34, 274, 284 | P1's per-gap bound is `G_norm + 2·rebroadcast_interval + Δ`: a proposal copy lost to two members the quorum needs is recovered by their proposal request once they see evidence (at worst the stage-2 votes, about `2·t_retx` later) or by the re-push. P1's p99 is taken after a 20-height warm-up (`qc_lat_ewma` starts at 250 ms). O6 SHOULD serve peers round-robin within a class (a flooding peer otherwise starves honest control traffic when the CPU is saturated). | §8.2 P1, §12.3 O6 | `sim::oracle::check_gap`, `sim::driver::Lane` |
| E36 | review (sync buffer poisoning) | Sync responses are bound to requests: a response answers one unanswered request of this node to its source and starts at that request's `from_height` (an empty one answers the source's latest request); other responses, including a second one for the same request, are dropped; buffered entries above a gap are dropped before a response is checked; `Status` candidates are members only (as rule 2 already said). 4.1 processed every response of a source that was ever asked and never replaced a buffered entry, so a Byzantine source asked once could fill the buffer with well-formed entries carrying forged CommitQCs for heights above `h`; they never reached their turn, every honest answer found the buffer full, and a lagging node never caught up. Now a forged entry reaches its turn and is dropped there, at most once per request sent to its source. | §6.9 rules 2–3 | `machine::sync::on_sync_response`, `SyncState::take_request`, `maybe_request_sync` |
| E40 | simulator, final 500-seed sweep: F5 seed 279 (`n = 22`, churn; successive equivocating leaders held every honest node at start levels 3–4 and one height took 26.4 s, failing the post-heal progress check) | (a) §9.2 raise: the start level rises iff `pm.last_exec_ms > T(start)/2` (unchanged) or the committing view was slow here, `t_commit(h) − anchor(h, v_c) > T(start)/2` with `v_c` the committing CommitQC's view, measured only when this node is in `v_c`; the rule "a view timed out on its deadline or by a join while this node held its proposal" (`failed_with_proposal`) and the timeout causes that existed only for it are deleted. That rule let every faulty leader raise the level at each of its turns (equivocate, or send a valid proposal to only `2f` honest members); now a failed view never raises. The rule as first drafted measured from `t_enter(h, v_c)`; the anchor is used instead because at view 0 the entry is followed by the pace or heartbeat wait (`idle_block_interval` = 5 s > `T(0)/2`): measured from the entry, a healthy idle chain in the simulator sat at `start_cap` = 4 and a loaded `n = 4` chain at level 1 (both stay at 0 from the anchor); for `v_c > 0` the two differ by at most `build_timeout`. (b) Early timeout on proven leader equivocation: two different proposals for the current `(h, view)`, both validly signed by `L(h, view)`, make the node sign its timeout at once (not only report evidence), so the view ends one join and one TC later instead of at its deadline. Safe (timeouts are always safe under the fence) and unframeable (the evidence needs two signatures of the leader over different preimages). Tests `det_l25`, `det_l26`; mutations ML25, ML26. | §6.2 step 2, §6.6, §6.8 step 4, §7.1 SR35, §7.5, §8.2 L2–L3, §8.3, §9.2, §13.4 | `pacemaker::Pacemaker::on_commit`, `machine::round::commit_height`, `machine::proposal::check_held`, `machine::timeout::sign_timeout` |
| E41 | adversarial review of E40: (1) every executor 3.0–3.9 s for non-empty blocks at `n = 4` defaults, with an executor that aborts discarded work (O4) — every height committed `EMPTY` at view 2 and the start level stayed 0; (2) a leader whose valid proposal arrives `T(start)/2 + ε` after `t_enter + P(0)` — the view commits, the leader is never demoted, and every honest node rose to `start_cap`, because faulty members lead every `n/f` heights and `decay_after` = 8 never catches up | (a) `pm.last_exec_ms` is the maximum over the height (it was the last value), and an execution still `Pending` when `discard_exec` removes it, or when its block commits, counts with `now − since`, a lower bound that never exceeds what the execution clause counts once the execution finishes. With the failed-view raise gone (E40), nothing else saw an execution that outlasted every payload view; the simulator's non-preemptive executor had hidden it (the next view's execution queued behind the cancelled job, so the committing view looked slow). (b) `d_c = t_commit − t_body`, where `t_body` is when this node first held the committing view's proposal together with its body; `d_c` is defined only if this node holds that view's proposal of the committed block. E40 measured from the anchor, which a late proposal stretches; measuring from `t_prop`, as the review proposed, would still count a body served late and the other twin's commit. §9.2 item (2) corrected: faulty members stretch `d_c` only through certificate delays that the stage ladder bounds, and decay cannot outpace a lever used at every faulty turn. Tests `det_l27`, `det_l28`, `det_l29` and `det_l26` (h), which now executes view 1's block before the commit; mutations ML27–ML30; scenarios F15 variant 5 and F36. | §6.0, §6.2 steps 8–9 and `discard_exec`, §6.3 step 1, §6.8 steps 4–5, §6.12, §8.3, §9.2, §13.3, §13.4 | `pacemaker::Pacemaker::record_exec`, `machine::proposal::{discard_exec, record_pending_exec, maybe_execute}`, `machine::round::commit_height` |

### E.2 Code-level choices (no protocol change)

| # | Topic | As-built choice | Code site |
|---|---|---|---|
| E9 | Signature length | Every signature and aggregate is 96 bytes (BLS12-381 min-pk; the fake scheme uses the same): `tc_digest` places `agg_sig` before a variable-length `opt(..)`, so variable-length aggregates could make two TCs share a digest and a cached verdict. | `types::SIGNATURE_LEN` |
| E10 | Decode bounds | At most 1024 committee members (bitmaps, TC entries, `skipped_leaders`) and 1024 entries per `SyncResponse`; `sync_batch` is validated against the latter. | `types::MAX_COMMITTEE_SIZE`, `message::MAX_SYNC_ENTRIES` |
| E11 | Bitmap bit order | Bit `i` is `(bytes[i / 8] >> (i % 8)) & 1` (least significant bit first). | `types::Bitmap` |
| E12 | Default `epoch_length` | 3 600 heights (one hour at 1 s). | `types::ChainParams::default` |
| E13 | Instance in digests | `qc_digest` / `tc_digest` take `I` from the certificate's own `instance` field (equal to `I` for every verified certificate). | `preimage::qc_digest_preimage` |
| E14 | Local defaults by size | The §9.3 `n = 4` column below 10 members, the `n ≈ 20` column from 10 on. | `api::LocalParams::for_committee_size` |
| E15 | `Init.instance` | `Init` carries the instance id: the core needs `I` before any record, header or certificate exists (a genesis start). | `api::Init` |
| E16 | `Init` validation | One `RecordState` per configured or retired key; a missing state, a signer for a retired key, a configured key without signer or a key listed twice is a `ConfigError`, not a guess. | `machine::restart::local_keys` |
| E17 | R1 consistency | A record that decodes but contradicts itself (certificates of other heights, instances or kinds; a timeout carrying a QC above its view; a `justify` not for `view − 1`) is corrupt (`Halt(SafetyRecordCorrupt)`). | `safety::SafetyRecord::check_consistency` |
| E18 | Key conflict | "Two configured keys in `C_h` sign with neither" applies whether or not a key abstains or is unanchored (a conflict never resolves to signing). Retired keys never count. | `safety::select_signer` |
| E19 | Abstaining members | A member whose key abstains or is unanchored keeps its routing roles (it aggregates votes sent to it as proxy tail and broadcasts the QCs it forms); only signing is suppressed. | `machine::Me` |
| E20 | Oversize payload | A built payload above `max_block_bytes(h)` (a signed defect if proposed) is replaced by `EMPTY`. | `machine::propose::payload_ready` |
| E21 | `T_max_eff` bound | `T_max_eff` is clamped to 2^40 ms, up to which `T(L)` and `level_cap` are exact in integer arithmetic. | `pacemaker::MAX_VIEW_TIMEOUT` |
| E22 | Config validation | Also rejected: zero `t_base`, `rebroadcast_interval`, `status_keepalive`, `fetch_retry`, `sync_retry` (they would busy-loop the timer), zero `build_timeout` (the build deadline would answer every build with `EMPTY` before the builder, O5, so the leader would never propose a transaction) and `sync_batch` above the decode bound. | `pacemaker::validate_local` |
| E23 | `t_retx` clamp | When `φ·T/2 < 50 ms` the clamp bounds cross; the lower bound wins. | `pacemaker::Pacemaker::t_retx` |
| E24 | Initial `latency_ewma` | None: the first sample initialises it, and `pace = block_time` until then. | `pacemaker::Pacemaker::pace` |
| E25 | Start-level "otherwise" | "Otherwise unchanged" keeps both `start` and `fast_streak`; the per-height `last_exec_ms` resets at every commit. | `pacemaker::Pacemaker::on_commit` |
| E26 | Sync buffer size | An entry counts its payload plus 256 bytes per header and 64 per skipped leader (no re-encoding). | `machine::sync::entry_bytes` |
| E27 | Want sources | The peer that relayed a proposal without its body is also a fetch source (bodies are self-verifying). | `machine::proposal::accept_proposal` |
| E28 | Evidence dedup | `reported` has one key per `(kind, view, signer)` for votes and timeouts and per `(kind, view)` for proposal evidence (equivocation and signed defect: `3n + 2` per view); it is capped at `3·(3n + 1)` keys and at 15 keys per accused signer (the leader of the view for proposal evidence: three pool-window views times five kinds), and evidence beyond either cap is dropped (evidence is best effort). The per-signer cap keeps one Byzantine member, which can sign defective proposals for any number of future views (no TC is needed for a signed defect) and timeouts for any future view, from filling the whole cap with keys that are never pruned within the height (found by review); `f` signers hold at most `15f < 3·(3n + 1)` keys. | `machine::Core::report` |
| E29 | Restored own messages | The Prepare and timeout rebuilt by R4 re-enter the own pools through `pool_insert` / `timeout_insert` (they are re-signed, identical bytes), last in `restore`, followed by `try_commit()`; formation may then commit. | `machine::restart::restore_round` |
| E30 | Probe answers | Answered at most once per peer per `rebroadcast_interval`, with a stamp separate from the §6.1 reply limit. | `machine::intake::answer_probe` |
| E37 | `Init.configs` | Only the heights `t` to `t + 2` are accepted, each at most once (a `ConfigError::InvalidInit` otherwise). A configuration above `t + 2` let commits run more than two heights ahead of apply (§10.2), and the next in-order `BlockApplied` then halted the core as a driver anomaly. | `machine::restart::check_init` |
| E38 | Chain parameters | `Core::new` also applies the transport-independent §9.4 chain-parameter rules to the initial configurations (`block_time ≤ idle_block_interval`, `empty_after_views ≥ 1`), and the leader applies the voters' fresh-block rule (§6.2 step 6) to every fresh payload, at view 0 too: with `empty_after_views = 0` from a committed configuration it proposes `EMPTY` instead of a payload every voter reports as a signed defect. | `machine::restart::Core::new`, `machine::propose::propose_fresh` |

### E.3 Simulator and verification

| # | Topic | As-built | Site |
|---|---|---|---|
| E31 | F33 | As literally described F33 needs the network to also delay the lock holder's votes: stage-2 broadcast voting otherwise re-forms the hidden PrepareQC among the honest members. The scenario does so. | `sim::byz::adv_net` |
| E32 | F4, F34 slow executors | A slow executor delays only its Prepare (a Commit needs no execution), so P2's "one gap per `n` heights" is not claimed for it; F4 and F34's pending-execution variant check progress, not P2. | `sim::scenarios::f34` |
| E33 | O3 in-flight reuse | The fake driver reuses a cached post-state whose commitment equals the certified result and otherwise re-executes at once; reusing an execution still in flight is a performance property checked only by the §13.5 production-driver conformance. | `sim::world::apply_block` |
| E34 | O-LIVE accounting | The precondition "`≥ q` honest, running members able to sign" (an unanchored member counts as faulty until it anchors, an abstaining one at its heights) is evaluated every 50 virtual ms; O-LIVE windows restart when it holds again. | `sim::oracle::check_live` |
| E35 | §13.4 meta-check | Every MS/ML row is a `cfg(sumeragi_mutation = "<ID>")` switch, which `build.rs` sets only with the crate feature `mutation-testing` (a production build can never be mutated). The meta-check is `scripts/sumeragi_mutation_gate.py` (named tests, then the scenario at 200 seeds, plus the unmutated build); no CI job runs it yet. It also covers ME1–ME7 (rules E1–E7 with their regression tests) and 18 `MR-*` mutations of revision-4 rules with `det_r4_*` tests (`MR-early-cause`, "an early timeout raises the start level", was removed with the rule it guarded, E40). MS33a is switched in both fake drivers (the simulator's record store and the deterministic-test harness's), and `try_prepare` (and `on_outcome`, before an early timeout) checks `proposal.view == view` explicitly, so MS3 removes these guards together with the proposal reset in `advance_to`. First run: 95 of 100 mutations are killed by their named tests. MS24 and ML14 are killed only by F13 and F19: their named tests run on the test harness, which has no O2 barrier and no builder. MS34, ML5a and ML10 survive; their named tests must be strengthened. O-AMX and the toy AMX application (F31) are not modelled. | `build.rs`, `scripts/sumeragi_mutation_gate.py` |
| E42 | Executor and adversary models of E41 | The fake executor optionally aborts a discarded running job and answers `Cancelled` at once (`Profile::abort_discarded`; O4 allows either), and non-empty payloads may carry extra latency that the builder's `exec_budget` estimate does not foresee (`Profile::exec_nonempty`), both used by F15 variant 5 (F15 seeds now cycle over six variants). `Strategy::LateLeader` delivers view-0 proposals, or their bodies, as late as the view still commits, timed from the attacker's `t_enter` (a simulator-only observation, `Core::round_entered_at`) and the lowest honest start level (F36). | `sim::driver::Executor::discard`, `sim::scenarios::{f15, f36}`, `sim::byz::Strategy::LateLeader` |
| E39 | §12.6 gate, traceability | `tests/spec.rs` counts the non-blank, non-comment lines of the core (everything under `src/` except `sim/`, `machine/tests/`, `testing.rs` and inline `#[cfg(test)] mod tests`, mutation alternates included), prints them per §12.6 module and fails above the 8 000-line MUST; the planned split is reported, not enforced (as built `message`, i.e. `message.rs`, `preimage.rs` and `crypto.rs`, is about 975 of 800 and `safety`, i.e. `safety.rs` and `machine/restart.rs`, about 720 of 700). The same test checks that every `§` reference and every `// SPEC:` marker of the crate resolves in this file. No CI job runs either yet. | `tests/spec.rs` |
