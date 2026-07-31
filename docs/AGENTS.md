# AGENTS Instructions

These guidelines apply to the `docs/` directory.

## Documentation ownership
- The canonical public and in-depth Iroha 3 documentation lives in the sibling
  [`hyperledger-iroha/iroha-docs`](https://github.com/hyperledger-iroha/iroha-docs)
  repository and is published at <https://docs.iroha.tech/>.
- Keep files under `docs/` concise and coupled to this source tree, including
  contributor guidance, generated reference artifacts, and validation notes.
- Put implementation specifications in `specs/`, formal artifacts in `formal/`,
  and executable documentation fixtures in `fixtures/documentation/`.
- Put new user guides, operator manuals, tutorials, conceptual explanations, and
  other long-form public material in `iroha-docs`. Link to it instead of
  duplicating it here.
- Describe the current first-release Iroha 3 implementation truth. Replace
  obsolete or pre-release guidance instead of carrying compatibility narratives
  into new documentation.
- Do not make any build, test, validation, or generation workflow in this
  repository depend on a sibling `iroha-docs` checkout.

## Development workflow
- Write in Markdown and keep links relative when possible.
- When code examples are updated, run the narrowest relevant tests and ensure
  they compile.
- Prefer runnable examples in the owning crate's `examples/` directory and link
  to them from repository-local notes when useful.
- Validate links and anchors when changing paths. Optionally use `lychee` with
  `lychee.toml` at the repository root for link checking.
- Treat the Kotlin SDK under `kotlin/` as the default Android/JVM SDK in new
  documentation. Keep Java Android docs only when they describe the mirrored
  compatibility surface or migration details.
- Follow the repository root `AGENTS.md` for formatting and general guidance.

## Status and roadmap
- For current implementation status and planned work, consult `status.md` and
  `roadmap.md` at the repository root.

## Useful commands
- Build docs for a crate locally: `cargo doc -p <crate> --no-deps --open`
