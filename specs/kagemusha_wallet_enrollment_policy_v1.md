# NEW first-release E1 app and enrollment policy contract

This proposal fills the two current E1 policy preimage gaps. Its types, tags, domains,
serialization choices and lifetime rules are NEW first-release protocol decisions. They are
not a recovered historical grammar, external approval, or a mapping of any existing opaque
policy digest. Kagemusha has no enable switch or disabled policy variant.

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
for small bounded policy objects; it is not an existing published ABI size. The proposed
Native roundtrip/byte-mutation controls must pass before adoption. Python consumes a private
typed projection and computes transcripts; it does not introduce another policy wire codec.

Hash identity uses the existing wallet `H` construction:
`SHA256("iroha:kagemusha:wallet:v1:" || ASCII(role) || 00 || LE64(body length) || body)`.
The two NEW role labels are `app-policy` and `enrollment-policy`. These identities choose
enrollment inputs; no recursive relation recomputes them. They introduce no Poseidon signing
domain, signature type, signer role or certificate chain. Existing signatures and E1 bytes
retain their current definitions. No signature over these unsigned policies is invented.

Transcript integers are fixed-width little endian. Digests/pins are raw nonzero 32-byte
values. Text uses `LE32(UTF8 byte length) || exact UTF8 bytes`, preventing concatenation
ambiguity; no Unicode normalization, case folding, alternate terminator or padding is used.
Transcript tags below equal explicitly indexed Norito enum discriminants; Norito encodes
the tag as u32 and each hash transcript as u8. Zero and undefined enum tags are rejected.

The app-policy transcript starts `LE16(version) || scheme_id`, followed by exactly one arm:

- Android tag 1: text package name, `LE64(package_version)`, app-signing certificate SHA256.
  Package grammar is the actual current Google verifier's dotted ASCII identifiers:
  `[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)+`, at most 255 bytes. Version is u64,
  including zero. The pin identifies the certificate; real verified chains/app identities
  remain required. No client can select or override these fields.
- Apple tag 2: text App ID, nonempty and at most 255 UTF8 bytes, matching the current App
  Attest verifier. A new restrictive Team-ID grammar would be an additional protocol choice
  and is not assumed here. Production environment is mandatory and is not a selector.

The largest app transcript is 334 bytes (Android with a 255-byte package); Apple is at most
294 bytes. App policy is historical enrollment identity. The credential preserves its digest
across renewals; this contract does not require re-enrollment after a software update.

The enrollment-policy transcript begins
`LE16(version) || scheme_id || asset_digest || app_policy_digest`, followed by one arm:

- Android tag 1: root DER SHA256; one-byte hardware selector (1 TEE, 2 StrongBox, 3 either);
  `LE32(patch_floor_yyyymm)`; `LE64(play_integrity_maximum_age_ms)`; one-byte recognized-app
  requirement; one-byte licensing requirement; one-byte minimum Google device verdict
  (1 `MEETS_DEVICE_INTEGRITY`, 2 `MEETS_STRONG_INTEGRITY`). Booleans are exactly 0/1.
- Apple tag 2: root DER SHA256. Production App Attest plus a durable fresh assertion binding
  E1 and the separately selected payment key are always required.

Both arms end with the existing 20-byte inline regulatory-policy transcript:
`LE32(permitted_controls) || LE64(blacklist_max_age_ms) || LE64(time_anchor_max_response_ms)`,
then `LE64(challenge_lifetime_ms) || LE64(attestation_lease_lifetime_ms)`.
Android is exactly 183 bytes; Apple is exactly 167. Order follows the E1 scope first, platform
verification inputs second, and credential regulator/lifetime rules last. There is no field
order sorting, optional arm, extension map or compatibility decoder.

The Android selectors preserve the actual current verifier. They admit only KeyMint hardware
levels 1/2, never software. Google revocation and fixed-endpoint authenticated server decode
run independently on every Android enrollment, even when a verdict-requirement boolean is
false. Those booleans change accepted Google labels; they do not disable Kagemusha, hardware,
TLS decoding, app/package/version/signing-pin equality, requestHash, freshness or revocation.
Google maximum age is positive u64 and, by this NEW contract, cannot exceed E1's challenge
lifetime. Patch floor is a valid YYYYMM in years 1900–9999. An unmet floor records an unset
patch-policy fact; current E1 does not reject solely for that fact. Apple gets no Android,
patch, boot or local-compromise facts invented from App Attest.

## Lifetimes, custody and producer ownership

Challenge lifetime is a positive u64 with no default. The NEW accepted interval is
`created_at_ms <= trusted_now_ms < created_at_ms + lifetime`, with positive genuine server
creation time and checked u64 addition. Duration has no newly invented wall-clock ceiling.
The issuer must store creation time with the exact challenge, authenticated subject and
selected originals; request timestamps are never substitutes. A liveness check does not
consume a challenge. The durable owner must enforce single use, exact-result retries and
permanent account/wallet activation claims before any HTTP result becomes visible.

Existing regulatory bits and consistency rules are preserved. Zero permitted controls does
not disable Kagemusha; it permits no extra blacklist/quota/lease restrictions. Lease lifetime
is zero iff the regulator does not permit attestation lease and otherwise positive. The NEW
initial producer computes expiry as checked `issued_at_ms + lease_lifetime` with positive
trusted issuer time. This is an attestation lease, never a Play Integrity lease. Renewals keep
the original credential app identity/regulatory policy and require their separate current
owner contract; this E1 proposal grants no renewal signer or mutable historical identity.

Root DER is a genuine external original selected by the approved issuer owner, outside the
public policy frame, nonempty and at most the existing platform `MAX_CERT` bound of 16,384
bytes. Python compares SHA256 of those exact bytes to the policy pin, then the
real chain verifier runs. A self-consistent root/pin pair is not approval. The first Android
producer remains Google-root-only with its actual Google revocation contract. Other vendors
need their genuine adapter-specific revocation semantics before installation.

Python's `PlayIntegrityEnrollmentPolicy.policy_digest` projection is explicitly NEW: it is
the selected typed enrollment-policy digest, never a separately caller-supplied opaque pin.
The actual Google response plus KeyMint leaf-first DER originals, or Apple attestation and
assertion originals, must be retained privately before credential signing. Native rederives
the current evidence digest from exact originals. Public projections or success flags are
not evidence originals or signer capabilities.

The coordinated Native issuer still owns authenticated workload purpose/method/path/body,
access subject and exact canonical account/asset scope. Current enrollment eligibility and
freeze status come from the user's bank middleware through the
[challenge-bound eligibility SDK contract](kagemusha_enrollment_eligibility_v1.md). An explicitly
enacted Parliament mandate may cover a governed non-regulated token; it cannot replace bank
eligibility for a regulated asset or claim bank KYC. The issuer independently authenticates
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
