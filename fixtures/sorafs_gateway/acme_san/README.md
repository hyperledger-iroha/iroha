# SoraDNS ACME/SAN fixtures

These fixtures anchor DG-3 TLS automation by pinning the canonical and wildcard
hosts derived from `cargo xtask soradns-hosts`. They are safe to use with the
self-signed ACME harness (`cargo xtask sorafs-gateway tls renew`) or any
production ACME client when staging GAR updates.

- `docs.sora.san.json` – Example SAN list for `docs.sora` including the
  canonical host, wildcard, and pretty host used by the gateway front door.
  Feed the `san_hosts` entries directly into the TLS automation command:

  ```bash
  cargo xtask sorafs-gateway tls renew \
    --host pkmpnve4b3o5jsbvodhfaqu7rcb4ipimqxcq5x2wjxit5uyp4zla.gw.sora.id \
    --host '*.gw.sora.id' \
    --host docs.sora.gw.sora.name \
    --out artifacts/sorafs_gateway_tls/docs.sora
  ```

- Regenerate the canonical values with
  `cargo xtask soradns-hosts --name docs.sora --json-out -` if the SoraDNS name
  changes or additional aliases are added.
