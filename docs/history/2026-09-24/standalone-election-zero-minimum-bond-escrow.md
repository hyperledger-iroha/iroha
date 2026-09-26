# Standalone election zero-minimum bond custody, 2026-09-24

Scope: the existing `optimizations` checkout. The legacy ZK ballot lock helper
previously marked custody as unescrowed when the governance minimum bond was
zero, and `lock_voting_bond` returned before transferring even a positive
bond. A zero minimum no longer changes custody: the helper records escrowed
custody and rejects any unescrowed record before calculating a transfer.

The focused Core test funds a voter with ten exact units at scale zero. With a
zero minimum, a false-custody attempt fails without changing balances; a zero
bond moves nothing; an initial four-unit bond moves four into escrow; and an
increase to six moves only the two-unit delta. The current-source helper test
passes 1/1, adjacent `plain_ballot_` Core tests pass 4/4, and the grouped
`gov_plain_conviction` integration selector passes 5/5 against the same source.
With `zk-tests` enabled, the guarded `gov_zk_ballot_lock_verified` integration
selector passes 1/1 and still refuses an unqualified ballot before lock
mutation. Scoped formatting and diff checks pass.

This is a conservation prerequisite in a production-closed election path. It
does not supply credential-linked authority, confidential bond positions,
immutable cast/update semantics, a sound tally, or a committee-free late-
dropout protocol. The standalone ZK relation guard remains closed.
