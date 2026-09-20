# Source-bound Sumeragi release binaries

The maintained `scripts/sumeragi_v2_prebuilt_bundle.sh` builder publishes five
mandatory executables through `scripts/sumeragi_v2_prebuilt_bundle.py`:

| Manifest role | Bundle path | Export |
| --- | --- | --- |
| `irohad` | `release/iroha3d` | `TEST_NETWORK_BIN_IROHAD` |
| `irohad_message_control` | `message-control/release/iroha3d` | `TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL` |
| `iroha` | `release/iroha` | `TEST_NETWORK_BIN_IROHA` |
| `kagami` | `release/kagami` | `KAGAMI_BIN` |
| `irohad_taira` | `release/iroha3d_taira` | `TEST_NETWORK_BIN_IROHAD_TAIRA` |

The production Taira launcher and standard daemon are distinct artifacts built
in the same default feature graph. The message-control daemon retains its
separate feature graph and cache. All five must pass the existing source,
toolchain, digest, size, ownership, mode, single-link and closed-directory checks.
The version-2 manifest has exactly 29 ordered fields. Four-entry bundles are
invalid; receipt publication and replay enforce the same five-artifact inventory.
Inherited authenticated bundles are verified and reused without rebuilding.
