# Bare-metal local swarm

Run `scripts/run_local_swarm.sh --help` from the repository root. It requires Bash,
Python 3 and either Cargo or existing `target/release/{iroha3d,iroha,kagami}` binaries.
It creates four bare-metal peers using the script's existing local demo identities.
`BASE`, ports, build choice and storage reset retain their existing meanings.
Both existing `GENESIS_PUBLIC_KEY_FILE` and `GENESIS_PRIVATE_KEY_FILE` are now
mandatory, together with `GENESIS_CREATION_TIME_MS` as canonical unsigned 64-bit
Unix milliseconds. The genuine signer receives that exact explicit timestamp.
The script creates no fresh genesis signing key and uses no implicit wall-clock
network identity. `RESET_STORAGE` defaults to `0`; prior storage is retained. Explicitly set it
to `1` to delete prior storage. Every other reset value is rejected before any
output or build.

Set `KAGEMUSHA_LOAD_AUTHORIZER_CUSTODY_DIR` to an existing canonical absolute
owner-only directory outside `BASE`. Supply eight original operator-provisioned
private files: `peer0-keyring.nrt` through `peer3-keyring.nrt`, and
`peer0-submitter-private-key` through `peer3-submitter-private-key`.
The directory and files must belong to the launching effective UID. Directory
permissions must allow owner read/search and no group/other access. Files must
be readable, regular, single-link, nonempty and inaccessible to group/other.
Symlinks and noncanonical directory paths are refused. FIFO and other special
files are opened nonblocking and refused before any byte read. Keyring files are limited
to 65,536 bytes and submitter files to 4,096 bytes, matching the actual parser.

These files are mandatory in every launch. The script checks only filesystem
metadata, retains absolute file references, and does not print, copy or generate
publisher secret bytes. It does not derive Load-role authority from the demo
seed, peer identity or genesis signer. Each keyring must contain genuine
network-bound certified Load keys, and each submitter must have ordinary ledger
permissions and finite fee admission. The operator must bind custody to the exact
signed genesis network used for this launch. The exact manifest, signer and explicit timestamp must be the originals used
to admit that keyring. The current real signer has a repeated-byte test for
explicit timestamp inputs; changing any signed input changes the required
network binding. An existing signing pair and timestamp do not themselves
certify a publisher keyring. Prepare and certify these originals through the
network's genuine operator ceremony before invoking this launch script.

Missing or unsafe custody fails before creating `BASE`, building binaries or
resetting storage. `BASE` must be an owner-only directory. All generated stop,
config, client, genesis, log and PID paths are preflighted against symlinks,
hardlinks, foreign ownership and group/other writes before any output is changed.
Existing private configs/scripts with public permissions are refused. Generated
private configs and stop script use a native no-follow writer that checks the
opened inode before truncation; neither a symlink nor public existing file can
be silently followed or repaired after losing the original bytes. Configs use
the exact, safely escaped original custody paths and owner-only permissions. After generating configs, the script
rechecks custody metadata and runs the actual daemon's `--check-config` against
all four configurations before starting a peer. The daemon retains its original
private-key parsing, native genesis and runtime publisher admission. Offline
configuration checks do not substitute for runtime role certificates, fee
admission, live revocation or finalized publication. The original files remain
mutable under their owner; a preflight check does not reserve authority or freeze
future reads. Keep them stable through launch and use the daemon's ordinary
operator procedure for later custody changes.

The existing generated `stop.sh` checks process ownership before signaling peers.
Use `cd "$BASE" && ./stop.sh` to stop a launched swarm.
