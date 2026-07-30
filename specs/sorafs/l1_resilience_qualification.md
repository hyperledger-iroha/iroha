---
title: SoraFS L1 Resilience Qualification
summary: Payload-free holistic resilience and disaster-recovery attachment for the qualified four-validator topology.
---

# SoraFS L1 resilience and disaster-recovery qualification

`scripts/check_sorafs_l1_resilience_qualification.py` validates one holistic
resilience receipt for the already-qualified four-validator production
topology. This receipt is a qualification attachment, not a readiness lane or
a new foundational prerequisite ID. It changes neither the canonical 17-lane
inventory nor the fixed nine-ID external-HSM envelope.

The checker requires exactly one receipt, one artifact root, and the exact
summary emitted by `check_sorafs_l1_deployment_qualification.py`. The receipt
and every artifact repeat the deployment identifier, environment, and all five
topology-binding fields. A different topology-summary file, even if it
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
- for `identical_post_recovery_peer_state` only, four unique validator IDs
  carrying the same finalized-state SHA-256.

Private evidence, transaction bodies, credentials, tokens, keys, PII, and log
payloads stay outside the receipt and summary.

## Local versus externally authenticated receipts

A local capture uses this exact authentication object:

```json
{
  "kind": "local",
  "algorithm": null,
  "public_key_fingerprint_sha256": null,
  "signature_hex": null
}
```

Successful local validation emits `status=configuration-qualified`,
`live_evidence_recognized=false`, `externally_authenticated=false`,
`promotion_eligible=false`, and `readiness_lane_count_delta=0`. It must never
be represented as genuine deployment evidence.

An operator may instead provide `kind=external-ed25519`, `algorithm=ed25519`,
the trusted public-key fingerprint, and an exact 64-byte lowercase-hex
signature. The signature covers the domain
`iroha:sorafs:l1-resilience-qualification:v1\0` followed by canonical JSON of
the receipt with `authentication.signature_hex` omitted. The checker verifies
it only against the separate operator-supplied `--trusted-public-key-hex`.
Private signing material is never accepted. Only that trusted-signature path
can emit `status=evidence-qualified` and make the attachment eligible for
promotion review.

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
existing foundational HSM signature therefore covers the binding without
inventing a tenth prerequisite ID. The aggregate requires exact equality
between that signed binding and its separately reviewed resilience input.
Missing, local-only, stale, tampered, wrong-key, wrong-deployment, or
wrong-topology summaries block both foundational preparation and promotion.

The production runner snapshots resilience as a separate replay input:
topology + resilience + foundation + 17 lane summaries equals 20 immutable
inputs. It still emits `summary_file_count=17` and
`recognized_summary_count=17`; the resilience attachment is neither passed
through `--evidence` nor entered in the lane registry.

## Invocation

Copy
`scripts/examples/sorafs_l1_resilience_qualification.args.example`, replace all
template values with one real capture, and run:

```bash
python3 scripts/check_sorafs_l1_resilience_qualification.py \
  @scripts/examples/sorafs_l1_resilience_qualification.args.example
```

For an externally authenticated receipt, append
`--trusted-public-key-hex <operator-trusted-raw-ed25519-public-key>`. The
example intentionally omits this argument and cannot claim live qualification.
After genuine external authentication, pass the resulting summary and the
same reviewed public key to foundational `prepare`, foundational `finalize`,
and the production-readiness runner through their dedicated resilience flags.
