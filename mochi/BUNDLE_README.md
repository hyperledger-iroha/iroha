# Mochi native developer bundle

The bundle contains matching native executables for the desktop, developer CLI
and four-validator runtime:

```text
bin/mochi              # desktop
bin/kagami             # developer CLI and managed process owner
bin/iroha3d            # matching validator
docs/README.md        # this guide
LICENSE
manifest.json          # file inventory with sizes and SHA-256 digests
```

Executable names end in `.exe` on Windows. Keep all three binaries together.

## Start from a project

```sh
/path/to/bundle/bin/kagami localnet up
/path/to/bundle/bin/kagami contract deploy hello.ko
/path/to/bundle/bin/kagami localnet down
```

No supplied TOML, source checkout, build tools or container runtime is needed.
Deployment accepts `.ko`, `.to` and existing Musubi packages. With no selected
context, deployment starts the default localnet in the same invocation.

Open the desktop for the same project with:

```sh
/path/to/bundle/bin/mochi --workspace /path/to/project
```

Omit `--workspace` to use the current directory. Both frontends call the same
services and select the same retained context. The desktop does not own a
separate network generator or supervisor.

## Retained state

Generated configuration, keys, journals and ledger files stay in private OS
application storage outside the project. Each workspace has its own selection.
Repeated `localnet up` and `down`/`up` retain the same network and account.
Repeated unchanged deployment recovers its original evidence and verifies
current code and alias state.

Use `kagami context list`, `context show`, `context use <name>`,
`localnet status` and `localnet logs` to inspect the environment.
`localnet reset local` explicitly retires a stopped generation; a subsequent
`localnet up` creates a fresh ledger. Reset does not transfer old funds or
contract state.

Advanced operator bundle generation is `kagami localnet generate`; managed
startup needs none of its input files or generated scripts.

## Artifact verification and qualification

`manifest.json` records every packaged file and its size and SHA-256 digest.
Release provenance must authenticate the manifest before those digests establish
trust in a downloaded bundle.

The release acceptance gates include clean-install runs on macOS/Linux ARM64
and x86-64 and Windows x86-64. Source compilation and component tests do not
establish those native runtime results. Private dataspace attachment and startup
latency remain separate acceptance gates in the repository's developer-experience
specification.
