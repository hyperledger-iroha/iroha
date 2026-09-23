# Fixed JavaScript child process custody slice

`scripts/sorafs_javascript_child_process.py` is a non-authorizing, one-shot
owner for the existing original child-input and physical Node runtime owners.
It enters the runtime owner first, so failed child-input entry closes runtime
descriptors. It rechecks both originals before launch and after the child has
reached both output-pipe EOFs and direct process exit. The original child-input
file descriptor is mapped to inherited fd3; argv is exactly the selected
runtime pathname, the fixed private child module, and the original input SHA-256.
The spawn environment is an explicit five-key map. A sole-Python-thread launch
closes observed unrelated inheritable descriptors and makes stdin `/dev/null`.

Stdout is bounded to the exact base64 expansion of an 8 MiB observation and
must contain exactly one `SORAFS_JAVASCRIPT_CHILD_V1` frame ending at EOF.
Stderr is bounded to 4 MiB and must be empty on successful exit. Timeout,
nonzero exit, truncation, extra output, malformed base64, pipe overflow, or
changed originals fail. On process failure, bounded captured stdout/stderr are
attached to the local error for diagnosis; no output is a release receipt.
Timeout cleanup signals only the isolated process group created for this owned
child and then reaps the direct child. Inert Python fixture tests are all under
this checkout's `target/` directory and do not run Node or a native addon.

Focused validation on this slice: `python3 -m pytest
scripts/tests/sorafs_javascript_child_process_test.py -q` (14 passed, including
partial-entry, descriptor-census, selector-cleanup, and failed write-close
controls). This validates process/pipe mechanics only.

Both SoraFS workflow path inventories now include the source and focused test.
`ci/check_sorafs_cli_release.sh` runs that test in its existing strict pytest
batch, and `scripts/check_sorafs_release_automation.py` requires the test's
single registration. The checker passed, shell syntax passed, and the combined
automation/process selection passed 1,499 tests before the additional
write-close regression, using repo-local temporary files. These are automation
coverage checks, not actual Node qualification.

TODO: wire this owner into the source-owned parent and original-index verifier,
and pin the complete selected Node runtime and actual mapped image relation.
`posix_spawn` opens the selected pathname after the parent recheck, so a
path-replacement race and dyld's actual mapped-image selection remain open.
The descriptor census is a sole-Python-thread snapshot before spawn; native
threads or OS activity could still race it. `posix_spawn` also inherits the
parent working directory; no authenticated working-directory owner is joined
here. The tests use an inert Python child,
not the reviewed Node24 runtime, installed SDK, genuine ABI checker, 55 cases,
or 172 assertions. No release gate or promotion authority follows from this
component.
