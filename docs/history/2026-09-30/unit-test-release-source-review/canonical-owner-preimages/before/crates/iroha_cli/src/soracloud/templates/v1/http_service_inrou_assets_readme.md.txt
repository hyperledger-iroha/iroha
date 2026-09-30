# Inrou Guest Assets

Place the boot assets for this hosted HTTP service here before deploy:

- `x86_64/vmlinux`: Linux kernel image for native `x86_64` guests
- `x86_64/rootfs.ext4`: Debian slim guest root filesystem image for `x86_64`
- `aarch64/vmlinux`: Linux kernel image for native `aarch64` guests
- `aarch64/rootfs.ext4`: Debian slim guest root filesystem image for `aarch64`
- optional `x86_64/initrd.img` and `aarch64/initrd.img`

The generated container manifest references these runtime member paths:

- `/inrou/x86_64/vmlinux`
- `/inrou/x86_64/rootfs.ext4`
- `/inrou/aarch64/vmlinux`
- `/inrou/aarch64/rootfs.ext4`

`app release` packages this directory into an immutable SoraFS artifact,
records the published artifact ref on each guest-image profile, and lets
eligible hosts hydrate `/inrou/*` from the exact authenticated SoraFS artifact
reference carried by the admitted manifest.

The generated workspace deliberately writes `published_artifact: null`; that
strict-null source shape cannot be signed or admitted. Release must publish
every guest ISA and construct concrete refs first. An admitted manifest with a
missing/null ref is rejected, and hosts never trust bundle-local guest files as
a fallback.

Inrou V1 exposes no SSH access field or login path.

The networking policy must be `Isolated`; first-release Inrou workloads cannot
request outbound egress.
