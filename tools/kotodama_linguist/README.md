# Kotodama GitHub Linguist Bundle

This directory packages the minimum artifacts needed to pursue first-class
Kotodama support in `github-linguist/linguist` so `.ko` files can render as
`Kotodama` on GitHub.com instead of being force-mapped to an unrelated
language.

Included assets:
- `grammar-repo/`: a standalone TextMate grammar repo scaffold that can be
  pushed as its own repository and referenced from `script/add-grammar`.
- `linguist/kotodama-language-entry.yml`: the proposed `languages.yml` entry.
- `linguist/PR_CHECKLIST.md`: the exact upstream steps and commands.
- `linguist/PR_BODY.md`: a ready-to-fill pull request body.
- `samples/`: representative `.ko` source files that exercise the current
  syntax surface.

What still must be verified outside this checkout:
- GitHub Linguist currently requires public usage evidence before accepting a
  new language. As of 2026-03-31, the published bar is at least `2000` indexed
  files in the last year for a normal file extension, excluding forks.
- Anonymous GitHub code search no longer exposes the result counts, so that
  evidence must be gathered with an authenticated browser session before
  opening the upstream PR.

Authenticated search queries to run:
- `extension:ko seiyaku NOT is:fork`
- `extension:ko kotoage NOT is:fork`
- `extension:ko "register_trigger" NOT is:fork`
- If one user or repo dominates the result set, re-run with exclusions such as
  `-user:<name>` or `-repo:<owner>/<repo>` to show broader adoption.

Suggested execution order:
1. Publish `grammar-repo/` as its own repository, for example
   `github.com/<org>/language-kotodama`.
2. Verify the grammar locally in a TextMate-compatible editor or VS Code.
3. Open the upstream Linguist PR using the files under `linguist/`.
4. Attach the authenticated GitHub search screenshots or links proving the
   adoption threshold and distribution across distinct repositories.

Source material for this bundle:
- `docs/source/kotodama_grammar.md`
- `crates/kotodama_lang/src/lexer.rs`
- `crates/kotodama_lang/src/parser.rs`
