# SoraFS Provider Admission Envelopes

Place governance-signed `ProviderAdmissionEnvelopeV1` files in this directory to enable Torii's
SoraFS discovery cache when running the Nexus (Iroha 3) profile. The sample Nexus config points to
this path via `torii.sorafs.admission_envelopes_dir`.

The directory ships empty so operators can copy envelopes produced by `sorafs_manifest_stub
provider-admission` without modifying the repository.
