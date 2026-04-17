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

The workspace ships `musubi` from the dedicated `musubi` package, so the binary
name matches the eventual crates.io install path:

Useful local commands:

```bash
cargo run -p musubi -- init --namespace dex.universal --name swap-core --dapp
cargo run -p musubi -- add std.universal/math@1.0.0 --alias math
cargo run -p musubi -- install --config client.toml
cargo run -p musubi -- pack
cargo run -p musubi -- build src/lib.ko --manifest-out target/lib.contract.json
cargo run -p musubi -- versions dex.universal/swap-core --config client.toml
cargo run -p musubi -- alias resolve swap --config client.toml
```

For source installs from crates.io after publication:

```bash
cargo install musubi
```

`install` resolves exact dependencies against the on-chain Musubi registry by
default and records the canonical package ref, SoraFS manifest digest, source
archive hash, byte count, and file count in `Musubi.lock`. Use
`install --offline` to write an unresolved exact-version lockfile without
querying a node.

`pack` computes the deterministic BLAKE3-256 source archive hash plus the source
byte and file counts. `publish --dry-run` prints the release payload. Without
`--dry-run`, `publish` submits a signed `PublishMusubiRelease` transaction using
the Iroha client config. Publish also parses package `.ko` sources and rejects
exports that are not defined by a Kotodama function in the source tree. `yank`
works the same way for `YankMusubiRelease`.
`versions` and `alias resolve` query the same registry. `alias set --dry-run`
prints a curated short-alias binding, and without `--dry-run` submits
`SetMusubiShortAlias`.

## Name Squatting Policy

Musubi avoids Cargo-style global name squatting by making
`namespace/package` the only canonical package name. Publishing into a namespace
must be authorized by the same ownership or delegated permission model used for
that Kotodama dapp namespace.

The registry policy is:

- packages are canonicalized as `namespace/package`
- releases are immutable; yanking hides a release from new resolution but keeps
  existing lockfiles reproducible
- empty reservations are rejected because a release must contain a non-empty
  canonical source archive and at least one exported Kotodama function
- package namespaces are enforced on-chain: `<dataspace>` namespaces require an
  active SNS dataspace owner, and `<domain>.<dataspace>` namespaces require the
  registered domain owner
- global short aliases are not first-come package ownership; the current handler
  only permits the target namespace owner as a temporary fallback until the
  governance permission token is wired
- lockfiles record canonical package ids, never short aliases

This means a project can use convenient import aliases locally while the shared
registry remains namespace-owned.
