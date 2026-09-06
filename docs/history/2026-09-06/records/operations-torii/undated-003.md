# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-afd4c196f4904b05259ed275a5b02fccbd1d9d2df76b7e070795c7e4201fb0ff"></a>

<!-- Original context: Roadmap / SORA Nexus and Taira -->
- Torii global pipeline-status reads now treat cached routing-plan hints as
  probes: hinted `Queued`, `Approved`, `Committed`, or malformed success
  responses fall through to full fanout, and only terminal hinted statuses can
  short-circuit. This keeps stale retired-lane status caches from hiding newer
  terminal results on active autoscale lanes.


<a id="record-aabedb1bfe45ad785c4e2d7aca30311108539305d8d1d5876f2d962268fe33f2"></a>

- Incoming Torii read and verified-query proxy requests now validate
  ingress-selected lane/dataspace hints against the receiver's current Nexus
  catalogs before local read execution. Active routes still execute locally to
  avoid proxy cascades during transient authority-view skew, but retired-lane
  and lane/dataspace mismatch hints fail as `route_unavailable` with
  `stale_route` diagnostics.

