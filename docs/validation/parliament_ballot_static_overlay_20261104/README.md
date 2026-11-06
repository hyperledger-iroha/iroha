# Current fixed-slot writer diagnostics

The source guard now accepts the two reviewed helper revisions. Their circuit,
framing, layout and resource formulas are unchanged; only antecedent evidence
paths/hashes and a diagnostic row-scope field changed. The fixed-slot writer
itself changes only those two authenticated helper hashes.

All nine circuit, composition, source-refusal and CLI tests pass. All four
row/role reports are identical to the historical outputs except their two
source-hash maps. Ordinary output exits 0; `--require-qualified` exits 2 with
identical JSON. Full request/root integration, physical costs and cryptographic
qualification remain open. Historical artifacts retain their original bytes.

`source-review.json`, exact helper snapshots/diffs and `review.py.template`
retain this bounded review. Run tests with
`python -m pytest -q scripts/tests/parliament_ballot_static_overlay_test.py`.
