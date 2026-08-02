<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Explicit domain normalization and universal accounts

Iroha 3 uses universal, domainless account identities. An `AccountId` and its
`AccountAddress` encode only the authorization controller. They do not carry a
default-domain marker, a truncated domain digest, or a registry selector.
Changing client display preferences therefore cannot change address bytes,
transaction validation, events, World state, or replay.

Domain context is represented explicitly:

- registered domains are keyed by the complete canonical `DomainId`;
- account aliases retain their complete domain/dataspace context;
- SNS leases use their explicit `NameSelectorV1` namespace and label; and
- ownership indexes map owners directly to complete `DomainId` values.

There is no persistent account-address-to-domain selector index and no node
configuration for a default account domain. A wallet may offer a local input
shortcut, but it must resolve that input to a complete `DomainId` or alias
before constructing a transaction.

## Domain-label normalization

Explicit domain and alias constructors apply the repository's canonical name
rules. `name::canonicalize_domain_label` performs NFC normalization followed by
strict UTS-46 ASCII conversion, lowercasing, DNS label-length validation, and
the reserved-character policy. The result is deterministic across peers.

This normalization is for explicit domain-bearing records only. It is never an
input to `AccountAddress` encoding.

## Canonical account-address payload

The first-release account-address payload consists of the version/class header
and controller payload. Canonical decoding rejects trailing bytes, so legacy
payloads containing selector tags or digests are not accepted. I105 formatting
wraps the same domainless canonical payload.

## Security invariants

Implementations and tests must preserve these properties:

1. the same controller always produces the same canonical account payload;
2. registering and deleting a domain uses the exact `DomainId` supplied by the
   instruction;
3. alias setup and SNS repair never derive state keys from process-local
   defaults; and
4. snapshot decoding rebuilds only indexes derived from canonical World data.

The shared address vectors under `fixtures/account/address_vectors.json`
exercise the selector-free account payload and strict rejection of
noncanonical trailing material.
