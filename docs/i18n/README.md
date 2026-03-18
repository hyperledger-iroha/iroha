# Documentation Localization Workflow

The `docs/i18n` directory centralizes configuration and tooling for the
documentation localization workflow. English files remain the source of truth;
translations live next to their source file using the pattern
`<name>.<language-code>.<ext>` (for example `README.ja.md`).

## Configuration

`manifest.json` describes which files require translations and which languages
the project maintains. Important keys:

- `primary_language` – the canonical language (English).
- `target_languages` – every additional language. Each entry contains the ISO
  language code, human-readable name, optional autonym, and the text direction.
- `include` – glob patterns (relative to the repository root) that opt files in.
- `exclude` – glob patterns for files or trees that should not be mirrored.

Update the manifest when new documentation directories or languages are added.

Current target languages:

- Japanese (`ja`, 日本語, LTR)
- Hebrew (`he`, עברית, RTL)
- Spanish (`es`, Español, LTR)
- Portuguese (`pt`, Português, LTR)
- French (`fr`, Français, LTR)
- Russian (`ru`, Русский, LTR)
- Arabic (`ar`, العربية, RTL)
- Urdu (`ur`, اردو, RTL)
- Burmese (`my`, မြန်မာ, LTR)
- Georgian (`ka`, ქართული, LTR)
- Armenian (`hy`, Հայերեն, LTR)
- Azerbaijani (`az`, Azərbaycan dili, LTR)
- Kazakh (`kk`, Қазақ тілі, LTR)
- Bashkir (`ba`, Башҡорт теле, LTR)
- Amharic (`am`, አማርኛ, LTR)
- Dzongkha (`dz`, རྫོང་ཁ, LTR)
- Uzbek (`uz`, Oʻzbekcha, LTR)
- Mongolian (`mn`, Монгол, LTR)
- Chinese Traditional (`zh-hant`, 繁體中文, LTR)
- Chinese Simplified (`zh-hans`, 简体中文, LTR)

## Generating translation stubs

Run the helper script from the repository root to create placeholder files:

```bash
python3 scripts/sync_docs_i18n.py
```

The script inspects every English source document matched by `include`, skips
any paths that already contain a translation for the requested language, and
creates a stub with metadata for translators. Use `--dry-run` to preview the
planned changes or `--lang <code>` to target specific languages.

## Translating

- Replace the stub text with the translated content and update the `status`
  field in the front matter (for example, `status: in-progress` or `status:
  complete`).
- Keep front matter keys intact so that tooling can track progress.
- When a source document is moved or renamed, update the translation filename
  accordingly.
- Treat all `needs-translation` stubs for Japanese, Hebrew, Spanish, Portuguese,
  French, Russian, Arabic, and Urdu as explicit work items; update both the body and
  metadata when a translation is completed or reviewed so the CI dry-run check stays clean.

## Adding new languages

1. Append a new entry to `target_languages` in `manifest.json`.
2. Re-run `python3 scripts/sync_docs_i18n.py --lang <code>` to generate stubs.
3. Translate the newly created files.
4. Update any build or publication pipelines to surface the new language (for
   example, include the language filter in site generators).

## CI integration

Future CI should check that every source document has a translation stub for
each configured language. A lightweight approach is to run the script in
`--dry-run` mode and fail the job if it reports planned creations. This
repository now enforces this via `python3 scripts/sync_docs_i18n.py --check`.
The check fails if new stubs are missing and emits a warning when existing stub
metadata is stale.

Publishing guardrails also read `docs/i18n/published_locales.json` to decide
which locales are allowed to ship. Stub-only translations (status
`needs-translation`) are permitted in the repository, but published locales must
be fully translated so portal builds do not ship placeholders.
