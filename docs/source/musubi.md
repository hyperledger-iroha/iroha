---
title: Musubi Kotodama Package Manager
---

# Musubi Kotodama Package Manager

Musubi is the package manager for Kotodama source packages. It gives
developers a Cargo-like workflow for sharing composable Kotodama programs while
keeping package identity tied to Sora/Iroha namespaces instead of a global
first-come name table.

## Package Names

Canonical package ids use:

```text
namespace/package
```

Exact release references use:

```text
namespace/package@version
```

There is no leading `@` before a namespace. The `@` separator is reserved only
for the version suffix.

The `namespace` segment intentionally matches the suffix used by Kotodama dapp
contract aliases:

- `universal/math` links to contract aliases such as `router::universal`
- `dex.universal/swap-core` links to aliases such as `router::dex.universal`

The namespace has either `<dataspace>` or `<domain>.<dataspace>` form. Musubi
validates dapp links by checking that every linked contract alias uses the same
namespace suffix as the package.

## Musubi.toml

A package manifest starts with a package table:

```toml
[package]
namespace = "dex.universal"
name = "swap-core"
version = "0.1.0"

[dependencies.math]
package = "std.universal/math"
version = "1.0.0"

[exports]
functions = ["quote"]

[dapp]
namespace = "dex.universal"
contracts = ["router::dex.universal"]
```

Dependencies are exact release references in this first slice. The lockfile
stores canonical package ids only; global short names are not accepted as lock
keys.

## CLI

The workspace currently ships `musubi` as a standalone binary in the existing
`iroha_cli` package to avoid adding a new workspace crate or touching
`Cargo.lock`.

Useful local commands:

```bash
cargo run -p iroha_cli --bin musubi -- init --namespace dex.universal --name swap-core --dapp
cargo run -p iroha_cli --bin musubi -- add std.universal/math@1.0.0 --alias math
cargo run -p iroha_cli --bin musubi -- install
cargo run -p iroha_cli --bin musubi -- pack
cargo run -p iroha_cli --bin musubi -- build src/lib.ko --manifest-out target/lib.contract.json
```

`install` currently validates exact dependencies and writes a registry-pending
`Musubi.lock`. `pack` computes the deterministic BLAKE3-256 source archive hash
that the on-chain release record will bind to a SoraFS manifest digest.

`publish --dry-run` and `yank --dry-run` print the release or yank payload. The
chain-side registry instructions, queries, namespace authority checks, and
SoraFS upload flow are the next implementation step.

## Name Squatting Policy

Musubi avoids Cargo-style global name squatting by making
`namespace/package` the only canonical package name. Publishing into a namespace
must be authorized by the same ownership or delegated permission model used for
that Kotodama dapp namespace.

The intended registry policy is:

- packages are canonicalized as `namespace/package`
- releases are immutable; yanking hides a release from new resolution but keeps
  existing lockfiles reproducible
- empty reservations are rejected
- global short aliases are curated governance records, not first-come package
  ownership
- lockfiles record canonical package ids, never short aliases

This means a project can use convenient import aliases locally while the shared
registry remains namespace-owned.
