# Microsoft Vega provenance

This directory vendors the native Rust Vega prover and its independent Python
reference oracle from Microsoft `vega-prover` at the exact reviewed revision:

- commit: `c0ee259053cd12eaf43ed71b5cde375452b3ee4d`
- canonical Git tree: `7226b6cbfbfe8613dd2d5ee831096b7578a5c115`
- GitHub commit signature status at capture: verified
- GitHub source-archive SHA-256: `449ed7f8ed48902a6cfb815051a4ca3ff0a6e2c34584e9996f282b462dddec22`
- license: MIT; the upstream notices and `LICENSE` are preserved unmodified

`UPSTREAM_MANIFEST.sha256` pins the pristine SHA-256 of all 104 vendored files
that originate in that Git tree. Most remain byte-identical. Five narrowly
scoped integration patches and every Iroha-added file are enumerated in
`IROHA_PATCHES.md`; its table pins both the pristine and patched hashes.

The SHA-256 of the sorted per-file SHA-256 manifest for all files in this
directory except this provenance file is below. Paths in the manifest are
canonical UTF-8 paths relative to this directory, with `/` as the separator,
so the digest is independent of the checkout location:

`539c54251c8853fa99673e71d777966a3e3e238e64028d47b3e683329023236f`

The manifest is reproduced with:

```sh
cd vendor/vega-prover
find . -type f ! -name IROHA_PROVENANCE.md -print0 \
  | LC_ALL=C sort -z \
  | xargs -0 shasum -a 256 \
  | sed 's#  \./#  #' \
  | shasum -a 256
```

The upstream `reference/.gitignore` remains byte-identical. Iroha adds the
more-specific `reference/fixtures/cubic/.gitignore` solely to force the two
pinned Python oracle binaries into source archives.

Three oracle artifacts are pinned separately:

- official committed transcript vector:
  `94967a280907fb3c5c61ff90ac593ff824d0029a1497dba819e701a4de507bc2`
- standalone Python-generated verifier key:
  `fdb982961889d7fe5757bf12b12a3a8b9fb18f764c024ad179d5eb145dec5b2e`
- standalone Python-generated proof:
  `59aa887109f509268e21614589198071f4a84beabb8ebb63bcd2ba23844fec8a`

The latter two were produced by the unmodified upstream
`reference/tests/test_standalone.py` from the pinned tree. That program performs
Python-only setup, proving, serialization, parsing, and verification before it
writes the artifacts; no Iroha or Rust prover code participates. Iroha's active
cross-conformance test then requires the pinned native Rust verifier to accept
that independently produced Python verifier key and proof.

Any future update must select a new exact commit, re-establish the pristine
manifest and signature status, review every integration patch, regenerate the
standalone Python oracle, and update every digest and executable provenance
gate in one reviewed change. An upstream-origin file may differ from its
pristine digest only when it is explicitly declared in `IROHA_PATCHES.md` and
the executable cross-conformance gate.
