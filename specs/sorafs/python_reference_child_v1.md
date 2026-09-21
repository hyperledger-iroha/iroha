# SoraFS Python reference child V1

`SorafsPythonConsumerQualificationRunner.py` is a fixed observation child for the
77 cases in `python/iroha_python/tests/sorafs_reference_validation_test.py`.
It consumes already installed native and SDK wheels under Python 3.12 and pytest
9.0.3 on a POSIX host with no-follow, nonblocking file opens. It does not build,
install, fetch dependencies or promote a candidate; other platform execution
remains unqualified.

The parent launches it with `-I -B --input <canonical-input.json>` in a private
venv, with `PYTEST_DISABLE_PLUGIN_AUTOLOAD=1` and no other `PYTEST_*` variables.
The closed `sorafs.python.reference_child_input.v1` object carries snapshot and
venv roots, native/SDK wheel path and original FileSeal pairs, sorted complete
source-file identities, and the actual Python executable identity. Sources are
exactly the runner, wheel verifier, case owner, test module and complete copied
SoraFS fixture, Norito source and Torii source trees. The entire snapshot admits
only those files and their exact parent directories; unexpected package initializers,
siblings and empty directories refuse before imports. Directory identities also
survive to the final source recheck. Norito and Torii are source
dependency owners here; they are not qualified installed distributions.

The child uses the existing sole wheel verifier without weakening its archive,
RECORD, installed-file or import policy. The fixed case owner checks the trusted
test AST inventory without importing the test to infer expectations. Actual
pytest collection must match all 77 ordered IDs, including the original 10,000
character parameter. Every setup, call and teardown phase must pass. Skips,
xfails, duplicates, missing phases, changed ordering and nonzero exit refuse a
report. `--assert=plain` still executes every test assertion; `--noconftest` keeps
checkout import setup out of this installed-owner boundary.

Original loaded module/spec/loader references, immutable descriptor values,
package search paths, installed files and copied sources survive to the final
checks. `sorafs._crypto` must again be the original extension after the deliberate
monkeypatch test's teardown. Every loaded wheel/dependency descendant must match
its own sealed file; the child preserves original wheel and file identities.

A successful child emits actual captured log bytes followed by one final
`SORAFS_PYTHON_REPORT_V1=<base64 canonical JSON>` line. JSON uses sorted keys,
ASCII escapes, compact separators and one LF. Logs are limited to 32 MiB and the
JSON report to 8 MiB; the total pipe bound is their sum plus Base64 expansion and
frame overhead. A framing LF, if needed, belongs to the captured log digest.
The parent must reject nonzero exit or unexpected stderr and compare every
pre-frame pipe byte with the report's captured-output size/digest. Native writes
which bypass Python stream capture therefore refuse instead of disappearing.

The report binds the input digest, complete source inventory, Python/pytest
observations, actual wheel/install/loaded-member observations, both source
dependencies and all 231 phases. Parsing it alone conveys no producer authority.

The [source-owned parent](python_consumer_producer_v1.md) supplies full CPython
runtime/stdlib and original wheel/dependency custody, bounded process execution,
actual logs, native manifest and candidate-source joins, and publication after
final original-input verification. TODO: complete its original-index adapter and
signed aggregate integration, then execute matching-candidate qualification.
Synthetic runner controls and the original inert wheel harness are not execution
of the canonical native assertions, cross-platform or SF11 release qualification.
