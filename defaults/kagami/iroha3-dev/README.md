# iroha3-dev sample bundle

- chain id: iroha3-dev.local
- collectors: k=1 r=1
- VRF seed: derived from chain id
- genesis public key: ed012035A459D88E70EE134677B87656A75C33689FFC0EB826287E843835BBE62CC14E
- peers:
- peer 1: public_key=ea0130A1DB3124CEBD4A5A9BBC968D3988FFC0DF0B8E50F9650554FD8599E7D7F07A7C8F7F1CFE1B2CBC397661A48363188EED address=127.0.0.1:1337 pop_hex=88f92a579492d254ebdeb00b9a320a0e30aabebfcef1c15314ac93cc34f34c3cff9331815a00b31f6f311fd4852bbbcd0307c0ff355bc5495f2fcc2e5f161291791af56fcefaf2442942c6170e6529a430ce240eac8183ecf7f300b0ce6479b5

Files:
- genesis.json — generated with `kagami genesis generate --profile iroha3-dev` and patched with deterministic topology+PoPs
- verify.txt — stdout from `kagami verify --profile iroha3-dev --genesis genesis.json`
- config.toml — minimal Nexus config matching the topology (ports 8080/1337)
- docker-compose.yml — single-node snippet mounting the config/genesis

Regenerate:
- cargo xtask kagami-profiles --profile iroha3-dev
