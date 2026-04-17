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
version = "^1.0.0"

[exports]
functions = ["quote"]

[dapp]
namespace = "dex.universal"
contracts = ["router::dex.universal"]
```

Dependencies may use exact versions, caret requirements, tilde requirements,
wildcards such as `1.*`, or comparator lists such as `>=1.0.0,<2.0.0`.
`Musubi.lock` records the full transitive graph selected from the on-chain
registry plus each source archive commitment and deterministic source archive
plan. Lockfile package identities are always canonical package ids; short
aliases are resolved before they enter the lockfile.

## CLI

The workspace ships `musubi` from the dedicated `musubi` package, so the binary
name matches the eventual crates.io install path:

Useful local commands:

```bash
cargo run -p musubi -- init --namespace dex.universal --name swap-core --dapp
cargo run -p musubi -- add std.universal/math --version '^1.0.0' --alias math
cargo run -p musubi -- install --config client.toml
cargo run -p musubi -- install --config client.toml --fetch --provider-payload math.payload
cargo run -p musubi -- install --config client.toml --fetch --gateway-provider 'name=hot-a,provider-id=1111111111111111111111111111111111111111111111111111111111111111,base-url=https://gw.example,stream-token=BASE64,package=math'
cargo run -p musubi -- cache import math --source-root ../math
cargo run -p musubi -- cache fetch math --provider-payload math.payload
cargo run -p musubi -- cache fetch math --config client.toml --gateway-provider 'name=hot-a,provider-id=1111111111111111111111111111111111111111111111111111111111111111,base-url=https://gw.example,stream-token=BASE64'
cargo run -p musubi -- pack --car-out source.car --sorafs-manifest-out manifest.norito --source-plan-out source-plan.norito
cargo run -p musubi -- build src/lib.ko --manifest-out target/lib.contract.json
cargo run -p musubi -- search swap --config client.toml
cargo run -p musubi -- versions dex.universal/swap-core --config client.toml
cargo run -p musubi -- alias resolve swap --config client.toml
```

For source installs from crates.io after publication:

```bash
cargo install musubi
```

`install` resolves dependency requirements against the on-chain Musubi registry
by default, follows transitive Musubi dependencies, and records lockfile v3
nodes with the canonical package ref, selected requirement, SoraFS manifest
digest, source archive hash, byte count, file count, exported functions, archive
plan, and each node's own dependency aliases. Use `install --offline` to write
an unresolved lockfile for exact-version dependencies without querying a node.
Use `install --locked` in CI to reject stale lockfiles.

`cache import` copies a fetched or checked-out source tree into the local
Musubi cache and verifies it against the archive commitment in `Musubi.lock`.
`cache fetch` and `install --fetch` reconstruct a source tree from a verified
provider payload or live SoraFS gateway providers using the lockfile source
archive plan. Local payload files use `--provider-payload <path>`. Live gateway
fetch uses one or more `--gateway-provider` specs:

```text
name=<alias>,provider-id=<64-hex>,base-url=<url>,stream-token=<base64>[,privacy-url=<url>][,package=<alias-or-ref-or-id>][,manifest=<64-hex>]
```

`--provider-payload` and `--gateway-provider` are mutually exclusive for one
fetch operation. If exactly one locked package is missing from the cache, an
unscoped gateway provider can be used. If more than one package is missing,
scope each gateway provider with `package=<dependency-alias>`,
`package=<namespace/package@version>`, `package=<namespace/package>`, or
`manifest=<64-hex SoraFS manifest digest>` so Musubi cannot fetch the wrong
archive for a lockfile node. Runtime gateway options include
`--gateway-client-id`, `--gateway-retry-budget`, `--gateway-max-peers`,
`--gateway-telemetry-region`, and `--gateway-scoreboard-out`.
Stream tokens are runtime credentials; they are not written into `Musubi.lock`.
`build` links cached dependency sources by rewriting calls such as
`math::add()` to deterministic internal Kotodama function names, and rejects
calls to functions that the dependency did not export. Musubi v1 libraries are
function-only: dependency source files containing state declarations, triggers,
kotoba blocks, constants, or other non-function contract items are rejected.

`pack` computes the deterministic BLAKE3-256 source archive hash plus the source
byte and file counts. With `--car-out`, `--sorafs-manifest-out`, or
`--source-plan-out`, it also builds the deterministic SoraFS CAR payload,
SoraFS manifest, and Musubi source archive plan from the same source file set.
`publish --dry-run` prints the release payload. Without `--dry-run`, `publish`
now writes default artifacts under `.musubi/dist/<namespace>/<name>/<version>/`,
rejects digest-only archive submissions, optionally uploads the manifest and
payload through Torii's SoraFS storage-pin endpoint with `--upload`, registers
the generated SoraFS pin, then submits the signed `PublishMusubiRelease`
transaction using the Iroha client config. Publish also parses package `.ko`
sources and rejects exports that are not defined by a Kotodama function in the
source tree. `yank` works the same way for `YankMusubiRelease`.

`search`, `versions`, and `alias resolve` query the same registry. `alias set
--dry-run` prints a curated short-alias binding, and without `--dry-run` submits
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
- global short aliases are not first-come package ownership; setting one
  requires the explicit `CanSetMusubiShortAlias` permission and the target
  package must already have at least one active release, and an existing active
  short alias cannot be silently retargeted
- lockfiles record canonical package ids, never short aliases

This means a project can use convenient import aliases locally while the shared
registry remains namespace-owned.
