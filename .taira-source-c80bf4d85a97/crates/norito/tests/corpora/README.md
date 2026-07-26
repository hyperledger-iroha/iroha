This directory holds large JSON corpora for Stage‑1 parity tests.

How to use (no network in code):
- Place known corpora here (e.g., `citm_catalog.json`, `twitter.json`).
- Generate `.tape` structural offsets for each JSON file:
  - `scripts/generate_tapes.sh crates/norito/tests/corpora`

Notes:
- The `.json` and `.tape` files are ignored by git (see root `.gitignore`).
- Tests will skip gracefully if no `.json + .tape` pairs are present.

