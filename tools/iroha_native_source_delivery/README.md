# Retained Python source delivery

This internal executable is the native filesystem transaction used by
[`ci/python_native_source_delivery.py`](../../ci/python_native_source_delivery.py).
It has a separate compilation boundary so Windows delivery does not load an SDK
or compile the node runtime to perform file custody. It uses `iroha_fs` for all
native custody and Norito JSON for bounded requests over inherited standard I/O.

Build from the same pinned source with
`cargo build --locked -p iroha_native_source_delivery`. Pass the absolute executable
path to the Python `promote` command as `--native-helper`. Windows requires this
argument. The backup must be an absent direct child of an existing retained parent,
or an empty directory already satisfying exact private custody. Existing access
permissions are never repaired. A new backup receives private access at creation.

One process retains the original file and both directory ancestry chains while
the Python owner verifies its original file seals. The ordered requests are
`begin`, `intent`, `stage`, one or more `chunk` requests, `seal`, `backup`,
`activate`, `receipt` and `complete`. Native libraries receive a separate exact
owner-read-and-execute seal (Unix `0500`; Windows read/execute DACL). Intent and
receipt files retain their exact read-only seal. Fresh files move through their
original sealed handles; sealed control readback must equal the caller's exact
authored bytes before acknowledgement. Old artifacts move through their initially admitted
handles. Destinations must be absent. Both namespace changes are synced. The old source
name may be reused only after its original object reaches the backup. The helper
keeps that object retained through final receipt validation.

EOF, a rejected request or an interrupted operation preserves all remaining
objects, including partial staging files. There is no rollback, cleanup or
automatic loader fallback. Recovery must authenticate the durable intent and
remaining physical objects. A failed helper run never produces successful delivery
evidence. The Python owner separately verifies source identity, wheel contents,
ABI evidence and the receipt; the helper does not establish build provenance.

Run the Rust controls with `cargo test --locked -p iroha_native_source_delivery`.
For the Python integration controls, build the binary, set the test-only
`IROHA_NATIVE_SOURCE_DELIVERY_TEST_HELPER` to its absolute path and run
`python -m pytest -q scripts/tests/python_native_source_delivery_test.py -k retained_native`.
The tests use inert native payloads and do not qualify an installed Python SDK.
