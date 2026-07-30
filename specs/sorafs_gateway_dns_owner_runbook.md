---
title: SoraFS Gateway & DNS Owner Runbook
summary: Operational checklist for governed SoraDNS and gateway cutovers.
---

# SoraFS Gateway & DNS Owner Runbook

This repository owns the runtime, SoraDNS, GAR, gateway-binding, and probe
contracts used in a governed cutover. It does not build or publish the public
Iroha documentation site. Public documentation is maintained in the sibling
`iroha-docs` repository and published at <https://docs.iroha.tech/>.

## Roles

| Scope | Primary owner | Required evidence |
| --- | --- | --- |
| SoraDNS zone and resolver bundle | Networking/Ops | Approved change ticket, signed zone bundle, resolver reload log |
| GAR and gateway binding | Property owner | Signed content manifest, GAR envelope, binding summary, rendered headers |
| Verification and rollback | QA/Security | Probe report, telemetry snapshot, previous signed zone and binding |

The property owner supplies a signed content manifest produced by its own
release system. Do not copy deployment credentials, signing keys, or
repository-specific build output into this repository.

## Pre-cutover

1. Record the governed cutover window, hostname, SoraDNS name, manifest digest,
   previous binding, and rollback owner in the change ticket.
2. Generate and review the deterministic host and ACME plans:

   ```bash
   cargo xtask soradns-hosts \
     --name <name> \
     --json-out artifacts/soradns/host_summary.json

   cargo xtask soradns-acme-plan \
     --name <name> \
     --json-out artifacts/soradns/acme_plan.json
   ```

3. Generate the gateway binding from the signed manifest:

   ```bash
   cargo xtask soradns-binding-template \
     --manifest <signed-manifest.json> \
     --alias <name> \
     --hostname <hostname> \
     --route-label production \
     --json-out artifacts/sorafs/gateway.binding.json \
     --headers-out artifacts/sorafs/gateway.headers.txt
   ```

4. Verify the binding and archive the command output with the ticket:

   ```bash
   cargo xtask soradns-verify-binding \
     --binding artifacts/sorafs/gateway.binding.json
   ```

5. Build the signed zonefile skeleton with
   `scripts/sns_zonefile_skeleton.py`, including the effective time, GAR
   digest, freeze metadata, and previous-zone rollback reference.
6. Rehearse the change against staging resolvers and run
   `ci/check_sorafs_gateway_probe.sh` against the candidate hostname.

## Cutover

1. Freeze the reviewed manifest, GAR, binding, header, and zone artifacts.
2. Publish the signed zone bundle and reload each authoritative resolver.
3. Verify DNSSEC answers from every resolver and confirm the expected
   `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, and
   `Sora-Route-Binding` headers.
4. Run `ci/check_sorafs_gateway_probe.sh` and archive its JSON report.
5. Watch GAR violations, gateway refusals, certificate expiry, and resolver
   proof-age telemetry for at least 30 minutes.

## Rollback

Rollback immediately when the signed binding does not match the served
manifest, proof verification fails, DNS answers diverge, or the gateway probe
fails.

1. Restore the previous signed zone bundle and reload resolvers.
2. Restore the previous GAR and gateway binding atomically.
3. Re-run DNSSEC and gateway probes against the rollback target.
4. Attach the before/after evidence and telemetry snapshot to the incident.

Never improvise a new manifest, GAR, or zone signature during rollback. Use the
previously reviewed artifacts recorded in the change ticket.

## Evidence retention

Keep the following together under the operator-controlled evidence store:

- approved ticket and cutover window;
- signed manifest, GAR, binding, headers, and zone bundle;
- host, ACME, binding-verification, DNS, and gateway-probe reports;
- telemetry snapshots and resolver reload logs;
- previous artifacts and the exercised rollback result.

Repository-local examples and fixtures may be used to validate the formats, but
they are not production deployment inputs.
