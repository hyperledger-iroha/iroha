---
id: suffix-catalog
title: SNS Policy Inspection
sidebar_label: SNS policy inspection
description: Catalog-free external names and live on-chain SNS policy reads.
---

> Canonical source:
> [`docs/source/sns/suffix_catalog.md`](../../../source/sns/suffix_catalog.md).

# SNS policy inspection

External dataspace, domain, and account-alias names are catalog-free textual
values. There is no built-in `.sora`, `.nexus`, or `.dao` table and no
external suffix mapping to numeric IDs 1, 2, or 3.

The alias planner resolves configured static mappings together with active SNS
records. Unknown mappings fail; matching mappings are accepted; conflicting
static and dynamic mappings fail with `alias.catalog.mapping_conflict`.
Consensus revalidates the canonical text and numeric `DataSpaceId` pair.

SNS exposes read-only record and live-policy inspection:

```bash
iroha app sns registration \
  --namespace account-alias \
  --literal merchant@banka.paynet
iroha app sns policy --suffix-id <u16>
```

Torii exposes the matching
`GET /v1/sns/names/{namespace}/{literal}` and
`GET /v1/sns/policies/{suffix_id}` reads. A numeric policy ID is an on-chain
resource-policy selector, not a textual suffix catalog.

Use `iroha app alias` for signed setup and lifecycle planning. Torii does not
provide SNS mutation routes.
