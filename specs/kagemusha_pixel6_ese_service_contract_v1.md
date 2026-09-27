# Pixel 6 internal eSE service conformance contract

Pixel 6 is a required phone profile. Its observed Android Keystore one-use
restriction is software-enforced, so a KAGEMUSHA V1 monetary profile on this
device requires a separately provisioned **internal** eSE service. The stock
Pixel 6 is not qualified by this document or by the transport model test.

The secure-element owner must supply an installed applet instance, its exact
5–16-byte AID and protocol version, an ARA-M access rule for that AID and the
release APK signing-certificate hash, and the applet's hardware attestation
chain and enrollment procedure. The current Android SDK default AID is
`F04F444A524E0001`; changing it requires an explicit release policy and matching
provisioning. A reader feature flag, successful SELECT, or a 96-byte capability
frame is discovery evidence, not a monetary qualification.

The applet is selected over an embedded `eSE` reader through Android OMAPI. Its
short-APDU transport is implemented by
`KagemushaSecureElementApduEndpointV1`: CLA `80`, capabilities INS `11`, begin
`12`, write `13`, commit `14`, read `15`, and transport abort `16`. Begin carries
the command-frame length and SHA-256; writes are indexed 224-byte chunks;
commit returns the response length and SHA-256; reads return indexed chunks.
Every response must end in ISO 7816 status `9000` before the SDK accepts its
data. The enclosed ABI-23 command and response frames, sizes, operation codes,
and authentication rules are fixed by `specs/kagemusha_device_bridge_v1.md`.

The provisioned service must demonstrate all of the following with exact
canonical Core-generated operation bodies, not host-chosen placeholder bytes:

1. Operation 5 reserves one outbox slot and one exact-next predecessor/successor
   binding under a durable, independent 32-byte operation ID. Operation 7
   consumes that predecessor and stores one terminal outcome atomically with
   its signed certificate. Its success frame is emitted only after persistence.
2. If operation 7 commits but the response is lost, INS `16` may clear only
   incomplete APDU transfer state. Operation 8, selected by the original ID and
   original public-input digest, returns the original retained outcome and
   certificate byte-identically. It must not advance the monetary counter or
   sign a replacement terminal certificate. An exact operation-7 retry can only
   recover that same committed result.
3. A second operation ID, candidate, or input digest that tries to consume the
   already reserved or consumed predecessor returns a closed conflict/stale
   status with empty payload and authenticator. It must never yield a second
   success or terminal signature, including after app restart and device reboot.
4. Recovery must still work after package update under the authorized signing
   identity and hardware-epoch rotation, subject to the retained historical
   binding rules in `specs/kagemusha_device_sender_v1.md`. Recovery cannot
   accept a substituted network, asset, lane, epoch, release, policy, or key.

An OEM conformance package must include canonical operation 5/7/8 fixtures,
the competing operation-5 fixture for the *same* predecessor, expected terminal
certificate and record digests, signed attestation/credential chain, and a
power-loss test transcript that shows the physical counter value and retained
outcome before and after restart. The verifier must check the applet signature,
attested key, app-release policy, exact operation/request binding, and original
certificate bytes independently of the applet's status codes. The package must
identify the device model, build fingerprint, SE firmware/applet versions,
provisioning authority, and exact APK signing certificate used for ARA-M.

`KagemushaOmapiDiscoveryDeviceTest` has a stock-phone diagnostic mode with no
instrumentation arguments. It completes after bounded OMAPI discovery and may
report `ONLINE_ONLY`; that is the expected result when the project applet or
access rule has not been provisioned. To require the provisioned Pixel 6
profile, run that test with these exact instrumentation arguments:

| Argument | Required value |
| --- | --- |
| `kagemusha.requireProvisionedPixel6Ese` | `true` |
| `kagemusha.pixel6EseReader` | The exact embedded reader name, such as `eSE1` |
| `kagemusha.pixel6EseAid` | The 5–16-byte release-policy applet AID as ASCII hexadecimal |
| `kagemusha.pixel6EseHardwarePolicyId` | The expected 32-byte hardware-policy ID as ASCII hexadecimal |
| `kagemusha.pixel6EseQualificationReportDigest` | The expected 32-byte qualification-report digest as ASCII hexadecimal |

The gate fails on non-Google/oriole hardware, a malformed or missing pin, a
removable or different reader, denied or absent AID, incomplete ABI-23
capabilities, or a policy/report digest mismatch. The exact reader and AID are
passed to OMAPI discovery; `AVAILABLE` is returned only after the full
capability frame is structurally admitted. Pin values must come from the
approved provisioning record, not from values observed in an untrusted
discovery run. This gate checks access and capability admission only; the
operation-5/7/8 physical no-fork and attestation evidence above remain required
before monetary qualification.

`KagemushaProvisionedEseTransportContractV1Test` exercises the loss/recovery
APDU seam against a deterministic test-only model. Its synthetic payloads and
authenticator are deliberately not valid monetary material; passing it does
not qualify an applet, device, or production offline spending profile.
