# Nix AppImage bundler

The repository bundles Linux derivations as a Type 2 AppImage: a statically
linked runtime followed by a Zstandard SquashFS image. The image contains the
complete derivation closure, the existing user-namespace/chroot `AppRun`, an
explicit executable symlink, and a mode-0755 mount root. Iroha's entrypoint is
`iroha3d`; the bundle filename remains `iroha3-<architecture>.AppImage`.

The locked graph uses the supported NixOS 26.05 package set, maintained
[AppImage/type2-runtime](https://github.com/AppImage/type2-runtime) source at
`fedfad51acc2860b2916a1eef32ba4a500ff31ef`, Squashfuse 0.6.5, and libfuse 3.18.2.
The libfuse override uses the source/hash published by current nixpkgs; all
runtime libraries are built statically. The runtime requires Squashfuse's
private headers, which are installed from the same locked source as its
libraries. Its Clang/compiler-rt toolchain follows upstream and supplies the
self-relocating musl startup code required by static PIE. There is no retired
AppImageCrafters runtime or FUSE 2 dependency.

`flake.lock` hashes were recomputed from the immutable upstream GitHub archives
using `nix-nar` 0.6.0. The same serializer reproduced the previous runtime,
Squashfuse, and flake-compat lock hashes and flake-utils' systems dependency hash
before the current source hashes were recorded. Nix 2.34.3 subsequently accepted
the unchanged lock during the Linux validation below.

## Validation

Run on each supported Linux architecture (`x86_64-linux` and `aarch64-linux`):

```sh
nix flake lock ./nix-appimage --no-update-lock-file
nix build ./nix-appimage#checks.$(nix eval --impure --raw --expr builtins.currentSystem).type2-structure --no-update-lock-file
nix build ./nix-appimage#smoke --out-link result-appimage-smoke --no-update-lock-file
APPIMAGE_EXTRACT_AND_RUN=1 ./result-appimage-smoke
./result-appimage-smoke
nix build .#appimage --out-link result-iroha-appimage
APPIMAGE_EXTRACT_AND_RUN=1 ./result-iroha-appimage --version
./result-iroha-appimage --version
```

The structure check verifies Type 2 magic, absence of a dynamic ELF interpreter
and shared-library requirements, runtime version execution, extraction, the
executable symlink, and mount-root permissions. The two smoke invocations then
exercise extraction and FUSE mounting through the real `AppRun`; they must print
`Hello, world!`. Iroha must report its daemon version through both paths.
The archive must contain a mode-0755 mount root; the upstream extractor creates
its private extracted directory with mode 0700. Both permissions are checked
independently.
Unprivileged user namespaces and an operational FUSE mount helper are required
for those execution checks. On Ubuntu with restricted unprivileged user
namespaces, AppArmor must explicitly allow `userns` for the admitted image;
otherwise `AppRun` fails while establishing its UID mapping. Grant permission
only to the reviewed executable, preserving the global restriction.

The locked runtime, structure check, and both hello execution paths passed on
Ubuntu 26.04 aarch64 with Nix 2.34.3. Qualification used an AppArmor profile
attached to the exact immutable test-image path and retained the global
user-namespace restriction. The x86_64 checks and full Iroha daemon AppImage
remain separate release qualification requirements.

To update sources, update the explicit upstream pins and run
`nix flake update --flake ./nix-appimage` on a Nix-enabled host, review the lock
and source changes, and repeat the checks above. The repository release-feature
guard verifies that the bundle still contains the reviewed Cargo derivation
and daemon entrypoint; the helper cannot introduce Cargo features.
