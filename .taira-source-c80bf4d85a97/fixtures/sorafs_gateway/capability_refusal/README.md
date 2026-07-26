SoraFS Gateway Capability Refusal Fixtures
==========================================

These fixtures document the canonical refusal responses exercised by the
capability conformance suite (`SF-5c`). Each scenario underpins automated tests
and the operator self-certification kit so gateways, SDKs, and monitoring
pipelines agree on the surface of deterministic refusals.

File layout:

- `scenarios.json` – ordered list of refusal cases with stable identifiers,
  descriptions, HTTP status codes, machine error codes, exemplar detail
  payloads, and relative paths to scenario-specific fixtures.
- `C*/request.json` – canonical request metadata (method, path, headers, and
  notes) that reproduces the refusal.
- `C*/response.json` – expected HTTP status and JSON body returned by the
  gateway under the refusal.
- `C*/gateway.json` – mutations to apply to the canonical fixture bundle in
  `../1.0.0` when spinning up a mock gateway for the scenario.

Gateway operators can replay any scenario with `sorafs_fetch` using the bundled
manifest and mock envelopes shipped in `../1.0.0` together with the scenario
mutations captured above.

Developers may also load `scenarios.json` when rendering dashboards or
troubleshooting guides.
