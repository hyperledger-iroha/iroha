# OpenAPI lock pin before unit repair, 2026-09-30

The complete OpenAPI tooling suite passed 153 controls and failed its exact root-lock test: the retained pin described 311,935 bytes while the canonical root lock has 304,257 bytes. The original pin is preserved verbatim here; `capture.json` binds both pins and the canonical lock. The reviewed dependency-free allocation boundary and current lock are documented in the adjacent unit-test release source review and dependency audit.

The maintained pin owner generated the replacement from the explicit canonical root lock into an external temporary staging directory, then verified the exact replacement with `pin --check`. The tracked release pin alone is updated. Cargo.lock, the pin parser, file/directory descriptor custody, exact byte/hash checks, Git index/tree checks and mutation controls are unchanged. This local unit repair does not establish signed OpenAPI generator/release provenance.
