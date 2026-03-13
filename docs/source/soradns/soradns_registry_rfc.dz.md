---
lang: dz
direction: ltr
source: docs/source/soradns/soradns_registry_rfc.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3af7d17805ec7f0d8b5fbc6a4798b10832157f0ff5ca6b91c2e0cca49544a9bb
source_last_modified: "2026-01-09T07:52:09.876207+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraDNS Registry & Proof Bundles (DG-1)

Status: Drafted 2026-02-20  
Owners: Networking TL, Governance Council  
Related roadmap item: DG-1 - Draft SoraDNS RFC

## 1. Motivation

The Sora DNS (SoraDNS) program provides a deterministic naming layer for Nexus
and Iroha deployments. Contracts record authoritative name metadata while
gateways and resolvers distribute signed zone material pinned in SoraFS. This
document specifies the registry contract surface, content hashes, proof bundle
format, resolver discovery flow, and continuity fallbacks required to unblock
implementation work (DG-2 and DG-3).

## 2. Goals

- Define an on-chain registry that maps canonical names to signed zone bundles,
  supports deterministic replay, and exposes governance controls for suffix
  stewardship.
- Standardise the namehash function, selector layout, and storage keys so every
  client resolves the same canonical ID.
- Describe how zone material is pinned in SoraFS (CAR + Norito manifests) and
  how registrars publish signed hashes for integrity.
- Specify the proof bundle consumed by resolvers, including DNSSEC-style key
  hierarchies (KSK/ZSK), delegation attestations, and freshness metadata.
- Outline resolver discovery: Resolver Advertisement Documents (RADs), Torii
  registry feeds, and conventional bootstrap (DoH/DoT/DNS).
- Capture operational fallbacks, including safe failure modes when SoraFS or
  Torii are degraded.

Non-goals: implement resolver code (DG-2), define auction/economic policy for
names (tracked under SNS roadmap), or cover browser gateway integration (DG-3).

## 3. Terminology

- **Label** - UTF-8 label within a zone (`app`, `dev`, `xn--unicode`).
- **FQDN** - fully qualified domain name, lowercased and NFC normalised
  (`app.dao.sora`).
- **Registry contract** - Iroha smart contract storing `NameRecord` entries.
- **Zone Signing Key (ZSK)** - key that signs RRsets within a zone.
- **Key Signing Key (KSK)** - key that signs the ZSK set (DNSSEC analogue).
- **ZSK bundle CID** - SoraFS CID pointing to the CAR file that contains signed
  RRsets and metadata for a specific zone version.

## 4. Registry Contract

All surfaces use Norito encoding. The contract lives under
`soradns::registry`. Primary structures:

```text
Enum ZoneStatus {
    Active,
    GracePeriod { expires_at: Timestamp },
    Tombstoned { reason: String, tombstoned_at: Timestamp },
}

Struct NameRecordV1 {
    fqdn: AsciiString,              // canonical lower-case FQDN
    namehash: [u8; 32],             // blake3 hash of selector (see Section 5)
    parent: Option<[u8; 32]>,       // namehash of parent zone, None for root
    zone_version: u64,              // monotonic counter incremented on publish
    zsk_bundle: SorafsCid,          // CID of the CAR zone bundle
    manifest_cid: SorafsCid,        // CID of the manifest describing files
    proof_bundle: SorafsCid,        // CID of proof bundle (see Section 6)
    ttl_seconds: u32,               // resolver cache TTL for RAD entries
    owner: AccountId,               // registrar/owner account
    status: ZoneStatus,
    max_ttl_seconds: u32,           // enforced upper bound for RR TTLs
    created_at: Timestamp,
    updated_at: Timestamp,
}

Struct DelegationV1 {
    child_namehash: [u8; 32],
    parent_namehash: [u8; 32],
    ds_record: DelegationSignatureV1,
    valid_from: Timestamp,
    valid_until: Timestamp,
}

Struct ZoneAuditLogEntryV1 {
    namehash: [u8; 32],
    zone_version: u64,
    manifest_cid: SorafsCid,
    proof_bundle: SorafsCid,
    change_reason: String,
    author: AccountId,
    timestamp: Timestamp,
}
```

Contract invariants:

1. `zone_version` strictly increases on every publish; old versions remain
   addressable through `ZoneAuditLog`.
2. `manifest_cid`, `zsk_bundle`, and `proof_bundle` must reference existing
   SoraFS pins signed by the registrar and the governance steward.
3. `ttl_seconds <= max_ttl_seconds` and the governance steward may lower
   `max_ttl_seconds` on suffix breaches.
