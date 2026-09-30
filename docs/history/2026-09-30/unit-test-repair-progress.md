# Unit-test repair progress, 2026-09-30

This supplements the earlier checkpoint with completed local runs. It does not
claim that the workspace, current native SDK artifacts, release or live network
are qualified. Native SDK counts in the earlier checkpoint remain scoped to
their frozen artifacts; later source changes require independent reruns.

| Completed run | Evidence |
| --- | --- |
| Complete Python suite | 4,583 passed, 10,900 subtests passed, five skips, 31m25s |
| Genuine compiler-cache prerequisites | Official pinned Solidity/TRON compilers authenticated; both formerly skipped artifact roundtrips pass, no lock rewrite |
| Platform exclusions | Three Python native-execution cases explicitly require Linux; final full Python rerun must include the two newly enabled compiler cases |
| Actual replay-origin regression | One genuine signed Parliament operation passes across refusals, replay and duplicate completion |
| IVM owned prepared arguments | One actual VM regression passes; wrappers retain the immutable prepared owner and decode once |
| Taira deployment/reset fixtures | 416 passed, no exclusions in the focused namespace |
| Canonical Kotlin Log construction | 12 parity controls pass; five Level tags and invalid tags use the existing framed codec |
| Android examples | 19 retail-wallet and seven operator-console tests pass, no skips; signed fields, duplicate/unknown/numeric/UTF-8 refusal and actual signer authority are exercised |
| Android owner workflow | Both debug APKs, canonical AAR, Java consumer test invocation and both sample manifests succeed; no physical-device qualification |
| SoraFS exporter | 14 tests pass, including exclusive parallel fixture directories and unchanged signature/publication refusals |
| Xtask optional binary | 464 passed, no exclusions; all nine observed fixture failures repaired |

The size gate remains removed. Rust library/default-bin and optional-bin gates,
153 standalone local/UI targets, the mixed local integration inventory, the
complete script suite and current authenticated Apple/JavaScript/native SDK
runs remain outstanding. Concurrent compiler ownership edits are preserved;
source seals, dependency fingerprints and fixture anchors must be reviewed on
the coherent final source, never on an intermediate migration.

The exact source-guard review records preserve originals and explain rejected
concurrent inputs. Local logs and runtime checkpoints remain ignored under
`target/unit-tests/`. Passing example tests and debug APK assembly establish
component evidence only.
