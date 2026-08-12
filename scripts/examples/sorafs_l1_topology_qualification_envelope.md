# L1 topology qualification detached-signature example

This is the public, no-private-key workflow for constructing the signed
companion to an exact SoraFS L1 topology qualification summary. Replace every
angle-bracket value and use a private runtime evidence directory. The external
software Ed25519 signer receives only `topology-signing.payload` and returns
exactly 64 raw signature bytes in `topology-signature.bin`.

```bash
umask 077
install -d -m 0700 /runtime/evidence

trust_args=(
  --topology-qualification-summary /runtime/evidence/l1-topology-qualification.json
  --deployment-id <REVIEWED-PRODUCTION-DEPLOYMENT-ID>
  --environment production
  --now-unix <REVIEWED-VALIDATION-UNIX>
  --max-topology-qualification-review-age-secs 1209600
  --topology-qualification-verification-public-key-hex <TRUSTED-TOPOLOGY-ED25519-PUBLIC-KEY-HEX>
  --topology-qualification-signer-service-id <TOPOLOGY-SOFTWARE-SIGNER-SERVICE-ID>
  --topology-qualification-signer-administrator-id <INDEPENDENT-TOPOLOGY-SIGNER-ADMINISTRATOR-ID>
  --topology-qualification-signer-key-revision <POSITIVE-TOPOLOGY-KEY-REVISION>
  --topology-qualification-signer-policy-revision <POSITIVE-TOPOLOGY-POLICY-REVISION>
  --topology-qualification-signer-policy-digest-hex <NONZERO-TOPOLOGY-POLICY-SHA256>
)

# The runtime evidence directory must remain owned by the invoking operator;
# do not run concurrent writers against these output names.

python3 scripts/build_sorafs_topology_qualification_envelope.py prepare \
  "${trust_args[@]}" \
  --reviewed-at-unix <REVIEWED-TOPOLOGY-UNIX> \
  --prepared-out /runtime/evidence/topology-envelope.prepared.json \
  --signing-payload-out /runtime/evidence/topology-signing.payload

# Submit only topology-signing.payload to the independently administered
# external software Ed25519 signer. Do not pass a private key to this tool.

python3 scripts/build_sorafs_topology_qualification_envelope.py finalize \
  "${trust_args[@]}" \
  --reviewed-at-unix <REVIEWED-TOPOLOGY-UNIX> \
  --prepared /runtime/evidence/topology-envelope.prepared.json \
  --signature-file /runtime/evidence/topology-signature.bin \
  --envelope-out /runtime/evidence/l1-topology-qualification.envelope.json

python3 scripts/build_sorafs_topology_qualification_envelope.py verify \
  "${trust_args[@]}" \
  --topology-qualification-envelope /runtime/evidence/l1-topology-qualification.envelope.json \
  --verification-out /runtime/evidence/topology-verification-a.json

python3 scripts/build_sorafs_topology_qualification_envelope.py verify \
  "${trust_args[@]}" \
  --topology-qualification-envelope /runtime/evidence/l1-topology-qualification.envelope.json \
  --verification-out /runtime/evidence/topology-verification-b.json

cmp /runtime/evidence/topology-verification-a.json \
  /runtime/evidence/topology-verification-b.json
```

Every output path must be new. The tool rejects symlinks, hardlinks, unsafe
parent traversal, non-canonical prepared JSON, changed summary bytes, changed
trust values, stale review clocks, and invalid signatures. The verification
output contains only the authenticated public topology binding; it never
contains the detached signature or signing payload.

The signing payload is the `prepare` completion marker. If the host or process
stops abruptly and only the prepared JSON exists, remove it and rerun
`prepare`; never hand an incomplete pair to the signer. This standalone
`verify` authenticates the topology envelope only. The foundational and
aggregate readiness flows remain authoritative for rejecting reuse of
resilience, lane, or promotion signer keys and administrator identities.

Repository-root `artifacts/*` is ignored. An ignored file is not durable
release evidence and can silently outlive its schema. Always regenerate and
reverify these files from the exact reviewed summary in the protected runtime
evidence directory before a release run.
