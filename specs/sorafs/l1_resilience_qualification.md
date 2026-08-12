---
title: SoraFS L1 Resilience Qualification
summary: Payload-free holistic resilience and disaster-recovery attachment for the qualified four-validator topology.
---

# SoraFS L1 resilience and disaster-recovery qualification

`scripts/check_sorafs_l1_resilience_qualification.py` validates one holistic
resilience receipt for the already-qualified four-validator production
topology. This receipt is a qualification attachment, not a readiness lane or
a new foundational prerequisite ID. It changes neither the canonical 17-lane
inventory nor the fixed nine-ID external-software-signer envelope.

The checker requires exactly one receipt, one artifact root, and the exact
summary emitted by `check_sorafs_l1_deployment_qualification.py`. The receipt
and every artifact repeat the deployment identifier, environment, exact Taira
network, chain ID, numeric chain discriminator, and all nine topology-binding
fields. A different topology-summary file, even if it
describes equivalent JSON, therefore fails the digest binding.

## Closed requirement inventory

The receipt contains these requirements in this exact order:

1. network partition recovery;
2. consensus view change;
3. validator restart;
4. Torii restart;
5. provider restart;
6. simultaneous peer submission;
7. signer rotation;
8. root rotation;
9. catalog rotation;
10. gateway failover;
11. Governance DAG failover;
12. stale-fork rejection;
13. crash recovery;
14. identical post-recovery peer state;
15. at least one repair outcome;
16. at least one settlement outcome;
17. backup restore;
18. release rollback; and
19. package yank.

Each inventory row binds an archive-relative path, exact SHA-256, and fresh
capture timestamp. The referenced JSON artifact repeats its requirement
identity and capture timestamp. Consequently, swapping two otherwise valid
path-and-digest pairs fails closed. Canonical file identities, paths, digests,
and requirement identifiers must all be unique. Unknown, missing, duplicated,
stale, tampered, symlinked, or secret-bearing inputs are rejected.

All artifacts use
`sorafs.l1.resilience_qualification.artifact.v1` and contain only:

- their requirement identity;
- the production deployment and topology binding;
- capture time, `result=passed`, and a positive observation count;
- `payload_included=false`; and
- for `identical_post_recovery_peer_state` only, the canonical ordered
  `taira-validator-1` through `taira-validator-4` identities carrying the same
  finalized-state SHA-256.

Private evidence, transaction bodies, credentials, tokens, keys, PII, and log
payloads stay outside the receipt and summary.

## Required external software authentication

Qualifying receipts use this schema-closed authentication object:

```json
{
  "kind": "external-ed25519",
  "algorithm": "ed25519",
  "backend": "software",
  "service_id": "<isolated-signer-service>",
  "administrator_id": "<independent-administrator>",
  "key_revision": 1,
  "policy_revision": 1,
  "policy_digest_sha256": "<nonzero-lowercase-sha256>",
  "public_key_fingerprint_sha256": "<trusted-key-sha256>",
  "signature_hex": "<64-byte-lowercase-ed25519-signature>"
}
```

The service and administrator identifiers must be canonical, production-marked,
and distinct. Key and policy revisions are positive, and the policy digest is
nonzero. Local, HSM, test-marked, incomplete, or substituted authentication is
rejected; there is no configuration-qualified compatibility mode.

The signature covers the domain
`iroha:sorafs:l1-resilience-qualification:v1\0` followed by canonical JSON of
the receipt with `authentication.signature_hex` omitted. The checker verifies
it only against the separate operator-supplied `--trusted-public-key-hex`.
Private signing material is never accepted. Only this trusted external
software-signature path can emit `status=evidence-qualified`.

## Promotion consumption

Promotion consumes the exact evidence-qualified summary through the dedicated
`--resilience-qualification-summary` input and independently re-verifies the
receipt with
`--resilience-qualification-signer-public-key-hex`. The consumer reconstructs
the receipt from its deployment, topology, timestamp, 19 artifact bindings,
and authentication object; recomputes `canonical_receipt_sha256`; and verifies
the domain-separated Ed25519 signature. Summary status booleans and declared
fingerprints are never trusted by themselves.

The foundational `prepare` and `finalize` commands reopen and authenticate the
same summary. They place its exact summary digest, receipt digests, receipt
timestamp, and resilience signer fingerprint in a
`resilience_qualification` binding beside `topology_qualification`. The
binding also carries the software backend, service and administrator
identities, key and policy revisions, and policy digest. The existing
foundational external software signature therefore covers the binding without
inventing a tenth prerequisite ID. The aggregate requires exact equality
between that signed binding and its separately reviewed resilience input.
Missing, non-software, locally signed, stale, tampered, wrong-key, wrong-deployment, or
wrong-topology summaries block both foundational preparation and promotion.

The production runner snapshots resilience as a separate replay input:
topology summary + signed topology envelope + resilience + signed L1 lane
evidence inventory + foundation + 17 lane summaries equals 22 immutable inputs.
It still emits `summary_file_count=17` and `recognized_summary_count=17`; the
resilience and signed-inventory attachments are neither passed through
`--evidence` nor entered in the lane registry.

## Invocation

Copy
`scripts/examples/sorafs_l1_resilience_qualification.args.example`, replace all
template values with one real capture, and run:

```bash
python3 scripts/check_sorafs_l1_resilience_qualification.py \
  @scripts/examples/sorafs_l1_resilience_qualification.args.example
```

The response file requires
`--trusted-public-key-hex <operator-trusted-raw-ed25519-public-key>`; replace
its placeholder before use. After genuine external authentication, pass the resulting summary and the
same reviewed public key to foundational `prepare`, foundational `finalize`,
and the production-readiness runner through their dedicated resilience flags.
