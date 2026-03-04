# SoraGlobal Gateway hardening (SNNet-15H)

The hardening helper captures security/privacy evidence before promoting
Gateway builds.

## Command
- `cargo xtask soranet-gateway-hardening --sbom <path> --vuln-report <path> --hsm-policy <path> --sandbox-profile <path> --data-retention-days 30 --log-retention-days 30 --out artifacts/soranet/gateway_hardening`

## Outputs
- `gateway_hardening_summary.json` — status per input (SBOM, vuln report, HSM policy, sandbox profile) plus retention signal. Missing inputs show `warn` or `error`.
- `gateway_hardening_summary.md` — human-readable rollup for governance packets.

## Acceptance notes
- SBOM and vuln reports must exist; absent inputs downgrade the status.
- Retention over 30 days marks `warn` for review; supply stricter defaults before GA.
- Use the summary artefacts as attachments for GAR/SOC review and incident runbooks.
