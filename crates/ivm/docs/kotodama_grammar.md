# Kotodama V1 grammar

The single normative Kotodama V1 language specification is
[`specs/kotodama_grammar.md`](../../../specs/kotodama_grammar.md).

This file is retained only as a stable link for readers of the `ivm` crate.
The compiler has no compatibility grammar or edition selector. V1 declaration
features use the paired canonical spellings `seiyaku`/`誓約`,
`kotoage`/`言挙げ`, `hajimari`/`始まり`, and `kaizen`/`改善`. English
`contract`, `entry`, `init`, and `upgrade` are rejected as declaration
keywords; other translated aliases are rejected too. Editor support, examples,
and generated language tables derive from the V1 grammar sources, while
repository-local examples are checked against the normative specification.
