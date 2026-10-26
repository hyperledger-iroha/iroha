# Original NFT and RWA owner relations

`grouped_ownership/nfts_rwas.rs` checks each authority over the same sealed
`RawStorageImages` current and undo readers used by committed capture and frozen
capture. NFT checks owner Current/Predecessor, then domain Current/Predecessor;
RWA checks owner, status, then frozen in that same image order. Each phase checks
sources before inverse membership rows. Every index stores one `(group, id) -> ()`
row per membership, so MissingMember and ForeignMember are its only mismatches.
The relation adds no account/domain existence, quantity, reference, time,
controller validity or embedded identifier authority.

Each physical advance is prepaid. Predecessor masks scan every undo key and
retain absent/noop entries; lookup and member scans continue after matches.
Typed equality prepays both complete original operands: domain name+dataspace,
NFT name, RWA hash32, borrowed AccountId controller/member/key geometry,
Option status tag+active Name bytes and bool1. It uses borrowed typed equality,
never point lookup, Ord, parsing, surrogate hashes or reconstructed production
maps. Work refusal remains local preparation refusal, separate from physical
allocation refusal and index inconsistency.

For a no-undo singleton group the two-image cost is `8+8*ID+8*group`.
The original fixture has domain16, NFT ID19, RWA ID48, Single Ed25519 account34,
None status1 and bool1: NFT `432+288=720`, RWA `664+400+400=1464`; active
Name6 status7 gives RWA1512. A Name6 absent NFT undo adds `4*44`, giving896;
an absent RWA undo adds `6*99`, giving2058. Local per-row schedules4320/4608
refer only to singleton/no-undo geometry with two domain components63, NFT
Name63 or RWA Some(Name63), and Single Ed25519. They are not worst-case bounds,
name or physical limits, numeric gas, or ledger validity rules. Larger retained
names, controllers, memberships and undo images can require more admitted work.

Committed capture materializes all three NFT or four RWA native currentness
Results before propagation. It holds the actual row encoder Result and original
readers, observes their identities, drops those readers and applies the final
State generation fence before returning either output or refusal. Frozen capture
retains the actual State World fields, verifies every exact target and equal
Ordinary/Replace mode, checks the same sole relation, and encodes original
current rows with the original State allocation pool. No refreshed owner or
parallel allocation pool is introduced; charged output backing lives through
its final owner drop. Existing16MiB test pools and LeafLimits are unchanged.

The two checked frozen adapters change catalog coverage only when integrated:
217 total,192 raw,14 checked and11 missing. These scoped relations do not
complete State authority, publication or recovery, Kura, finality, private proof,
joint commitment or release qualification. TODO: join every remaining original
cell, history and owner to the sole complete StatePublication and Kura path.
