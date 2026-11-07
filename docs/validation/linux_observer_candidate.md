# Linux observer candidate publication

The [candidate workflow](../../.github/workflows/linux_observer_candidate.yml)
prepares the native observer needed by the Linux resource harness. It builds
from fixed source commit `3743ff550ca3632e92b23e5d60d084b41a069b29`, separately
from the pull request's candidate driver. That revision contains the normal
`build-bridge-publish` interface and the complete source keeper.

The [driver](../../scripts/prepare_linux_observer_candidate.py) retains that
checkout through the publisher build, normal bridge publication and copying the
two original artifacts. Cargo and Rustc are explicit native toolchain binaries;
the native publisher retains its tools, configurations and locked offline build
inputs. Output and build roots are private and outside the source. Failed
commands retain logs and produce no completed candidate receipt.

Execution trust comes from the selected source, compiler and GitHub-hosted
runner. The receipt records those choices and observed hashes; it does not
authenticate the build environment independently or prove complete dependency
closure. The bridge is never loaded. No diagnostic provenance fixture is used.

The uploaded review packet contains `bridge/libiroha_process_bridge.so`,
`bridge/publisher-source.json`, `candidate.json` and command logs. A successful
receipt has status `requires_independent_approval`. Before any harness invocation,
an independent reviewer must validate this packet and bind its exact source
commit, canonical source manifest, native target, module hash, provenance hash
and immutable artifact identity in approved consumer configuration. Do not feed
the candidate job's observed output hashes directly into a loader as authority.

The existing [native test runner](../../scripts/tests/run_zk_resource_harness_native.py)
can then receive those independent pins and a separate checkout of the approved
observer source. Candidate publication alone does not qualify its 20 native
tests or replace a required CI consumer gate. See the
[harness validation notes](linux_resource_harness.md) for that execution boundary.
