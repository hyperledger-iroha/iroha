# Superseded Taira checkpoint before release 78

Retained verbatim from the previous status view; this is historical evidence,
not release readiness.

The latest Taira rollout rolled back because QEMU 10.0.11 rejected the required `exit-with-parent` option. QEMU 11.0.2 from Debian backports and its rebuilt immutable runtime closure pass actual four-slot KVM, namespace, identity, capability, QMP and empty-shutdown checks. Packaging now rejects unsupported QEMU before publishing a closure; startup reports the actual child exit and bounded stderr. Three native diagnostic tests and 28 packager tests pass. These probes do not qualify workload canaries, live finality or the application rollout.

Taira candidate convergence, canaries and four ordered restart proofs now precede public cutover. The real Inrou producer/consumer check order is corrected; 29 focused endpoint, prepared-custody, replay and HTTP-producer tests pass. Rollback now reconciles stopped-owner cgroups and exact owned firewall rules under the slot lock, and cached stop evidence rechecks their absence. All 81 maintained CLI gates pass, including ten stopped-owner regressions. The current host residue has been retired under the native control and owner locks; no live chain or application rollout is qualified.

[Executable build identity](docs/build_identity.md) removes revision stamping from shared Core, Torii and telemetry libraries. A controlled revision-only warm build completed in 25.073 seconds with all six production/test shared artifacts cached. The identity changes pass 31 native and 32 build-support tests; conflicting local/release metadata fails before compilation. This local measurement does not predict a full release build or rollout duration. The maintained retry command reuses completed build/transfer work and requires four candidate origins before retiring an attempt. Its 37 retry tests and the prior 18 capacity tests remain bounded offline evidence.

