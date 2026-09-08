# Compiler test-source inventory

`kotodama_fixtures_v1.manifest.json` seals the current `.ko` consumers in the
compiler, semantic analysis, and IR test modules. It records exact include paths,
function or test-macro ownership, lexical test order, fixture bytes, and content
hashes. The checker also requires complete membership of the explicitly owned
fixture directories and the declared external sample fixtures.

Run the read-only CI check from the repository root:

```sh
python3 scripts/check_kotodama_test_sources.py
```

After an intentional source, fixture, or test inventory change, regenerate and
review the manifest with `python3 scripts/check_kotodama_test_sources.py --write`.
Regeneration rejects missing, duplicated, unreferenced, or out-of-policy fixtures.
Changes to the owned Rust include modules or fixture directories also require an
explicit policy update in the checker. The manifest describes current consumers;
it does not claim to reconstruct historical Rust string literals.

Validate checker changes with
`python3 -m pytest pytests/scripts/kotodama_fixture_manifest_test.py` after
installing the pinned Python dependencies in `scripts/requirements.txt`.
