# iroha3-dev sample bundle

- chain id: iroha3-dev.local

- VRF seed: derived from chain id
- deterministic genesis creation-time base (ms): 1700000000000
- genesis public key: ed012067822E5566A0F5A2DDBD8745CFCAA215EB684D2D8BA200EA9F2DD9B247B1A306
- peers:
- peer 1: public_key=ea0130A1DB3124CEBD4A5A9BBC968D3988FFC0DF0B8E50F9650554FD8599E7D7F07A7C8F7F1CFE1B2CBC397661A48363188EED address=172.28.0.10:1337 pop_hex=88f92a579492d254ebdeb00b9a320a0e30aabebfcef1c15314ac93cc34f34c3cff9331815a00b31f6f311fd4852bbbcd0307c0ff355bc5495f2fcc2e5f161291791af56fcefaf2442942c6170e6529a430ce240eac8183ecf7f300b0ce6479b5
- peer 2: public_key=ea013095F9E716AB9F2F0670D588FBFA5420AC598CFFB58F0AEF969543D7BBCF81CE132A2A20E813A1CC0647B4B179B76B09CD address=172.28.0.11:1338 pop_hex=9590b9ef26cc1eb1000cc7c7b7532ee8b166a0d899c5ee7e088f53ded39e0faf57cc7f2d9ed4261ae3b278f094fe443100e5672f2ee4f3d52f89c769a72b4d5feda007835cfd404fde03afc6e2d8258c9a7fee1b8cb64625e001fcbfb0089152
- peer 3: public_key=ea01309753D5AA860989AD67EC4FB2A14558A879AACC538A4C656CCDF2FE8D646CD5B6DF7E32F397A91CD371951645D89ED917 address=172.28.0.12:1339 pop_hex=992e8cae69b3297999f88b0b9b2c646713f88889b5204ad0ee9c94fe93f485b9e3732724673cc104a57ffa80cad413b703232c18cf57c7f5a4c0281f08cfdc03816db298fef45874ab4fa383037e8d6c991fd61f760b4cb37f87dde7e2b1d747
- peer 4: public_key=ea013089E0110B3F07AB1CEFAC930756E4DBC25E37D606E4D03513BF4C744DA0F6134DE8B7E52749511C823C230672F736C304 address=172.28.0.13:1340 pop_hex=810957f8c36f3db895f02157573187fb4aadc21256c9cb837369697a2d605942ea8b53880c8f7842d4b7e58ac037cc340afbae35f05adbaad21782e9c80a952318f217e197a8ecbb7ddf4af4633bab3e8256663244aa3fc12c2a08cf93436776

Files:
- genesis.json — generated with `kagami genesis generate --profile iroha3-dev`, patched with deterministic topology+PoPs, and rebound to the exact staged Nexus/AMX context through `kagami genesis sign`
- genesis.signed.nrt — canonical signed genesis wire artifact consumed by every validator
- genesis.public_key — canonical one-line verifier key for the signed genesis artifact
- genesis.expected_hash — canonical checked `hash:<64 uppercase hex>#<CRC16>` NetworkId encoding the independently provisioned signed-header hash
- verify.txt — stdout from `kagami verify --profile iroha3-dev --genesis genesis.json`
- config.toml and config-peer-*.toml — compatibility names for the generated validator configs
- peer0.toml through peerN.toml — canonical prepared-bundle validator configs
- docker-compose.yml — full validator committee mounting the shared genesis and per-peer configs

Regenerate:
- cargo xtask kagami-profiles --profile iroha3-dev
