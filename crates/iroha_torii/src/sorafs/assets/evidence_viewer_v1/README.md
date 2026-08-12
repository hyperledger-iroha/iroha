# SoraFS protected evidence viewer assets v1

The HTML, CSS, and JavaScript files are the exact UTF-8 response bodies served
by the protected evidence viewer. They remain compile-time `&'static str`
constants through `include_str!`; no runtime parsing or allocation was added.

`manifest.json` pins byte lengths, SHA-256 digests, historical Rust line spans,
and the unique package-local consumers checked by the repository asset audit.
