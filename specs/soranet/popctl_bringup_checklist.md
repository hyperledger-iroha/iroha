---
title: SNNet-15A1 PoP Automation Checklist
summary: Bring-up guide for the `soranet-popctl` tooling that seeds PoP configuration templates, validates promotion gates, and verifies health prior to traffic enablement.
---

SNNet-15A1 introduces the `soranet-popctl` CLI so Edge Infrastructure teams can
promote new PoPs deterministically. The tooling packages three workflows:

- **Template generation** – emit a Norito-backed JSON template with rack layout,
  service inventory, sigstore policy, and promotion stages wired to the default
  SNNet phases.
- **Configuration validation** – ensure operator edits preserve required
  services, rotation policies, and sigstore constraints before committing to git
  or shipping to the PXE pipeline.
- **Health evaluation** – compare the orchestrator health feed against the PoP
  contract and refuse promotion when probes are missing or degraded.

## Generating a template

Run the template command with optional overrides for the PoP name, region,
environment, and anycast prefixes. The output is Norito JSON that can be passed
directly into provisioning pipelines.

```bash
cargo run -p soranet-relay --features dev-tools --bin soranet-popctl -- \
  template \
  --name pop-nyc01 \
  --region us-east-1 \
  --environment alpha \
  --asn 64520 \
  --anycast-ipv4 198.51.100.0/24,203.0.113.0/24 \
  --anycast-ipv6 2001:db8:1::/48,2001:db8:2::/48 \
  --out environments/us-east/pop-nyc01.json
```

Every template ships with:

- Rack definitions (`edge`, `core`) and placeholder nodes.
- Service inventory for the edge gateway, resolver, and telemetry plane.
- Sigstore attestation policy pre-filled with Fulcio/Rekor endpoints and
  mandatory annotations for stage tracking.
- Promotion stages (`lab`, `alpha`, `ga`) with sigstore and health gates.
- Health probes with latency thresholds to enforce before accepting traffic.

## Validating edits

Once operators customise the template, validate the payload. The command fails
with a joined error list when required services or policies are missing.

```bash
cargo run -p soranet-relay --features dev-tools --bin soranet-popctl -- \
  validate \
  --config environments/us-east/pop-nyc01.json
```

Current checks cover:

- Presence of an `edge-gateway` service with a non-empty image reference.
- At least one anycast prefix (IPv4 or IPv6).
- Non-empty secret rotation intervals.
- Health checks with explicit expected status labels and declared service targets.
- Promotion gates that target declared services.
- Sigstore endpoints (`fulcio_url`, `rekor_url`).

The validation helper is CI-friendly and intended to gate configuration reviews.

## Evaluating health before promotion

`soranet-popctl` consumes the JSON feed exported by the orchestrator and
compares it against the configured probes. Missing services or unknown status
labels raise errors; degraded checks cause a non-zero exit unless the operator
explicitly allows them.

```bash
cargo run -p soranet-relay --features dev-tools --bin soranet-popctl -- \
  health \
  --config environments/us-east/pop-nyc01.json \
  --report artifacts/health/pop-nyc01.json
```

For brownouts where degraded service is acceptable, add `--allow-degraded`.
The output prints failed or missing checks alongside the worst-case status,
making it easy to record in the promotion log.

## Verifying sigstore attestations and PXE logs

Before promoting a PoP, verify the image attestation bundle and PXE execution
records. The attestation must originate from a trusted issuer and include the
annotations declared in the PoP configuration, while the PXE log must capture a
successful provisioning run for every declared host.

```bash
cargo run -p soranet-relay --features dev-tools --bin soranet-popctl -- \
  attest \
  --config environments/us-east/pop-nyc01.json \
  --bundle artifacts/attestations/pop-nyc01.json \
  --pxe-log artifacts/pxe/pop-nyc01.json
```

The command fails when annotations drift, an issuer is not on the allow list, or
PXE logs show missing/failed hosts. These checks integrate cleanly with CI so
automation can reject promotion attempts that skip attestation review or lack
PXE success artefacts.

## Local input corridors

The first-release CLI accepts only direct regular files and reads each input as
one stable, bounded snapshot. Configuration files are limited to 1 MiB, health
reports to 4 MiB, attestation bundles to 512 KiB, and PXE logs to 8 MiB. Norito
JSON preflight also bounds individual decoded strings (64 KiB for configuration
and 16 KiB for the other inputs), aggregate strings, collection counts,
allocations, and nesting depth before typed decoding starts.

Semantic admission limits retain at most 2,048 configuration entries per
critical collection, 4,096 configured nodes, 2,048 health-report services,
8,192 total health-check results, 2,048 attestation annotations, and 8,192 PXE
events. Service names, node hostnames, health service entries, and health checks
must be unique in their respective scopes. Keep producer batches within these
corridors; an exact-limit document remains valid, while limit-plus-one input is
rejected before promotion evaluation.

## Integration points

- **PXE / Golden image pipeline** – use the generated template to inject
  sigstore annotations and rack layouts. The `sigstore.required_annotations`
  map mirrors the policy enforced by the orchestrator.
- **Git automation** – add `cargo run … validate` to merge-request workflows so
  configuration drift is caught before scheduled rollouts.
- **Observability** – export the health summary to Grafana annotations prior to
  toggling traffic; the CLI output includes precise check identifiers to match
  dashboards.

With the CLI in place, PoP promotion now has an explicit checklist that spans
config templating, secret rotation, sigstore gating, and health verification—
exactly what SNNet-15A1 requires for deterministic rollouts.
