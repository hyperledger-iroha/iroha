# Implicit function line-count gate retirement

The workspace's pedantic Clippy group implicitly enabled `too_many_lines`,
turning function line counts into failures in strict CI. The root manifest now
allows that one lint. All other lint levels remain unchanged. The obsolete
expectations are removed from 113 Rust files: 233 entries, including five mixed
attributes whose other expected lints and reasons remain intact. The lexical
retirement record authenticates 14,521,495 exact original bytes. The separately
recorded canonical Taira fixture guard is a generator refusal control.

The dependency review preserves every existing limit and compares all six
scope measurements with the preceding reviewed compiler ownership graph.
Every measurement is identical. Only the manifest fingerprint changes to
`sha256:bdd6511aef73ae211b751d06aa0ec3529865ab8684bd5855581e38c442f92308`.
All 46 dependency guard tests and the actual source graph check pass. A focused
strict Clippy run passes for sm3-neon. The nine retained source guard suites
pass 59 tests and 51 subtests after expectation retirement.

The panic inventory changes only the 32 authenticated source hashes affected
by annotation retirement. An independent complete boundary scan retains all
973 paths and the same 54 catch, 21 blocking, 397 task and 10 upgrade counts.
The previous inventory and exact 32-row review are preserved separately. The
current inventory digest is
`78647065c5d1def770c4c63840639e48ee2fb48eb934867d9914bf0b7e215981`.

`clippy-loc-preimages.json` preserves all 33 changed selected inputs relative
to the successfully tested source-size/compiler seal. There are still 7,021
selected release inputs. This source review authenticates a development
candidate; complete unit and native SDK qualification remains separately
required and does not establish release or settlement readiness.
