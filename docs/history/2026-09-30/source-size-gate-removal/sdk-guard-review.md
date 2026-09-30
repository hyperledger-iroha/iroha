# Assigned source-size guard removal

The nine exact guard preimages in `sdk-guard-preimages.json` retain the preceding source-size policy as historical evidence. Current source line counts, reduction targets, and formatting-only line-width limits are retired. Semantic owners, inventories, assertion token seals, source-custody bindings, and behavioral mutation controls remain. Whitespace-growth controls now demonstrate that source size is not an admission requirement.

The typed Kotodama registry region preimages retain their exact reviewed byte digests. Current registry authentication uses the Rust-token hashes of those exact bytes; `sdk-registry-semantic-seal-review.json` records the mapping, and a unit test authenticates both historical byte hashes and token hashes. Quoted literal contents and every assertion remain part of the current token seal.

Focused validation: 59 tests and 51 mutation subtests passed. This evidence qualifies these guards, not native SDK artifacts or a release.
