# __SORACLOUD_APP_NAME__ Live API

This service is the hosted HTTP plane for the split app.

The starter server owns:

- `GET /api/v1/health`
- `POST /api/v1/search`
- `GET /api/v1/search/:id/events`
- `GET /api/v1/airports/search`
- `GET /api/v1/filters/metadata`
- `GET /api/v1/luxury/catalog`
- `POST /api/v1/links/resolve`

Mutable live-search state is written to lease-backed directories exposed through
`SORACLOUD_LEASE_VOLUME_*`.
Each root and non-root lease disk is materialized separately for this replica;
the directories are not shared or multi-attached between replicas.

## Local dev

```bash
./dev.sh
```

The starter server falls back to local `tmp/` lease directories, so you can run
the hosted live plane directly without Soracloud while wiring the frontend to
`http://127.0.0.1:8787/api`.

Build:

```bash
./build.sh
```

The build emits `build/live-api.tgz` as a deterministic canonical gzip/USTAR
bundle containing only `app/server.mjs` with mode 0755.

Stage these guest assets under `services/live/inrou/` before deploy:

- `x86_64/vmlinux`
- `x86_64/rootfs.ext4`
- `aarch64/vmlinux`
- `aarch64/rootfs.ext4`

Optional initrd images live at `x86_64/initrd.img` and `aarch64/initrd.img`.

The live container manifest references these runtime member paths:

- `/inrou/x86_64/vmlinux`
- `/inrou/x86_64/rootfs.ext4`
- `/inrou/aarch64/vmlinux`
- `/inrou/aarch64/rootfs.ext4`

`app release` publishes the staged guest-image directory into SoraFS, records
immutable artifact refs in the submitted Inrou manifest, and hydrates hosts
from those exact authenticated SoraFS artifacts.

Inrou V1 exposes no SSH access field or login path. Its networking policy must
be `Isolated`; first-release Inrou workloads cannot request outbound egress.

The generated app manifest references that tarball through `bundle_file`, so:

```bash
iroha soracloud service sync-manifests --app-manifest ../../app_manifest.json
```

will refresh the live service `bundle_hash` automatically.
