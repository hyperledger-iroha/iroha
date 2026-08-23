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
eligible hosts hydrate `/inrou/*` from SoraFS. Distribution defaults to global
and may target explicit geography tags; unknown host geography falls back to
lower observed latency.

SSH access is disabled by default with `ssh_authorized_keys: []`. Add a real,
trusted public key only when SSH access is intentionally required.

Outbound network access is isolated by default. Inrou V1 accepts only isolated
egress or a nonempty allowlist resolving exclusively to public endpoints.
