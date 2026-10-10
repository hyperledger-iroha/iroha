# NEW first-release E1 app and enrollment policy contract

This proposal fills the two current E1 policy preimage gaps. Its types, tags, domains,
serialization choices and lifetime rules are NEW first-release protocol decisions. They are
not a recovered historical grammar, external approval, or a mapping of any existing opaque
policy digest. Kagemusha has no enable switch or disabled policy variant.

Revision 2026-10-10 applies the single hardware-attestation admission rule of the
[design](kagemusha_single_design_proposal.md) §2.2. The Android app arm pins a package, a
minimum package version and a signer set. The Android enrollment arm pins a root set whose
entries each name a revocation source. No arm carries a hardware selector, a Play Integrity
field or any other vendor online verdict. This is a first-release replacement: the earlier
Android arms have no decoder, and the Model, Python projection, SDK consumers and shared
vectors change together, in the coordinated cutover of design
[§9.1](kagemusha_single_design_proposal.md#91-cutover-sequencing). Until that cutover the
deployed contract of Iroha `67728cc6f3` stays in force. These are app-attestation and
issuer-policy changes: the policy frames are issuer inputs that no ledger instruction,
validation rule or state decodes, and the E1 challenge and credential carry only their
32-byte digests.

The Native issuer selects and retains two unsigned canonical Norito originals:
`KagemushaWalletAppPolicyV1` and `KagemushaWalletEnrollmentPolicyV1`. Public clients carry
their digests in the existing E1 transcript. Neither policy construction, validation, hashing
nor E1 comparison proves operator approval. The issuer must authenticate the actual approved
selection for the scheme and asset using its genuine operator-owned configuration and
custody mechanism. There are no bundled production policies, approvals, roots or signer keys.

## Canonical frames and transcript encoding

Each policy uses canonical Norito with the existing wallet decode limits and a NEW 1024-byte
complete-frame cap. The NEW frames contain no u128 and require zero type padding, asserted
in the shipping Model without changing the preserved existing alignment contract. Decode
order is byte cap, canonical decode under payload-derived limits,
version 1, independently selected scheme, then structural rules. Pair and E1 binding checks
follow before verification or signing. The 1024 cap is an explicit conservative frame budget
for small bounded policy objects; it is not an existing published ABI size. The largest
Android enrollment-policy frame (eight Vendor roots with 16-byte adapter identifiers) must
fit it; the Model asserts that case. The proposed Native roundtrip/byte-mutation controls
must pass before adoption. Python consumes a private typed projection and computes
transcripts; it does not introduce another policy wire codec.

Hash identity uses the existing wallet `H` construction:
`SHA256("iroha:kagemusha:wallet:v1:" || ASCII(role) || 00 || LE64(body length) || body)`.
The two NEW role labels are `app-policy` and `enrollment-policy`. These identities choose
enrollment inputs; no recursive relation recomputes them. They introduce no Poseidon signing
domain, signature type, signer role or certificate chain. Existing signatures and the E1
challenge layout retain their current definitions. No signature over these unsigned policies
is invented.

Transcript integers are fixed-width little endian. Digests/pins are raw nonzero 32-byte
values. Text uses `LE32(UTF8 byte length) || exact UTF8 bytes`, preventing concatenation
ambiguity; no Unicode normalization, case folding, alternate terminator or padding is used.
Transcript tags below equal explicitly indexed Norito enum discriminants; Norito encodes
the tag as u32 and each hash transcript as u8. Zero and undefined enum tags are rejected.
A count is one byte in the transcript and the Norito sequence length in the frame.

The app-policy transcript starts `LE16(version) || scheme_id`, followed by exactly one arm:

- Android tag 1: text package name, `LE64(minimum_package_version)`, `u8 signer_count`, then
  `signer_count` app-signing certificate SHA256 digests. Package grammar is dotted ASCII
  identifiers: `[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)+`, at most 255 bytes. The
  minimum version is u64, including zero; an attested `versionCode` at or above it passes,
  so an app update needs no new policy. The signer set holds 1–4 nonzero digests in strictly
  ascending unsigned byte order, hence unique. It lists every signing certificate that a
  released distribution channel uses (for example an app store's signing key, the operator's
  direct-download release key and another store's signing key) and any signing-key rotation
  history; an install from a channel whose signer is absent cannot enroll. The set
  identifies certificates; real verified chains and app identities remain required. No client
  can select or override these fields.
- Apple tag 2: text App ID, nonempty and at most 255 UTF8 bytes, matching the current App
  Attest verifier. A new restrictive Team-ID grammar would be an additional protocol choice
  and is not assumed here. Production environment is mandatory and is not a selector.

The largest app transcript is 431 bytes (Android with a 255-byte package and four signers);
Apple is at most 294 bytes. App policy is historical enrollment identity. The credential
preserves its digest across renewals; an app update never requires re-enrollment. Raising the
minimum version or changing the signer set changes only the policy that new E1 challenges
name.

The enrollment-policy transcript begins
`LE16(version) || scheme_id || asset_digest || app_policy_digest`, followed by one arm:

- Android tag 1: `u8 root_count` (1–8), then `root_count` root entries in strictly ascending
  unsigned byte order of `spki_sha256`, then `LE32(patch_floor_yyyymm)`. A root entry is
  `u8 family || family fields || spki_sha256 || u8 revocation_source`:
  - family GoogleRsa 1, no fields: Google's RSA hardware attestation root key. Pinning the
    key covers every re-issued certificate of that key.
  - family GoogleCa1 2, no fields: Google's ECDSA P-384 "Key Attestation CA1" root.
  - family Vendor 3: text `adapter_id`, 1–16 bytes matching `[a-z][a-z0-9-]*` (for example
    `huawei`). It names an Android vendor whose own KeyMint attestation root a reviewed
    policy original admits.
  - `spki_sha256`: SHA256 of the root certificate's DER `SubjectPublicKeyInfo`; nonzero and
    unique across entries.
  - `revocation_source`: GoogleAttestationStatus 1 (Google's attestation status list) or
    OperatorDenylist 2 (the operator denylist only). GoogleRsa and GoogleCa1 require 1;
    Vendor requires 2. A vendor-published revocation service would be a new source tag with
    its own reviewed adapter. The operator denylist is checked for every entry, whatever its
    named source.
- Apple tag 2: root DER SHA256. Production App Attest plus a durable fresh assertion binding
  E1 and the separately selected payment key are always required.

Both arms end with the existing 20-byte inline regulatory-policy transcript:
`LE32(permitted_controls) || LE64(blacklist_max_age_ms) || LE64(time_anchor_max_response_ms)`,
then `LE64(challenge_lifetime_ms) || LE64(attestation_lease_lifetime_ms)`.
An Android transcript is 174 bytes with one Google root and at most 572 bytes with eight
Vendor roots; Apple is exactly 167. Order follows the E1 scope first, platform verification
inputs second, and credential regulator/lifetime rules last. There is no field order
sorting, optional arm, extension map or compatibility decoder.

Patch floor is a valid YYYYMM in years 1900–9999. Meeting it sets the PATCH_POLICY_MET fact;
an unmet floor leaves that fact unset and never refuses E1. Apple gets no Android, patch, boot
or local-compromise facts invented from App Attest.

Adding a root, for example a `Vendor("huawei")` entry once a genuine device chain proves that
root exists, is a reviewed policy-original change: data, not code. Removing a root or
changing the signer set likewise affects only new E1 challenges.

## Android admission

E5 Android evidence is the KeyMint attestation certificate chain and nothing else. The issuer
admits it only when every check below holds, in this order. The first failing check gives the
E1 refusal reason of design §2.4 shown in brackets. A failure to obtain a current revocation
source or the operator denylist is `Unavailable`, never a refusal or an admission.

1. Shape: 2–8 DER certificates, leaf first, each 1–16,384 bytes and at most 65,536 bytes in
   total [EvidenceInvalid].
2. Root and path: the last certificate is self-signed and its SPKI SHA256 equals one root
   entry. X.509 path validation from the leaf to that root succeeds, each certificate issued
   and signed by the next. Under GoogleRsa no certificate validity period is evaluated, per
   Google's guidance for factory-provisioned chains; revocation remains mandatory. Under
   GoogleCa1 and Vendor every certificate is valid at the issuer's trusted verification time
   [UntrustedAttestationRoot].
3. Revocation: no chain certificate is listed by the entry's revocation source, and none is
   listed by the operator denylist [AttestationRevoked]. The Google status list may be reused
   only within the freshness lifetime its HTTP `Cache-Control` states; otherwise it is fetched
   for the check. A missing, failed or malformed fetch is `Unavailable`.
4. Key description: the Android key description of the certificate nearest the root that
   carries one attests the leaf key, which is the E5 marker's payment key, and its challenge
   equals the E1 `challenge_digest` [EvidenceInvalid]. Attestation version 1 (Keymaster 2)
   has no app identity [AttestationWithoutAppIdentity]. Other attestation and KeyMint version
   pairs are those AOSP documents: (2, 3), (3, 4), (4, 41) and equal KeyMint versions 100 and
   above [EvidenceInvalid].
5. Secure hardware: the attestation and KeyMint security levels are equal and
   TrustedEnvironment (1) or StrongBox (2); attestation version 2 is TrustedEnvironment only
   [KeyNotInSecureHardware]. The attested level, never a policy selector, gives the evidence
   kind AndroidKeyMintTee or AndroidKeyMintStrongBox.
6. Key properties, hardware-enforced: purpose exactly SIGN, algorithm EC, key size 256, curve
   P-256, digest exactly SHA-256, origin GENERATED and a RootOfTrust. USAGE_COUNT_LIMIT (405)
   and ALL_APPLICATIONS (600) are absent from both authorization lists. Unknown
   context-specific tags are ignored; a duplicate or out-of-order tag rejects
   [EvidenceInvalid].
7. App identity: `attestationApplicationId` (709) appears exactly once, in either list. It
   names exactly one package, equal to the app-policy package, and lists at least one signer
   digest, every one in the signer set [AppIdentityMismatch]. The package version is at or
   above `minimum_package_version` [AppVersionBelowMinimum].
8. Boot, from the hardware-enforced RootOfTrust: `deviceLocked` is TRUE [BootloaderUnlocked];
   `verifiedBootState` is Verified and `verifiedBootKey` is nonzero, and for KeyMint
   (attestation version 100 and above) `verifiedBootHash` is 32 nonzero bytes
   [BootNotVerified]. Before KeyMint the hash is recorded when present and never required.
9. Patch: the hardware-enforced OS, vendor and boot patch levels are recorded; the floor sets
   only the PATCH_POLICY_MET fact.

An admitted Android evidence record carries facts HARDWARE_BACKED_KEY, BOOTLOADER_LOCKED,
VERIFIED_BOOT, APP_SIGNING_IDENTITY and REVOCATION_LIST_CLEAR, plus STRONGBOX for StrongBox and
PATCH_POLICY_MET when the floor is met. The issuer never sets fact bit 9
(PLAY_INTEGRITY_SIGNAL). That is issuer policy; record validation is unchanged here, and
removing the bit from the Model's masks is the separate validator-facing change that design
§9.1 schedules.

Nothing else is an Android admission input: no Play Integrity token or verdict, Google
account, Google Mobile Services, Huawei Mobile Services, app-store installation, exact
version, StrongBox-only selection, patch-level refusal, model or device allowlist, second
attested key or periodic online check. StrongBox, remote versus factory provisioning and
patch levels are recorded facts that never refuse a phone.

The operator denylist is one operator-owned original that the issuer selects and
authenticates like the policy originals but holds outside them, because it changes without
changing any policy digest. It is UTF-8 JSON with exactly the members `version` (the integer
1) and `entries`, an array of at most 65,536 unique objects. Each entry has exactly one
member. `serial` is a certificate serial number in lowercase hexadecimal without leading
zeros, the spelling of Google's status list; it matches any chain certificate except the
leaf, whose serial the phone chooses. `spki_sha256` is 64 lowercase hexadecimal digits of
SHA256 over a certificate's DER `SubjectPublicKeyInfo`; it matches any chain certificate,
including the leaf. Duplicate members, unknown members and malformed values reject the whole
original. The issuer applies its current denylist to every verification; a missing,
unreadable or malformed denylist is `Unavailable`, never an empty list. An empty `entries`
array is a deliberate operator statement.

## Apple admission

E5 Apple evidence is the App Attest key identifier, the original attestation object and the
fresh first key-binding assertion. The issuer admits it only when the attestation chains to the
pinned Apple App Attestation root DER [UntrustedAttestationRoot]; the App ID equals the app
policy's and the environment is production [AppIdentityMismatch]; and the nonce, key
identifier, counter and key-binding assertion over
`H("enrollment-key-binding", challenge_digest || payment_key)` all match [EvidenceInvalid].
An admitted record carries APP_ATTEST_GENUINE_DEVICE, APP_ATTEST_KEY_BINDING and
APP_ATTEST_PRODUCTION.

## Lifetimes, custody and producer ownership

Challenge lifetime is a positive u64 with no default. The NEW accepted interval is
`created_at_ms <= trusted_now_ms < created_at_ms + lifetime`, with positive genuine server
creation time and checked u64 addition. Duration has no newly invented wall-clock ceiling.
The issuer must store creation time with the exact challenge, authenticated subject and
selected originals; request timestamps are never substitutes. A liveness check does not
consume a challenge. The durable owner must enforce single use, exact-result retries and
permanent account/wallet activation claims before any HTTP result becomes visible. A refusal
is an exact result: it consumes the challenge, and a retry returns the same refusal reason.

Existing regulatory bits and consistency rules are preserved. Zero permitted controls does
not disable Kagemusha; it permits no extra blacklist/quota/lease restrictions. Lease lifetime
is zero iff the regulator does not permit attestation lease and otherwise positive. The NEW
initial producer computes expiry as checked `issued_at_ms + lease_lifetime` with positive
trusted issuer time. This is an attestation lease. Renewals keep
the original credential app identity/regulatory policy and require their separate current
owner contract; this E1 proposal grants no renewal signer or mutable historical identity.

Android roots are pinned by SPKI and the chain carries its root certificate, so no Android
root DER original is configured. The Apple root DER is a genuine external original selected
by the approved issuer owner, outside the public policy frame, nonempty and at most the
existing platform `MAX_CERT` bound of 16,384 bytes. Python compares SHA256 of those exact
bytes to the policy pin, then the real chain verifier runs. A self-consistent root/pin pair
is not approval.

Python's typed enrollment-policy projection digest is the selected typed enrollment-policy
digest, never a separately caller-supplied opaque pin. The KeyMint leaf-first DER originals,
or Apple attestation and assertion originals, must be retained privately before credential
signing. Native rederives the evidence digest from exact originals. Public projections or
success flags are not evidence originals or signer capabilities.

The coordinated Native issuer still owns authenticated workload purpose/method/path/body,
access subject and exact canonical account/asset scope. Every universal-dataspace token may
use offline enrollment without Parliament approval or a named-token allowlist. Under a
Bank policy, current KYC approval and freeze status come from the user's bank middleware
through the [challenge-bound eligibility SDK contract](kagemusha_enrollment_eligibility_v1.md).
Other schemes select their authorized operator and may explicitly admit public enrollment.
Parliament is an optional provider for assets it governs; it has no required protocol role.
A missing or failed bank observation never selects an operator policy implicitly. The issuer independently authenticates
current authority/routing and retains each fresh observation at its durable boundary. It owns
approved original selection, CSPRNG nonce, challenge CAS/use, durable Apple counters, genuine
time, original evidence/result retention and one genuine scheme-rooted Enrollment-role P256
signer. FI access/DPoP/workload keys are separate transport signers. It must build the current
credential with typed app digest, exact regulator/lease result and verified evidence, freeze
the actual signer output and call `verify_enrollment` against the retained exact E1/payment
key before delivery. Reusing deleted RetailEnrollment/Ed25519 authority is not an option.

Model, Python adapter, specification, digest-role inventory and shared DATA vectors form one
proposal. Swift/Kotlin/JS/C# consumers and the current issuer/Native bridge must migrate their
actual policy ownership and use these new preimages together. The shared vectors are
unadmitted public DATA with repeated placeholder pins, without root originals, approvals,
signers, device evidence or production deployment qualification. Passing them proves byte
agreement only. Canonical Native frame qualification remains a required independent check.
