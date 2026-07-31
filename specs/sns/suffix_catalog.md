<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
title: Sora Name Service Policy Inspection
summary: Catalog-free external names and live on-chain SNS policy inspection.
---

# Sora Name Service policy inspection

External dataspace, domain, and account-alias names are catalog-free textual
values. The first-release surface does not embed a `.sora`, `.nexus`, or
`.dao` suffix table and does not assign those strings the numeric IDs 1, 2,
or 3. Operators must not derive a textual name-to-`DataSpaceId` mapping from a
documentation snapshot.

The planner resolves configured static mappings together with active SNS
records. Unknown mappings fail, matching mappings are accepted, and a
static/dynamic disagreement returns `alias.catalog.mapping_conflict`.
Consensus revalidates the canonical text and numeric ID pair during execution.

## Read-only inspection

SNS retains read-only record and policy inspection:

```bash
iroha app sns registration \
  --namespace account-alias \
  --literal merchant@banka.paynet

iroha app sns policy --suffix-id <u16>
```

The corresponding Torii reads are
`GET /v1/sns/names/{namespace}/{literal}` and
`GET /v1/sns/policies/{suffix_id}`. A policy ID selects a live on-chain
resource policy; it is not an external textual suffix catalog.

Alias acquisition, repair, renewal, primary changes, rebinding, and auto-renew
configuration are not SNS mutation routes. Use the signed planner and ordinary
transaction flow under `iroha app alias`, as documented in
[`registrar_api.md`](./registrar_api.md). The exact persisted SNS and alias
types are documented in [`registry_schema.md`](./registry_schema.md).