4. `ZoneStatus::GracePeriod` automatically transitions to `Tombstoned` after
   `expires_at` unless renewed.

Registrar operations:

- `PublishZone(NameRecordV1, DelegationV1[])`
- `UpdateZone(StatusUpdate)`
- `RotateZsk(new_bundle, proof_bundle, zone_version++)`
- `TransferOwnership(new_owner)` - requires suffix steward approval.

Governance operations:

- `SetSuffixPolicy { suffix, steward, price_class, guardrails }`
- `FreezeZone { namehash, reason }` - enters GracePeriod with mandatory proof.
- `RecordAudit(ZoneAuditLogEntryV1)` - immutably logs policy checks.

## 5. Namehash & Selector

### 5.1 Normalisation

1. Lowercase ASCII portion; apply Unicode NFKC + casefold for non-ASCII.
2. Reject labels exceeding 63 bytes post normalisation or containing control
   characters.
3. Ensure the full FQDN is <= 255 bytes.

### 5.2 Selector Layout

```
struct SelectorV1 {
    u8 version = 1;
    u8 depth;                  // number of labels
    [LabelRef; depth];         // offsets into `labels`
    bytes labels;              // null-terminated UTF-8 labels
    u16 suffix_id;             // registry ID for TLD/suffix (per SNS plan)
}
```

`LabelRef` stores `{ start: u16, len: u16 }`. The selector serialises using
Norito's byte string mode (`bytes`) to guarantee deterministic encoding.

### 5.3 Hash Derivation

```
fn namehash(fqdn: &str) -> [u8; 32] {
    let selector = build_selector_v1(fqdn);
    blake3::hash(selector_bytes).into()
}
```

Example (`app.dao.sora`):

```
selector = {
  version: 1,
  depth: 3,
  labels: ["app", "dao", "sora"],
  suffix_id: 0x0010 (".sora")
}
namehash = 0x4a0d3a7bcefb6cc4439af5efec3f61e141c4bf7f45ed05edbf6a6844d8b89990
```

Resolvers must verify the selector version and suffix ID, rejecting unknown
values. Future selector versions MUST change the `version` byte and retain v1

## 6. Zone Bundles & Proofs

Zone material is packaged as a CARv2 archive containing:

| Path | Content |
|------|---------|
| `/manifest.norito` | `ZoneBundleManifestV1` (Norito) |
| `/rrsets/*.norito` | RRset payloads signed by the ZSK |
| `/delegations/*.norito` | `DelegationSignatureV1` entries |
| `/metadata.json` | Optional human-readable summary |

`ZoneBundleManifestV1`:

```text
Struct ZoneBundleManifestV1 {
    namehash: [u8; 32],
    zone_version: u64,
    rrset_cids: Vec<SorafsCid>,
    delegation_cids: Vec<SorafsCid>,
    signed_by: PublicKey,          // ZSK identifier
    signed_at: Timestamp,
    signature: Signature,          // Norito-coded zsk::Signature
    max_rr_ttl_seconds: u32,
}
```

### 6.1 Proof Bundle

Proof bundles link the on-chain record to the Zone bundle:

```text
Struct ProofBundleV1 {
    namehash: [u8; 32],
    zone_version: u64,
    manifest_hash: [u8; 32],       // BLAKE3 digest of manifest bytes
    car_root_cid: SorafsCid,
    ksk_set: Vec<KskEntryV1>,
    zsk_signatures: Vec<ZskSignatureV1>,
    delegation_chain: Vec<DelegationProofV1>,
    freshness: FreshnessProofV1,
    policy_hash: [u8; 32],         // hash of suffix policy snapshot
}

Struct KskEntryV1 {
    public_key: PublicKey,
    valid_from: Timestamp,
    valid_until: Timestamp,
    signature: GovernanceSignature,
}

Struct FreshnessProofV1 {
    issued_at: Timestamp,
    expires_at: Timestamp,
    signer: AccountId,             // steward account
    signature: GovernanceSignature,
}
```

- `manifest_hash` is compared against the retrieved `/manifest.norito`.
- `policy_hash` references governance policy (price class, TTL cap). Resolvers
  may fetch the policy from Torii to display warnings.
- `delegation_chain` provides parent -> child DS-style attestations, rooted in
  the suffix steward.

Proof bundles are pinned separately in SoraFS and referenced from the registry
record. Clients fetch them over Torii REST (`/v2/soradns/proof/{namehash}`) or
via SoraFS gateway.

## 7. Resolver Discovery

