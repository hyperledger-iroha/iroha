---
lang: ru
direction: ltr
source: docs/source/soradns/deterministic_hosts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b8cf7d26ce7788692fb1716314c4b2c92577c61a318a057b9f12db9c3e1d7fdc
source_last_modified: "2026-01-09T06:56:21.714117+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraDNS Deterministic Hosts (DG-3a)

Status: Published 2026-03-17 (revised 2026-04-05)  
Owners: Networking TL, Docs DevRel  
Related roadmap item: DG-3a — Deterministic host mapping & resolver SDK

## 1. Purpose

SoraDNS names must resolve to HTTPS gateways without trusting intermediate
discoveries. This note specifies the deterministic mapping between a registered
FQDN and its gateway hosts, documents the constraints applied to “pretty”
aliases, and captures the SDK interfaces now available for resolvers and
clients to enforce Gateway Authorisation Record (GAR) policy before dialing.

## 2. FQDN Normalisation Rules

The helper expects Punycode/ASCII domain names and applies the same guardrails
as `soradns_registry_rfc.md`:

1. Trim surrounding whitespace; reject empty inputs.
2. Reject a leading or trailing dot, as well as consecutive dots.
3. Allow only `a–z`, `0–9`, and `-` inside labels (case-insensitive);
   uppercase letters are folded to lowercase.
4. Enforce the DNS limits: each label ≤63 characters and the full FQDN ≤253.

Inputs breaching these rules raise a validation error so clients never derive a
gateway host for an ill-formed name.

## 3. Canonical Host Derivation

Given a normalised FQDN `name`, the canonical gateway host is constructed as:

1. Compute `blake3(name.as_bytes())`.
2. Encode the 32-byte digest with lowercase base32 (no padding).
3. Append the canonical suffix (`gw.sora.id`).

```
canonical_label = base32(blake3(name))
canonical_host  = `${canonical_label}.gw.sora.id`
```

This host is eligible for a wildcard certificate. It is the primary address
returned by resolvers when HTTPS connections are established without relying on
the “pretty” alias.

## 4. Pretty Host Policy

Resolvers also publish a human-readable alias so that browsers and tooling can
present names without hashes:

```
pretty_host = `${name}.gw.sora.name`
```

The `.gw.sora.name` zone is issued via on-demand ACME certificates. Because it
inherits the user-controlled labels, the normalisation rules above are enforced
verbatim to keep certificates reproducible and ensure GAR policy remains
deterministic.

### Public DNS delegation (regular internet)

SoraDNS host derivation does not replace public DNS delegation. The regular
internet finds your nameserver through standard DNS hierarchy:

1. A recursive resolver (e.g. 1.1.1.1, 8.8.8.8) asks the root servers.
2. The root refers the resolver to the TLD servers (your parent zone).
3. The parent zone returns NS records (and DS for DNSSEC) that point to your
   authoritative nameservers.
4. Your authoritative nameservers answer with A/AAAA/CNAME/TXT records for the
   domain.

That parent-zone NS/DS delegation is set at your registrar or DNS provider and
is not managed by this repository.

For public DNS records that point at the SoraDNS gateway:

- For subdomains, publish a CNAME from your public name to the derived pretty
  host (for example, `app.sora.example` -> `<fqdn>.gw.sora.name`).
- For an apex/TLD (for example, `example` or `example.tld`), use your DNS
  provider's ALIAS/ANAME feature or publish A/AAAA records to the gateway
  anycast IPs. CNAME is not valid at the apex.
- The canonical host (`<hash>.gw.sora.id`) lives under the SoraDNS gateway
  domain and is not published inside your public zone; clients use it for
  gateway verification and GAR policy checks.

The `tools/soradns-resolver` binary is a recursive resolver prototype that
serves configured static zones for testing. It does not register NS/DS
delegations or act as a public authoritative DNS service.

## 5. GAR Requirements

Gateway Authorisation Records must authorise the following host patterns for
each registered name:

1. The canonical host (`<hash>.gw.sora.id`).
2. The canonical wildcard (`*.gw.sora.id`).
3. The pretty host (`<fqdn>.gw.sora.name`).

Clients should reject GAR payloads that omit any of these entries. The Rust
helper returns the trio in the expected order so verifiers can compare directly
against `GatewayAuthorizationRecord::host_patterns()` without bespoke logic.

## 6. SDK Interfaces

Two SDK surfaces now expose deterministic host derivation:

- **Rust** (`crates/iroha_primitives/src/soradns/hosts.rs:1`):
  `derive_gateway_hosts(fqdn)` returns a `GatewayHostBindings` struct with the
  canonical label, canonical/pretty hosts, wildcard pattern, and host-pattern
  helper methods. The type is re-exported from the resolver crate so tooling
  can depend on it without pulling in additional modules.
- **TypeScript / JavaScript** (`javascript/iroha_js/src/soradns.js:1`,
  re-exported as `deriveSoradnsGatewayHosts`): the helper invokes the native
  binding for accurate Blake3 hashing, returning an immutable object that mirrors
  the Rust struct. `hostPatternsCoverDerivedHosts(patterns, derived)` validates
  GAR entries and ships with full TypeScript definitions
  (`SoradnsGatewayHosts`, `hostPatternsCoverDerivedHosts` overloads) so browser
  and Node callers get static checking for every field.

