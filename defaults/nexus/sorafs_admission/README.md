# SoraFS Provider Admission Envelopes

Place governance-signed `ProviderAdmissionEnvelopeV1` files in this directory to enable Torii's
SoraFS discovery cache when running the Nexus (Iroha 3) profile. The sample Nexus config points to
this path via `sorafs.discovery.admission.envelopes_dir`.

The directory ships empty so operators can copy envelopes produced by `sorafs_manifest_stub
provider-admission` without modifying the repository.

The adjacent developer config trusts only the deterministic fixture council key with a one-key
threshold. Replace `trusted_council_keys` and `signature_threshold` with the deployment's real
governance council before adding envelopes or exposing discovery; never reuse a node identity or
provider advert key as a council trust root.
