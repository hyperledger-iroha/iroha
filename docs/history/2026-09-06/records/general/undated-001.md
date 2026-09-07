# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-43779deb89d3b249109e5bc7ecc16109455013337bc505e93b0239d3a3547d86"></a>

<!-- Original context: Status -->
# Status

Last updated: 2026-09-05

Current signing and key-custody qualification is provider-neutral. Iroha has no
HSM- or PKCS#11-specific product mode, API, configuration, schema, or named
release gate. Software signing and custody are valid for every Iroha
deployment; no deployment or release gate requires an HSM. Custody-provider
selection is outside Iroha. Deployments may place a provider-neutral signing
service behind their own custody boundary without exposing that implementation
to Iroha; the custody implementation is not an Iroha profile or capability.
Authentication, runtime-only
non-persistent secret handling, rotation and revocation, sealed-CAS durability
where state is retained, failover and recovery evidence, and independent review
remain mandatory. Dated entries below do not define current work; superseded pre-release
protocol history is intentionally omitted.

This provider-neutral statement covers general node, client, and consensus
signing custody. It does not weaken KAGEMUSHA's separate optional monetary
device contract: offline spending remains unavailable unless a device implements
the complete qualified non-forking hardware service described below.



<a id="record-2d1e9e1289445d822eeca1600f420530d5265cadaafeeeafbecdd6d28094819c"></a>

<!-- Original context: Roadmap -->
# Roadmap

Last updated: 2026-09-05

This roadmap is the public, high-level view of current Hyperledger Iroha work.
Completed history lives in [`status.md`](undated-001.md#record-43779deb89d3b249109e5bc7ecc16109455013337bc505e93b0239d3a3547d86).

Signing and key-custody release gates are provider-neutral. Iroha will not add
HSM- or PKCS#11-specific product modes, APIs, configuration, schemas, or named
release gates. Software signing and custody are valid for every Iroha
deployment; no deployment or release gate requires an HSM. Custody-provider
selection remains outside Iroha. A deployment may place the provider-neutral
signing service behind its own custody boundary; the custody implementation is
not an Iroha profile or capability. Qualification still requires
authenticated provider binding, runtime-only non-persistent secret handling,
rotation and revocation, sealed-CAS durability where state is retained, failover
and recovery evidence, and independent review.

This provider-neutral policy covers ordinary node, client, and consensus
signing. The optional KAGEMUSHA monetary device service is deliberately
separate: offline spending requires a governed, qualified non-forking hardware
profile and never permits software fallback.



<a id="record-11c49efd369322ecc4d3b01c2db1484e14fcde550f8f1c2ec38deafaffe888ee"></a>

<!-- Original context: Roadmap / Generated HF isolated execution -->
## Generated HF isolated execution



<a id="record-b524eca1ab54a3b04e2744f0f6861950360059c5732e14271091dafb32193edb"></a>

- Keep generated Hugging Face imports storage-only in V1. Any future execution
  feature must be a new signed and pinned Inrou guest admitted through the fixed
  authenticated PortableVM corridor. Never restore the retired host Python
  runner, provider credential or bridge, generated-HF Torii proxy, broker
  operation, warmth heartbeat, compatibility decoder, or host-local execution
  path.



<a id="record-58c29285ca2b4d0d4867fc8f0be753bfc0e9b4f4ad95ca986a02046a56c39d5e"></a>

<!-- Original context: Roadmap / Nexus topology model closure -->
## Nexus topology model closure



<a id="record-720186043ef576b0a1765cd5ab737a0889d787800dd0a1483b6e8be4ab6bbbe2"></a>

- Replace overloaded account-route strings with typed namespace and dataspace
  match fields. Preserve `MusubiNamespaceBindingV1` as an explicit
  namespace-to-home-dataspace binding and stop inferring either identity from a
  shared textual suffix.


<a id="record-b014cec3380ec155257ac3fd2df781ff66ad516769c237330aa4b1c6c5291e55"></a>

<!-- Original context: Roadmap / Exact network identity completion -->
## Exact network identity completion



