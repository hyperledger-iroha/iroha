# Same-network engineering issuer composer

`provisioning.rs` is an explicit Linux-only helper in the `integration_tests`
`sumeragi` harness. It creates a fresh issuer journal, fresh P-256 Enrollment signer
and Ed25519 SchemeOperator signer, and a private Torii configuration for the
controller's current registered Scheme. Its public engineering catalog root is
fixed; a different root is refused. It supplies no bank KYC authority.

The helper consumes `KAGEMUSHA_ENGINEERING_ISSUER_REQUEST` (an absolute private
JSON file) and `KAGEMUSHA_ENGINEERING_ISSUER_REQUEST_SHA256` (the independently
selected exact SHA-256). The engineering acceptance controller selects actual
private inputs through `issuer-composer-input.json` and invokes the same-source
compiled harness with:

```text
kagemusha_network_setup::continuation::issuer::provisioning::prepare_same_live_engineering_issuer_configuration --exact --ignored --nocapture --test-threads=1
```

The request is closed and uses schema
`iroha.kagemusha.engineering-issuer-provisioning.v1`. Every original reference is
exactly `{ "path": "<absolute path>", "sha256": "<lowercase SHA-256>" }`.
The controller bounds the request to 64 KiB and compares its three controller
originals before invocation. It retains only references and hashes, the exact
command, selected binary, PID, raw streams and natural completion; private request
and configuration bodies are not copied into controller receipts. A direct manual
invocation does not satisfy that controller execution ownership.
Its fields are:

- `issuer_request`, `setup`, `registered`: exact current private controller
  originals. The request supplies the verifier pack; its hash, Scheme and manifest
  must agree with registration. The installed pack authenticates its own artifacts.
- `peer_index`: one of the four retained validators, `0` through `3`.
- `private_parent`, `output_name`: existing private directory inside this
  same-source checkout's `target/qualification` and one exact fresh child name.
  External, traversing or aliased parents and non-child names are refused before
  signing keys, journal or output are created. Existing or partial output is
  refused and preserved. The parent remains under native custody through
  publication; this observation does not reserve the absent child.
- `scope_hex`: explicit nonzero lowercase 32-byte journal identity.
- `application_scopes`: exactly `audience`, `release` and `service_origin`, each
  an original reference. Supply the raw application-session `audience` UTF-8
  bytes (1–512 bytes, no surrounding whitespace or control characters), decoded
  `releaseOriginalBase64` bytes (1–16,384 bytes), and exact HTTPS `requestOrigin`
  UTF-8 bytes (1–2,048 bytes). The origin must already be canonical, without a
  path, credentials, query, fragment, trailing slash or redundant default port.
  No bytes are trimmed, normalized or replaced. The composer uses the Native
  permit digest helper with the `Fi`, `Release` and `ServiceOrigin` roles and
  retains exact copies as `audience.bin`, `release.bin` and `service-origin.bin`.
  Use these same originals in the independently authenticated application release
  and session authority. These inputs select scope DATA, never session authority
  or eligibility. The retired three caller-provided digest fields are rejected.
- `observation_endpoint`, `observation_credential`, `observation_tls_root`: actual
  HTTPS middleware URL, existing private bearer-token file, and either `null` or
  one original reference to a private DER TLS root, at most 16 KiB. Native TLS
  roots and hostname verification remain enabled. The composer does not install
  trust into the OS.
- `worker`: every current `KagemushaEnrollmentWorker` field, including explicit
  `attestation_root: null` for Android. Python/OpenSSL originals are bounded to
  256 MiB; the verifier zipapp is bounded to 8 MiB. Paths and SHA-256 values select
  actual reviewed programs; hashes alone do not establish program provenance.
  `exchange_timeout_ms` is `1..=300000`.
- `android`: `package_name`, `minimum_package_version`,
  `signing_certificates_der` (one to four original references), `roots` (one to
  eight `{ "family": "google-rsa" | "google-ca1", "spki": <original> }`
  objects), and `patch_floor_yyyymm`. These must be actual selected certificate
  and SPKI originals. The composer hashes them and validates the model policy;
  it does not authenticate arbitrary trust-root selections or generate evidence.
- `maximum_response_ms`: `1..=60000`; `challenge_lifetime_ms`: positive.
  Regulatory controls remain off and attestation lease lifetime is zero.

Prepare the actual worker installation through the maintained E1 zipapp builder
and `iroha-wallet-e1-store-init` / `iroha-wallet-e1-denylist` tools. Existing private
`wallet-e1.generation`, `wallet-e1.sqlite3` and `wallet-e1-denylist.custody` are
required; denylist custody has the worker's 12 MiB bound. The serving worker owns
schema, counter, generation and revocation validation. No placeholder worker or
store is created here.

The fresh `operator/` directory contains `template.norito`, raw `signer.key` and
`current.json` with an empty decision table. The maintained
`kagemusha_eligibility_operator` and `scripts/kagemusha_eligibility_https.py` serve
that directory when explicitly started with reviewed runtime inputs. Empty state
refuses every subject. Only the authenticated operator may publish real current
eligibility decisions; this composer never publishes an approval.

`issuer.toml` is written before `issuer-selection.json` is published next to the
selected controller request. Every selected input and the emitted configuration
are retained and checked before publication; publication never overwrites.
Private partial outputs survive failure. The controller then restarts the selected
existing validator only after the exact composer test has completed naturally and
passed its source/input checks. A visible selection after failure or uncertain
completion is retained for reconciliation and cannot authorize that restart. The
controller checks unchanged registered history. Configuration parsing
here checks shape and selected originals; normal node startup still performs the
complete configuration, TLS, signer and worker validation.

The composer does not contact middleware or perform enrollment. Issuer startup
therefore establishes neither online enrollment nor physical-phone acceptance.
Current engineering wallet acceptance uses its own explicit engineering issuer;
it must not be described as enrollment through this live Torii route.

Ordinary controls are selected by
`kagemusha_network_setup::continuation::issuer::provisioning::tests::` in the same
harness. They use DATA fixtures, do not start a native worker, and cannot qualify
Linux startup, online enrollment or a physical phone. Run the Linux composition
and full current campaign separately before claiming those outcomes.
