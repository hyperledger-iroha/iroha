# Retired Clippy source-length expectations

The workspace permits `clippy::too_many_lines` because source line count is not
an acceptance criterion. These exact preimages preserve the preceding candidate
before removing 233 corresponding expectation entries across 113 live Rust files.
Five mixed attributes retain their other expected lints and exact reasons. Local
`allow` attributes, cfg/test attributes, function bodies and executable assertions
are retained. Historical Rust sources were excluded.

`clippy-expect-preimages/index.json` authenticates each original by SHA-256 and
byte count and records the lexical migration. Balanced Rust attributes were
processed outside comments and string/character literals; all code tokens other
than the retired expectation entries were compared before writing. Synthetic
checks cover raw strings, nested comments, mixed lists, reasons, and untouched
local allowances. This policy change does not alter runtime or wire bounds.
