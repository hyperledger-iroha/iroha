# Sumeragi fixed-scaling preflight inventory drift — 2026-09-23

This is a non-release snapshot of the mutable `optimizations` working tree at
HEAD `b45ec0457eb0664e46c2cdfd8a4b619caefd02b9`, including uncommitted
source edits. The listed current digests were measured for this snapshot and
must be rechecked after any further source change.
It is not a sealed candidate inventory or qualification receipt. The current
`pytests/scripts/scaling_preflight/inventory.json` has 269 source rows;
33 differed from the snapshot filesystem (1 missing path).
The complete-preflight owner rejects before child execution with
`preflight source inventory changed`; the source gate must remain closed.
Regenerate the canonical inventory only after the combined source is frozen,
then execute the full selected suite and independently verify its outcomes.

| Path | Inventory SHA-256 | Current SHA-256 |
|---|---|---|
| `crates/iroha/src/client.rs` | `e9ac3b0e619a0c3a0492c70bb7d28bfe41b2f0fe82335b6e0904fb405a8044b4` | `0aeafc5bc5e81707ad047fac8f79d1c9ef3637ba49b426463b5b54bdb345d178` |
| `crates/iroha/src/client/activation_evidence_tests.rs` | `8105144db5e0031d8228c52df873723c0acac5bc42cbf4cd458e3c23f93f0d96` | `5603bfe40f029a2288bf9b866c40638c5d5df602873a76c7eedad9c85463e875` |
| `crates/iroha_cli/README.md` | `50c9ced3b327dec24bb045eee737284f885cf0464da2966c4f3a2eea1ad526e7` | `46dcc8244cc5f781cb169e277ea24d15e99bb4950054da791800139d69b76c5e` |
| `crates/iroha_cli/src/main_shared.rs` | `85602d62e7a81bc43dd125ed0675c648f1aea7329eccd6999dd7e0f3d19bcfec` | `e27567936017f702bba23718e24d7a0a9bf4fd238993da04627ec73fc99de622` |
| `crates/iroha_cli/src/transaction_load/collect_inputs/fixture.rs` | `c62dbda67a49d94b4ace1cefd26ecd361f34c405a9e783f178359e530ae2fb79` | `b828c721428f23e697602432fa7a519d3bcc1c7e72708e43009a7736142d2ba2` |
| `crates/iroha_cli/src/transaction_load/output/canonical_inputs/tests.rs` | `818de94c1effc1cdb242a29449bb7832124600fae93cd1e2e0d7027e8b3b8b01` | `763814e06c9d86f9f44455c8f2a349ddbc9b0fdfb5491dcac679a30311420543` |
| `crates/iroha_core/src/state.rs` | `5789cb9e235db7af432b65e6dd69ea539241156af368384b0b5c018ff87f7f6a` | `8a35846fd95d59daa379538a1e8030983e38c4f6ce96925a174506c355514bee` |
| `crates/iroha_core/src/state/strict_replay_tests.rs` | `19e53f96bb446227260dc2cdf5cca97cbc7adce6d18e0086affa0aec46841dc6` | `2732141361eb1978faf9160e98ae5dd7c22b9b41def922c009780d70c8572a15` |
| `crates/iroha_core/src/state/tests.rs` | `6515d088f2788442e2c89d1f5d5d188671a98f4299bbed6cee0f8dc2f4419f36` | `09669e72b67842917ca10238577fb145fc6cf818848a5bc007791db68ec7c1cd` |
| `crates/iroha_core/src/sumeragi/mod.rs` | `4473a9754ef1b1496b56a8d4e68b63cbef39aff3e4340d4b90e50f5e04ffd1f4` | `54e36439533afec8f9dfc98e1f23e46490bf0e9a9cff29afd560160acdc918b4` |
| `crates/iroha_core/src/sumeragi/tests/v2_runtime_pending_binding_cases.rs` | `8683ae95503f38637e23d97aeb97163740cdfb36cfcd15520e87d4259343e261` | `ee242159678a494bd3924d38367dee3a3cd17e505790964228f916db6ee361e9` |
| `crates/iroha_core/src/sumeragi/v2_context.rs` | `ea7b042f200b9dcb89a783bd24ca5eaa4cba01198f0fd234df94b1e435d0b028` | `3c0fe3e6ca03c93416e1df598a1f28eb5e71dad9f5dcada0ef099e5bf69c2e82` |
| `crates/iroha_crypto/src/hash.rs` | `3f36c68b506d5f24fdfbc5e20062f3e59f2d800d6715e26ff4cc83987ba37dbd` | `34f9398701857ef1be0aff4bbca0020f0d64d1e0ecdaeff286dd257b0ca74355` |
| `crates/iroha_kagami/CommandLineHelp.md` | `c9814f7efbc0a9a34acb6dba0df1864f1f87f8de407b8f7e687a705cafa64393` | `505cb5768b9f0772035d71f2e2810241086d748f9fb57dc4c737aaab4077160a` |
| `crates/iroha_kagami/src/genesis.rs` | `c95df915825059d60c19fc7af3c18a531020e15d12ecb8a2d00a06267289ced1` | `2de09c1c8c26043cfeaa6ae2d1a398a37d58ce8ffe1d259a7ea89403e86a643c` |
| `crates/iroha_kagami/src/genesis/sign.rs` | `7fe58496e392fa5ff05f8cfe34c971d4e4816374ec219730d46b71491d08c575` | `ef2338fbcf6ed407e0ced52e921914a19297e5f92bff14fbe9e7c9bf71776a2d` |
| `crates/iroha_kagami/src/kagemusha/derive_mint_finality_next_epoch_v1.rs` | `32d5c45f3531e9e7cb5c3e46b3e8ff714bebc8d65c1d0b01070e4537e03f865b` | `MISSING` |
| `crates/iroha_kagami/src/kura.rs` | `1db62d2e5c20b6e912838ece2fe4c55356d4e78707fd00512f89f74d956c56fa` | `edf657cf63b557d042d928d7888bfa1da0849e9ebd2e35dbed6364de5de97f05` |
| `crates/iroha_kagami/src/kura/scaling_evidence/fixture.rs` | `6c3c3e0b445b201d7fda2771c9e4ad4f0fb630335fd840d11f48d00b3c26b6af` | `b6228d997874d479157b26f93d7edcb2cb1baf74b2ba608b9414d370c019e846` |
| `crates/iroha_kagami/src/localnet.rs` | `f2ffcf1dd268c5b1263a25baf434842cbe64507eaeee54cd6897ef7461f145a4` | `7ecde49f48ab3d4c7992e05fa015666bca9db40fe584a2a4bbc4f873aa8f2010` |
| `crates/iroha_torii/assets/openapi/torii.json` | `4297eb710aae2b6d564ec197ef609c88100268549e0265a69867fc6e068d5c5b` | `79df0660996cbf6bb261506657b6fdb383345752dba0c51f1bb15a47496dcea4` |
| `crates/iroha_torii/src/lib.rs` | `ee7b319777cdcd7508cc47ed06c65db36cfc8c43aaf271f470aaf8b92ebdeedf` | `0add90312796c86286ed07ed287de49fe20b42f8ec85a6d346e945d247c313c1` |
| `crates/iroha_torii/src/openapi/tests/diagnostics_schemas.rs` | `bd1e69bb47652fbdc69b5b10eb95910834bcf6bd777b8229049f6da1fc0343c6` | `83cd7d23368efc6855a66e3a9d2f7958c909fae1261c5cd320d062594121b6a4` |
| `crates/iroha_torii/src/routing.rs` | `c46e31373de473d7a605fb73e69e16a1b624c8bb70bfab78f362d966a95bdf00` | `65acfe4e05c29d136930be77e331a77522981be7d3e8413a77d25b61bc69762e` |
| `crates/iroha_torii_shared/src/lib.rs` | `a1422c40a97d3e2de3937b71a06f89c6f97c110a522611ce41c5f863114c2e1f` | `2ff556676c941f89dd938b28aed5a8abf2ddff3d778fde333bf0e8b30eb644ad` |
| `docs/source/taira_dataspace_deploy.md` | `a678a86333d0bd6837d897d001d9b6d9c127bc87cb4821a1b325971db1c07d11` | `d3127ddedfd5b5b3123783192e71030b374a00878e418d07329012115ac667d5` |
| `pytests/scripts/sumeragi_v2_release_bootstrap_test.py` | `1a7de567a1b377dd3ecdd87a3f9ace07ae055bf98e072fef8a5f042d3e24ba17` | `a261b5c38cf9e8c584bc8c5b347cbf7c94d8b7e97f2bba6db909e2cb99dd72ac` |
| `scripts/bootstrap_sumeragi_v2_release.py` | `42902589fe5175f22f13a0881684f721c0cfd72fa0cb3048f1b413f3aa6433d2` | `3a1e4d2ae83b189d26621da1b8f42d51c8d5cfb5e50b05d951adaf55020175f9` |
| `scripts/formal/sumeragi_v2_proof_ledger_release_inventory_contracts.py` | `a6265d481f3bc2988ffe0b022b3e6645aa396c05705d6969cf30adf6ef58d061` | `d8a454f99be86e503110cc7bf4f78388a4605f34926fec6716f7262b8ea87e2a` |
| `scripts/run_sumeragi_v2_release_gates.sh` | `0ccddc273146893cbff65e5d621f44235a3ffccb4b2e30e1dd26f36b82064ef2` | `b6d5901daddd981ceb92d79b5f02f2baaa7697037cda8d4340397da2620014a8` |
| `scripts/validate_sumeragi_v2_release_bootstrap.py` | `bdc14744f7b66f98f54fbb9f75f933cd36db3e7fe54d2d2201e80de163f1f2d6` | `757261b184d3e165c1e1036025edd9a7526e81a52089d74206b475dd1929781e` |
| `scripts/write_sumeragi_v2_release_receipt.py` | `384bd35a5ccba4b8d1e6570107c41c8fe23cbca5f28fdce9c86a82cdd633c0b2` | `6d0291d6a663da09da0d11340b640bb8eba0d26768ff66e8615bb7dd8479fc07` |
| `scripts/write_sumeragi_v2_release_receipt_corridor_log.py` | `b464b36f2ad4bf07c7ec969f14d97b7d29b99e27dd74b1f47ecd1dcaabf0014c` | `0ce65185b396f72cf66b2929b0564f2d33175275ae9335b5dca86c8fc504b697` |