Both bindings enforce the normalisation rules above and surface descriptive
errors when validation fails.

## 7. Validation Checklist

- Unit tests cover happy-path derivation, invalid character rejection, label
  length enforcement, and pattern matching in both languages.
- GAR validation helpers confirm that payloads include all three required host
  entries.
- JS bindings require the compiled `iroha_js_host` module; CLI tooling should
  continue to run `npm run build:native` during release builds so deterministic
  hashing is available.

## 8. SDK Quickstart

### 8.1 Rust example

The snippet below demonstrates how resolver tooling can derive canonical hosts,
persist the GAR host-pattern list, and compare it against responses served by a
gateway without issuing a network request first.

```rust
use iroha_primitives::soradns::{derive_gateway_hosts, GatewayHostError};

fn main() -> Result<(), GatewayHostError> {
    let bindings = derive_gateway_hosts("docs.sora")?;
    println!("Normalized name: {}", bindings.normalized_name());
    println!("Canonical host : {}", bindings.canonical_host());
    println!("Pretty host    : {}", bindings.pretty_host());
    println!("GAR patterns   :");
    for pattern in bindings.host_patterns() {
        println!("  - {pattern}");
    }
    Ok(())
}
```

### 8.2 TypeScript / JavaScript example

A runnable recipe (`javascript/iroha_js/recipes/soradns.mjs`) is included in
the SDK to mirror the Rust helper. It accepts a SoraDNS FQDN, prints the
derived hosts, and optionally validates a comma-separated list of GAR host
patterns:

```bash
npm install
npm run build:native
node ./recipes/soradns.mjs docs.sora \
  --gar-patterns app.dao.sora.gw.sora.name,*.gw.sora.id
```

Internally the script uses:

```ts
import {
  deriveSoradnsGatewayHosts,
  hostPatternsCoverDerivedHosts,
} from "@iroha/iroha-js";

const derived = deriveSoradnsGatewayHosts("docs.sora");
console.log(derived.canonicalHost);

const patterns = [
  derived.canonicalHost,
  derived.canonicalWildcard,
  derived.prettyHost,
];
console.log(hostPatternsCoverDerivedHosts(patterns, derived)); // => true
```

SDK consumers can import the same helpers in browser contexts (after bundling)
thanks to the TypeScript definitions included in `index.d.ts`.

## 9. GAR validation flow

Resolvers must reject gateway responses unless the advertised GAR host patterns
authorise all three derived hosts. A typical validation loop is:

1. Derive bindings locally via the Rust or TypeScript helper.
2. Load the GAR payload from the registry or resolver response and extract its
   `host_patterns` array.
3. Call `GatewayHostBindings::host_patterns()` or
   `hostPatternsCoverDerivedHosts()` and abort if the boolean outcome is false.
4. Only after step 3 succeeds should the client attempt HTTPS connections
   against the canonical or pretty hosts.

The `soradns.mjs` recipe performs the same checks against comma-separated input,
making it a convenient diagnostic tool during GAR rollouts.

## 10. CLI verification helper

`cargo xtask soradns-hosts` now accepts `--verify-host-patterns <path>` so
operators can compare GAR payloads against the derived host set before
publishing. The file must be JSON using either of the following shapes:

```json
[
  {
    "name": "docs.sora",
    "host_patterns": [
      "b6ukbnpp2tthm3e7fmaxqll2aq.gw.sora.id",
      "*.gw.sora.id",
      "docs.sora.gw.sora.name"
    ]
  }
]
```

or a shorthand map:

```json
{
  "docs.sora": [
    "b6ukbnpp2tthm3e7fmaxqll2aq.gw.sora.id",
    "*.gw.sora.id",
    "docs.sora.gw.sora.name"
  ]
}
```

The command fails when a record omits any of the three required host patterns or
when the file contains entries for names that were not requested, keeping GAR
evidence deterministic:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --verify-host-patterns artifacts/soradns_gar/docs.sora.json
```

## 11. GAR template helper

`cargo xtask soradns-gar-template` bootstraps a Gateway Authorization Record
payload so DG-3 change tickets can include deterministic host patterns and
header templates without manual copy/paste. The helper emits canonical hosts,
the wildcard pattern, default CSP/HSTS/Permissions-Policy templates, and empty
policy scaffolding that authors can extend before signing:

```bash
cargo xtask soradns-gar-template \
  --name docs.sora \
  --manifest artifacts/sorafs/portal.manifest.json \
  --manifest-digest ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789 \
  --telemetry-label dg-3 \
  --valid-from 1735771200 \
  --json-out artifacts/soradns_gar/docs.sora.template.json
```

Use `--csp-template`, `--permissions-template`, or `--hsts-template` to override
the default header strings, `--valid-until` to bound the validity window, and
supply additional `--telemetry-label` flags as needed. Passing `--json-out -`
writes the payload to stdout so operators can inspect it inline or feed it into
signing tooling.
