ZK CLI Samples

This directory contains small, ready-to-use samples for the ZK app API commands exposed via `iroha_cli`.

Files
- `vk_register.json` — JSON body for `iroha zk vk register --json vk_register.json` (uses base64 `AQID`, i.e. `[1,2,3]`)
- `vk_update.json` — JSON body for `iroha zk vk update --json vk_update.json`
- `vk_deprecate.json` — JSON body for `iroha zk vk deprecate --json vk_deprecate.json`
- `proof.json` — tiny JSON proof-like payload for attachments upload
- `zk1_min.b64` — base64 of a minimal ZK1 Norito envelope (4 bytes: `ZK1\0`)

Commands (examples)

1) Verifying Key registry (register/update/deprecate)

```bash
# Register (uses vk_bytes as base64 placeholder)
iroha zk vk register --json vk_register.json

# Update (version must increase)
iroha zk vk update --json vk_update.json

# Deprecate (marks as Deprecated; retained up to per‑backend cap)
iroha zk vk deprecate --json vk_deprecate.json

# Read a VK record
iroha zk vk get --backend halo2/ipa --name vk_add
```

2) Attachments (upload/list/get/delete)

```bash
# Upload JSON payload (Content-Type must match)
iroha zk attachments upload --file proof.json --content-type application/json

# List attachments
iroha zk attachments list

# Download an attachment (replace <ID> with actual id)
iroha zk attachments get --id <ID> --out downloaded.bin

# Delete an attachment
iroha zk attachments delete --id <ID>
```

3) Norito (ZK1) envelope upload and prover reports

The prover classifies Norito uploads by checking for the ZK1 magic prefix (`ZK1\0`). Use `zk1_min.b64` to create a binary payload:

```bash
# Linux: base64 --decode
base64 --decode zk1_min.b64 > zk1_min.bin

# macOS (BSD): base64 -D
base64 -D zk1_min.b64 > zk1_min.bin

# Upload the minimal ZK1 envelope (4 bytes)
iroha zk attachments upload --file zk1_min.bin --content-type application/x-norito

# List prover reports (non-consensus)
iroha zk prover reports list

# Fetch a single report (replace <ID> with attachment id)
iroha zk prover reports get --id <ID>

# Count reports (server-side filters)
iroha zk prover reports count
iroha zk prover reports count --content-type application/x-norito --has-tag PROF
```

4) Vote tally helper

```bash
iroha zk vote tally --election-id demo-election-1
```
