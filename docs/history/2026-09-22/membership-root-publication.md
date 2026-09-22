# Membership roots bound to their original publisher

Transaction membership now has a private authenticated-root capability issued
from the actual committed storage cut. Cold construction borrows the original
writer and visits the current and exact predecessor membership cuts. It commits
domain-separated entrypoint and canonical u64 height hashes, both map roots and
the committed frontier. Older values hidden by the latest block therefore remain
authenticated for replacement. The
capability retains the storage's original publication identity; its constructor
accepts neither a caller-selected root nor a replacement inventory.

Incremental preparation checks that identity before external node access and
uses the admitted transition's borrowed committed-to-staged changes. This covers
ordinary promotion, replacement removal and restoration of shadowed historical
values. An advance carries the old current root as its predecessor; replacement
and repeated publication retain the original predecessor root. It does not scan or clone unrelated history. Foreign or stale baselines
fail even when current touched preimages match. Store failures leave the original
root and State unchanged; caller-owned store/workspace resources remain borrowed.

An unpublished root retains the exact prepared owner's issuer identity. Both
ordinary and detached consuming publishers reject another preparation before
publication and return both original owners. Reacquisition and abort retain the
original issuer. Only successful canonical membership publication exposes the
new committed capability; existing installation and retirement owners retain
their cleanup. An identical repeated publication preserves the existing committed
identity. Pointer identities never enter canonical commitment bytes.

The final candidate passes all 72 transaction-membership controls, including the
10 new ownership/rollback controls. The complete Core library test target builds
without warnings; the non-test Core library check passes with one existing warning
for two unchanged unused State cache helpers. These final checks retain the same
7,556 captured Rust inputs. The final cold test injects every read and write failure
across populated current and predecessor reconstructions. Structural and hygiene
receipts are recorded separately in the same evidence directory.

The earlier candidate passed
all 1,004 previously selected Core controls plus its eight new ownership controls,
with unchanged captured inputs; its full executable is preserved losslessly with
a verified compressed archive. That run predates the explicit predecessor-root
refinement and is not relabeled as final-candidate coverage. The final controls
also distinguish actual histories with equal current membership but different
rollback values and verify every publication mode's exact predecessor root.
Captured commands, source inputs, executables and logs are under
`dist/sumeragi-main-work/generation162-core/`.

This composes the [bounded external update kernel](authenticated-map-node-updates.md)
with the actual membership component publisher. It is not activation of the live
complete-State publisher, a durable node-store implementation, or snapshot/finality
authority. The height preimage still requires an authenticated physical value
record before production lookup can return a height. Retain the capability and
funded store in the original complete State owner, authenticate cold restore,
complete reader retirement and incremental State checkpoints, then migrate both
production Apply checkpoint sites and the retained Validate-to-Apply path.
No per-height cold-rebuild fallback, compatibility decoder or history eviction
has been added. Four/seven-validator fault/restart/final-transaction qualification
and all L1–L6 outcomes remain open.
