<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Docs/DevRel Monthly Sync Agenda

Use this meeting to keep implementation-coupled documentation in this
repository aligned with the code and to coordinate public documentation work.
Public guides, tutorials, conceptual material, and translations belong in the
sibling [`iroha-docs`](https://github.com/hyperledger-iroha/iroha-docs)
repository and are published at <https://docs.iroha.tech/>.

## Cadence and logistics

- **Frequency:** monthly, or before a release when documentation changes are
  substantial.
- **Duration:** 30 minutes.
- **Audience:** Docs/DevRel, affected code or SDK owners, Release Engineering,
  and Support/QA as needed.
- **Record:** keep concise notes under
  `specs/docs_devrel/minutes/<yyyy-mm>.md`.

## Pre-work

| Owner | Task |
|-------|------|
| Scribe | Create the month's notes file from the template below. |
| Code and SDK owners | List behavior changes that require README, Rustdoc, specification, fixture, or validation-note updates in this repository. |
| Docs owner | Link any corresponding `iroha-docs` issues or changes needed for public documentation. |
| Support/Release | Bring documentation gaps that block release or operator readiness. |

## Agenda

1. **Implementation truth:** confirm that repository-local READMEs,
   specifications, generated references, fixtures, and validation notes match
   the current implementation.
2. **Public documentation coordination:** identify user-facing changes that
   belong in `iroha-docs`; do not copy its in-depth or translated content into
   this repository.
3. **SDK and release readiness:** review documentation work that blocks SDK or
   release validation.
4. **Actions:** record an owner, target repository, and due date for each open
   item.

## Minutes template

```markdown
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Docs/DevRel Monthly Sync — YYYY-MM-DD

## Attendees

- Chair:
- Scribe:
- Participants:

## Implementation-coupled updates

- …

## Public documentation coordination

- `iroha-docs` issue or change:

## Decisions and actions

| Item | Repository | Owner | Due |
|------|------------|-------|-----|
| … | `iroha` or `iroha-docs` | … | … |
```
