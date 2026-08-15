# Retired Sumeragi Aggregator Design

This document is a tombstone for the pre-revision-4 collector/aggregator
routing design. Current Sumeragi has no collector set, collector `k`/redundant
send `r` parameters, `CollectorPlan`, or `sumeragi::collectors` module.

Every validator sends Prepare, Commit, and timeout votes to the complete frozen
committee. Any validator may aggregate the exact equal-vote quorum
`q = 2f + 1`; NPoS stake affects seat eligibility, not vote weight. See
[`sumeragi.md`](./sumeragi.md) and [`sumeragi_v2.md`](./sumeragi_v2.md) for the
current protocol. Historical telemetry or API field names are compatibility
surfaces only and do not restore executable collector routing or configuration.
