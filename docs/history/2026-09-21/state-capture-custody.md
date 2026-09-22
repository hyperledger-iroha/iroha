# State capture custody

The checkpoint154 controls reproduce premature notification across the State
boundary: World parameters can notify while State topology remains locked, and
the first runtime cell can notify while the last runtime cell remains locked.
All three original controls failed before the change, using ordinary and
replacement writers and their actual release observations. They establish the
ordering defect, not a measured production deadlock by themselves.

`PreparedCarrier` now retains its original fields through journal admission and
fallible archive projections. Its Drop releases World, all four runtime cells
and transaction membership before destroying their payloads. Admission refusal
still returns the original retryable carrier; admission panic unwinds through
the same aggregate owner and preserves native mutex poison.

World exposes an opaque capture generated from its sole field inventory.
State installs that original owner, the four runtime capture slots, membership
and the already detached hash generation before any fallible capture. Each
slot retains its original attached, prepared or captured phase, including
notifications. Failure releases every sibling before cleanup. Success finishes
every physical capture before materializing journals or delivering a callback.
No second World inventory, extra State allocation or compatibility path is added.

Membership preparation and standalone runtime/World capture use the same
respective kernels. Terminal release revokes read, mutation and publication
authority; it cannot manufacture a detached journal. Prepared membership checks
its original identity before changing live state.

## Evidence and limits

Development builds, original failing controls and current validation artifacts
are retained in `dist/sumeragi-main-work/generation154-core` and the corresponding
`generation154-formal` and `generation154-release-census` directories. Qualification
must join exact source inventories, emitted binaries and test identities.
Checkpoint153's final join was invalidated by concurrent source changes; its
individual passing scopes do not qualify the current source. The prior sealed
checkpoint152 remains historical evidence.

New controls cover ordinary/replacement runtime capture, actual carrier success,
dropping an admission-refused carrier, admission panic, late membership refusal,
original membership allocation/notification custody, unwind and terminal
revocation. Existing admission, framing, gossip, repair and publication controls
remain in the required selection. The release checker includes all eleven new
Core controls. Formal mutations bind the same concrete owners and release order.

Consuming commit, enclosing State acquisition, effect-lock preparation, complete
resource admission, the retained production Validate/Apply cutover and unchanged
four/seven-validator fault/restart/final-transaction qualification remain open.
All L1–L6 remain active; this capture correction alone establishes no release
readiness.
