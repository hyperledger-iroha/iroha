# language-kotodama

TextMate grammar scaffold for the Kotodama smart-contract language used by the
Iroha Virtual Machine.

This repository layout is intended for two jobs:
- editor syntax highlighting during grammar development; and
- GitHub Linguist ingestion via:

```sh
script/add-grammar https://github.com/<org>/language-kotodama
```

The current grammar targets the syntax described in:
- `docs/source/kotodama_grammar.md`
- `crates/kotodama_lang/src/lexer.rs`
- `crates/kotodama_lang/src/parser.rs`

The scaffold intentionally keeps the grammar self-contained so it can be split
out of this workspace into a dedicated repository with minimal changes.
