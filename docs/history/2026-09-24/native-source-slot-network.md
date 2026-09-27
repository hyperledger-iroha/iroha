# Native source-slot local network qualification

This is local development evidence from `/Users/takemiyamakoto/dev/iroha` on
`optimizations`, not a signed release or a general liveness proof. The tested
source was `HEAD c4a66997872c683763e92c500650f92b9db0418c` with tracked
binary diff SHA-256
`9f05bcd0deb136a854682e882637f77de08d56dfb99bb6b4c33922f9a7e64f4f`.
The source, index, branch, HEAD and binary hashes were unchanged before and
after each network run. This evidence record was written after those runs.

The Native process now retires an obsolete signed Validate source request
against its exact lifecycle owner, preserves an already buffered response
until that owner is settled, and prunes closed-lane candidate waits only after
an authenticated current-lane observation. The candidate source test helper
is gated by both `test` and `bls`; the production daemon build caught and
resolved its prior test-only helper reference before the network runs.

The `test-network-message-control` local-release `iroha3d` binary had SHA-256
`2974162708edff1b1b970015680bede7f3e0b459535d2baa26b946dde921d349`.
The isolated integration harness had SHA-256
`093e12988e79c92b69dc5ff06183d6d111764e9be33323c6d06db07bc91ee585`.
Each run used those exact copied binaries, required a real network, allowed
one startup attempt, and retained its peer logs under
`dist/sumeragi-main-work/current-native-source-network/`:

| Case | Result | Elapsed |
| --- | --- | ---: |
| Four validators: silent first Native author, one finite input and restart | passed | 95.57 s |
| Seven validators: finality with two offline, then both restart | passed | 218.59 s |
| Four validators: same-subject locked reproposal after ordered quorum release | passed | 67.76 s |
| Four validators: distinct-subject PrepareQCs after causal release | passed | 189.17 s |

The same copied binaries also passed the four-validator NPoS stopped-leader
rotation case in 173.01 s. Its snapshot includes this evidence file, which
was written after the four runs above; the Rust binaries did not change.

The nine-peer signed-observer slow-reader pressure case then exposed a
separate liveness failure on those binaries. Four validators and four of five
observers reached the successor, but `excellent_fieldmouse` retained a
certified height-two body fetch and stayed at height one after the relays
reopened. Its exact-output fanout had completed four targets and retained
actor tickets for four others. Two historical certified-body requests had
been refused at a bounded archive-worker gate as busy/rate-limited. Periodic
`FetchBody` retries were coalesced with the unfinished fanout, so the
responsive archives did not receive another request. The one-attempt run
failed after 367.03 s; raw peer logs and its unchanged-input summary remain
under `observer-pressure-20260924-1/`.

The live fetch owner now retires the previous certified request's transport
fanout before re-offering the same signed request on a reducer retry. This
keeps one fetch owner and allows a temporarily busy archive to be asked again
even when a different target still holds an actor ticket. The new regression
`certified_fetch_retry_reoffers_after_one_archive_target_keeps_its_ticket`
passes, as do all 37 selected `certified_fetch_` Core tests. The daemon rebuilt
from the changed code with SHA-256
`5d7fc1d7982258ae4e43aaa429f54b84ca6f7e2d6edb9cf97fef93fe693ff455`;
the isolated harness remained byte-identical. On the same unchanged tracked
candidate (`HEAD c4a66997872c683763e92c500650f92b9db0418c`, diff SHA-256
`5f77351b7d4bdd9174297df5aa88685595111ee8935730aa98c0c0068065b42a`),
the exact nine-peer scenario passed twice, in 174.51 s and 168.21 s. Both
one-attempt runs required a real network, matched their source snapshots
before and after, and retained logs under `observer-pressure-retry-20260924-1/`
and `observer-pressure-retry-20260924-2/`. This record update came afterward.

The same copied daemon and harness then passed the seven-validator
`authoritative_v2_finalizes_through_two_validator_restarts` case in 270.12 s,
including finality with two validators offline, both restarts and the finite
final transaction. Its unchanged tracked snapshot had diff SHA-256
`726c98ce7c37886e2399d5a0e2216d8b74fdfcee8d1327218c139336039bcb2f`
after this record and the retry regression were tightened; the production
binary was unchanged. Logs and the one-attempt summary are retained under
`two-outages-retry-20260924-1/`.

The focused `iroha_core --lib native_source_` selection passed 17/17. The
canonical `check_sumeragi_v2_multilane_models.py` source-binding gate passed.
The passive-recovery contract mutation suite passed 103/103 in an ignored
Python virtual environment. `cargo fmt --all -- --check` and
`git diff --check` passed. The production daemon and isolated harness both
compiled from this checkout. The network summaries, binary hashes, source
snapshots and raw test logs are retained with each case.

The full 32-seed four/seven-validator restart matrix, broader loss,
reordering, backpressure and final-transaction campaign, and clean signed
release qualification remain open. Queue-retirement publication still treats
local pending Queue work as a wait condition; it needs a state-derived drain
protocol that cannot depend on the height it blocks. The earlier intermittent
controlled-drain timeout described in
[`native-source-observation-refresh.md`](../2026-09-23/native-source-observation-refresh.md)
also remains unresolved. The two observer-recovery passes establish this
specific retry path, not all network schedules or the remaining acceptance
matrix.
