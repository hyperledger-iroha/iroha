# RAM-LFE public guidance correction

Date: 2026-09-29. Scope: two authored pages and their 20 maintained translations
in the optional sibling `iroha-docs` checkout. No publication is claimed.

The RAM-LFE page now states that both first-release BFV backends reject encrypted
execution, including signed mode. The exact-lift construction loses its public-key
noise modulo the plaintext modulus; the rounded alternative is unqualified.
HKDF evaluation is a distinct primitive and does not provide encrypted execution.
The page distinguishes ciphertext, independently authenticated plaintext openings
and receipt attestations, and states the requirements for future activation.
The existing Torii route anchor remains valid. The private-dataspace fee-sponsor
guide replaces its unsafe phone/email walkthrough with current availability;
account, alias and sponsorship instructions remain intact.

Both English sources and all 40 localized pages pass the repository's normal
content and i18n validators on an exact scoped snapshot. Translation-service
requests failed with HTTP 429 without changing files; the retained replacement
translations are explicitly labelled `codex-assisted`, with no expert linguistic
review claimed. Nonaffected localized fee-guide sections remain byte-identical
apart from translation frontmatter. Actual VitePress browser checks of Arabic,
Hebrew and Urdu confirm correct RTL direction, readable headings/prose/tables,
LTR code literals and no horizontal overflow. The temporary preview was stopped.
A complete site build and generated provenance refresh remain separate work.

Ignored evidence under `dist/zk-remediation/2026-09-29/ram-lfe-public-docs/`
contains the 42-file before/after hashes and patch, failed translation logs,
source validators, scoped snapshot metadata, successful validation results and
browser-review observations. The source-adjacent universal-account guide also
states current unavailability. No encryption implementation or release security
qualification follows from these documentation checks.
