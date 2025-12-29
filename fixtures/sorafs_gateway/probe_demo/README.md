# Gateway Probe Demo Fixtures

These fixtures back the `ci/check_sorafs_gateway_probe.sh` drill harness and the
`scripts/telemetry/run_sorafs_gateway_probe.sh` wrapper. They simulate a gateway
responding with the expected Sora headers and a compact GAR signed by the
`demo-gar` key.

- `headers_demo.txt` – captured HTTP headers returned by the demo gateway.
- `demo.gar.jws` – compact Gateway Authorization Record (`typ=gar+jws`)
  authorising the host patterns referenced in the headers.
- `gar_pub.hex` – hex-encoded Ed25519 public key matching the `kid=demo-gar`
  header embedded in the GAR. Use this value with `--gar-key demo-gar=<hex>`.

The artefacts are intentionally simple (no private keys are stored) so CI can
exercise the probe + PagerDuty/rollback pipeline without hitting live gateways.
