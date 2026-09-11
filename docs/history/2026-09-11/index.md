# Subsystem evidence recorded on 2026-09-11

These results apply to their recorded sources and artifacts, not the complete release.

| Subsystem | Evidence |
| --- | --- |
| [JSON context and primitive ownership](json-context-primitives.md) | Explicit primitive codecs and collection error cleanup; the coherent four-package selection passes 1,854 tests, both compiler UI suites and strict all-target lint. |
| [Native JSON value destruction](json-value-destruction.md) | Ordinary owner destruction replaces cleanup callbacks; two actual overflow reproducers are fixed, with 1,862 tests and strict all-target lint passing in the isolated candidate. |
