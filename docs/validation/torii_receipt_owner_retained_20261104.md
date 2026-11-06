# Retained Torii receipt-owner diagnostic

On 2026-11-04 UTC, the exact test
`finality_interval::tests::maximum_payment::full_payment_blocks_use_actual_native_owner_for_every_receipt_and_last_http_slice`
passed: **1 passed, 0 failed, 0 ignored**, exit 0, no timeout. Libtest reported
89.23 seconds; `/usr/bin/time` reported 89.28 seconds elapsed. The native child
ran from 15:33:31 to 15:35:00 UTC under a 900-second outer supervisor ceiling,
with unchanged internal limits and default stack/worker settings. The supervisor
held the State campaign lock throughout; the diagnostic ran after the original
State campaign terminated and before its separate continuation.

This is the [native HTTP receipt-owner test](../../crates/iroha_torii/src/finality_interval/maximum_payment.rs),
distinct from the six passing SNS lease tests in the
[earlier component evidence](sns_current_cut_takeover_20261104/README.md).
It executes two exact 16 MiB paid proposals, checks four original payment
receipts through independently verified finality, and exercises Single(H3) and
Interval(H2–H3) responses. The assertions preserve original query credit and
charged response ownership through the final HTTP byte slice, then verify full
release. Related Core controls do not substitute for this HTTP-owner result.

Both response cases printed the same request-owner accounting, in bytes:

| Request grant W | Fixed | Frames | Metadata | Graphs |
| ---: | ---: | ---: | ---: | ---: |
| 384,000,000 | 9,561,666 | 133,082,720 | 1,288 | 241,354,326 |

The four components sum exactly to W. This is the request reservation. The
separately observed whole-process maximum RSS was **655,589,376 bytes**, and
peak memory footprint was 464,209,168 bytes. The request grant does not establish
a total-process RSS ceiling. The response cap remained 64,000,000 bytes and
each route retained its 65-second internal deadline.

The retained executable was
`target/debug/deps/iroha_torii-9af9d7faacdfc9fc`, SHA-256
`4b442b8e0251aadc4182306bf3080062ee20bf8a6423ea4dc47df795c097efd9`
(1,303,819,440 bytes), with fingerprint SHA-256
`5384069d29cfd4b3c2bb1c90449100acf9a5405f26c7ba7c37cea0790f018a7a`.
Artifact, fingerprint, all 28 source guards, observed HEAD and live lock matched
before and after execution.

**`current_candidate_qualified` remains `false`.** Eleven guarded files belong
to the original narrow SNS source capture, including the committee-authority
repair in `query_reader.rs`. Seventeen additional files are present-read guards
only. The receipt-owner test and native payment fixture match recorded HEAD
bytes, but that observation does not establish retrospective build custody.
`crates/iroha_torii/src/lib.rs` and `torii_fanout_decode_helpers.rs` changed after
the retained binary was built. Their current bytes are not attributed to that
binary. The recorded build lock
`eee6edbd0c0c7ee334e617cbf3e4e6d20d0ba3d552a508495ef2fec9675b8384`
is not rebound to the observed live lock
`de2c5b604926e792d35705a4d426bb0547d103779d96fb572103ca96c5bf6e88`.
The [original lock-drift evidence](sns_current_cut_lock_drift_20261104/README.md)
remains unchanged. This result establishes a retained-artifact component
diagnostic; full transitive build custody, current-candidate, whole-node,
network and release qualification remain open.

Generated logs, receipts and supervisor sources remain external and untracked.
The following paths are relative to
`/Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/state-candidates-isolated-20261104/torii-receipt-owner-diagnostic-01/`.
The nine synthetic supervisor controls passed without native execution; their
original output was retained without a rerun. Root separately reviewed the
supervisor and the completed result, with no blocking findings or test rerun.

| External artifact | SHA-256 |
| --- | --- |
| `../supervise_torii_receipt_owner.py` | `a4829db0cc24b4f9f6864f6cc06d4c5c87205512b1bd512ff679ad2d7bba4481` |
| `../test_supervise_torii_receipt_owner.py` | `d494ad765360febf4a7e5baa391514cb74316711f36b1e04cc8f4e2d41e869fe` |
| `preparation.json` | `b37fea51dd1e21c082297f095746acd25187cdd11db3e59eab83a429cd7da2c2` |
| `supervisor-final9-controls.json` | `777923d1ee0bbbfeb62208348b034f43c0a9bd8854a96c47615dbc46e7bdb906` |
| `supervisor-final9-controls.log` | `ba0f0e59a6227cb362eca5007a29426ec0525c99666a3218213d68cedffa66a4` |
| `root-supervisor-review.json` | `1f513562a80cc3c656548d3b860e522850cf5ae77235b7c0a047f95725819290` |
| `run-bd8ef8b88f9f49bea5792e571a067d3b/result.json` | `73917a2a1a4a6111359836edf1900ef39275606a78eebf87866778682e88f06e` |
| `run-bd8ef8b88f9f49bea5792e571a067d3b/stdout.log` | `c3c93802a5abbbf30848c9e0ea7994fc5966a3f464edf3678cce28152be7f900` |
| `run-bd8ef8b88f9f49bea5792e571a067d3b/stderr.log` | `2d074b930864f5b5c76aad4140c121e7cf3436fe33c59d28a2a5674046745174` |
| `root-result-review.json` | `ddc61db93712a5bdc89fa02acb1681eff7e9999ad5c1395201fc80b1275e96da` |