### 7.1 Resolver Advertisement Documents (RAD)

Operators publish RAD entries under the registry contract:

```text
Struct ResolverAdvertV1 {
    resolver_id: String,           // e.g. `resolver.sora.net`
    region: String,                // ISO 3166-1 alpha-2 or `global`
    doh_endpoint: Url,
    dot_endpoint: Option<Url>,
    soranet_endpoint: Option<Multiaddr>,
    supported_suffixes: Vec<u16>,  // suffix IDs
    proof_bundle: SorafsCid,       // attestation for resolver config
    telemetry: SorafsCid,          // optional metrics spec
    updated_at: Timestamp,
    signature: Signature,          // operator account signs advert
}
```

Clients obtain RAD data via:

1. Torii REST: `/v2/soradns/resolvers` returns paginated adverts.
2. SoraFS pinned snapshot: `soradns/resolvers/latest.car`.
3. Gossip feed (RAD topic) using SoraNet circuits for low-latency updates.

### 7.2 Bootstrap Strategy

1. Ship resolvers list with the client bundle (signed manifest).
2. Attempt Torii fetch via the public gateway (`https://torii.sora.net`).
3. If Torii unavailable, fall back to DoH bootstrap against a baked resolver
   set; responses MUST include proof bundles.
4. Persist validated RAD entries in a local cache with expiry = `ttl_seconds`.

Clients must validate RAD signatures against operator key material in the
registry contract. Unknown suffix IDs trigger a soft failure (resolver entry is
ignored but cached separately for diagnostics).

### 7.3 Public DNS delegation (regular internet)

SoraDNS host derivation does not replace public DNS delegation. The regular
internet finds authoritative name servers through the standard DNS hierarchy:

1. Recursive resolvers ask the root servers.
2. Root servers refer the query to the TLD servers.
3. The parent/TLD zone returns NS records (and DS when DNSSEC is enabled).
4. The authoritative name servers answer with A/AAAA/CNAME/TXT records.

That parent-zone NS/DS delegation is set at your registrar or DNS provider and
is not managed by this repository.

For public DNS records that point at SoraDNS gateways:

- For subdomains, publish a CNAME from your public name to the derived pretty
  host (`<fqdn>.gw.sora.name`).
- For an apex or TLD, use your DNS provider's ALIAS/ANAME feature or publish
  A/AAAA records that point at the gateway anycast IPs. CNAME is not valid at
  the apex.
- The canonical host (`<hash>.gw.sora.id`) lives under the SoraDNS gateway
  domain and is not published in your public zone; clients use it for gateway
  verification and GAR policy checks.

See `docs/source/soradns/deterministic_hosts.md` for the deterministic host
derivation and GAR requirements.

## 8. Fallback Paths

- **SoraFS Degradation:** If SoraFS gateways are unreachable, clients may
  request bundles directly from resolvers over SoraNet with `car-proxy` mode.
  Resolvers serve CAR archives signed with the same `ProofBundleV1` manifest
  hash; clients verify equality before accepting.
- **Torii Outage:** Use the baked resolver list or cached RAD data. Registrars
  must still publish updates on-chain; clients reconcile once Torii recovers.
- **Governance Emergency:** Stewards can issue `EmergencyOverrideV1` records
  that swap the resolver set or pin alternate bundles. Overrides are recorded in
  the audit log and expire automatically after 72 hours unless ratified.
- **Clock Skew:** Freshness proofs carry +/-300 s tolerance. Clients with larger
  skew should warn but may continue using cached records until `expires_at`.

## 9. Security & Compliance Considerations

- Keys use `iroha_crypto` (Ed25519 today; future ML-DSA IDs reserved).
- Bundles and proofs must be reproducible; registrars publish signed manifests
  with build metadata (tool versions, script hashes).
- Resolver telemetry must respect privacy policies; metrics exports are optional
  and carry their own Norito schema referenced in `ResolverAdvertV1`.
- Governance maintains an allowlist of suffix stewards; transfers require
  multi-signature approval recorded in the audit log.

## 10. Next Steps

- DG-2: Implement `soradns-resolver` prototype that consumes `ProofBundleV1`
  and RAD feeds, falling back to embedded bootstrap data.
- DG-3: Build gateway automation that maps canonical hosts to resolver outputs,
  manages ACME issuance, and surfaces resolver health telemetry.
- SNS roadmap items leverage this contract for registrar workflows and pricing.

Appendix with selector pseudocode and full Norito schema definitions will be
added once the generator scaffolding (DG-2) is ready.
