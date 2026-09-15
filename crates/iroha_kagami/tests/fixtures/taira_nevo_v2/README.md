# Taira NEVO genesis fixtures

Production composition reads the canonical Taira genesis and the single
`configs/soranexus/taira/nevo_genesis_overlay.template.json` transaction. It does
not read the full golden genesis. The unsigned golden below independently checks
that the current base is preserved and exactly that overlay is appended.

Run `python3 scripts/refresh_taira_nevo_fixtures.py` to check the generated pair.
After reviewing a canonical base or overlay change, run it with `--write` to
refresh `unsigned-genesis.template.json` and the three source/genesis digests in
`review.json`. The command preserves the public identity, permission, credential
hash and secret-boundary inventory; it refuses unreviewed public-input drift.

Run `pytest pytests/scripts/refresh_taira_nevo_fixtures_test.py` and the Kagami
privacy release tests after regeneration. Native validation still checks the
complete review inventory, current template hashes, exact recomposition,
authority ordering, alias permissions and strict Taira account decoding. Fixture
refresh is not approval, signing, deployment or release qualification.
