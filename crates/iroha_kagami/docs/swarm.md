# Iroha Swarm

Tools for generating Docker Compose configuration for Iroha.

## Usage

```bash
kagami swarm [OPTIONS] --peers <COUNT> --config-dir <DIR> --image <NAME> --out-file <FILE>
```

### Options

- `-p, --peers <COUNT>`: Specifies the number of peer services in the configuration.

- `-s, --seed <SEED>`: Sets the UTF-8 seed for deterministic key-generation.

- `-H, --healthcheck`: Includes a healthcheck for every service in the configuration. 
  - Healthchecks use predefined settings. 
  - For more details on healthcheck configuration in Docker Compose files, see: [Docker Compose Healthchecks](https://docs.docker.com/compose/compose-file/compose-file-v3/#healthcheck).

- `    --peer-config <FILE>`: Loads peer overrides from a TOML file.
  - The file provides the human-readable service names and external port mappings for each peer.
  - Example:
    ```toml
    [[peers]]
    name = "alpha"
    p2p_port = 2000
    api_port = 9000

    [[peers]]
    name = "beta"
    p2p_port = 2001
    api_port = 9001
    ```
  - The number of entries must match `--peers`.

- `-c, --config-dir <DIR>`: Sets the directory with Iroha configuration. 
  - It will be mapped to a volume for each container. 
  - The directory should contain `genesis.json`. If you plan to upgrade the executor at genesis,
    include the executor bytecode file and reference it from `genesis.json`.

- `-i, --image <NAME>`: Specifies the Docker image used by the peer services. 
  - By default, the image is pulled from Docker Hub if not cached. 
  - Pass the `--build` option to build the image from a Dockerfile instead. 
  - **Note:** Swarm only guarantees that the Docker Compose configuration it generates is compatible with the same Git revision it is built from itself. Therefore, if the specified image is not compatible with the version of Swarm you are running, the generated configuration might not work.

- `-b, --build <DIR>`: Builds the image from the Dockerfile in the specified directory. 
  - Do not rebuild if the image has been cached. 
  - The provided path is resolved relative to the current working directory.

- `    --no-cache`: Always pull or rebuild the image even if it is cached locally.

- `-o, --out-file <FILE>`: Sets the path to the target Compose configuration file. 
  - If the file exists, the app will prompt its overwriting. 
  - If the TTY is not interactive, the app will stop execution with a non-zero exit code. 
  - To overwrite the file anyway, pass the `--force` flag.

- `-P, --print`: Print the generated configuration to stdout instead of writing it to the target file.

- `-F, --force`: Overwrites the target file if it already exists.

- `    --no-banner`: Do not include the banner with the generation notice in the file.
  - The banner includes the passed arguments in order to help with reproducibility.

- `    --consensus-mode <MODE>`: Stamp the target consensus mode (`permissioned` or `npos`) into the generated Compose env so the signing sidecar passes it to `kagami genesis sign`.

- `    --next-consensus-mode <MODE>`: Stage a future consensus mode behind `--mode-activation-height` (Iroha2 only; Iroha3 disallows staged cutovers).

- `    --mode-activation-height <HEIGHT>`: Activation height for `--next-consensus-mode`; requires `--next-consensus-mode` and forwards the height to the signing sidecar.

## Examples

Generate a configuration with 4 peers, using `Iroha` as the cryptographic seed, using `./peer_config` as a directory with configuration, and using `.` as a directory with the Iroha `Dockerfile` to build a `myiroha:local` image, saving the Compose config to `./my-configs/docker-compose.build.yml` in the current directory: 

```bash
kagami swarm \
    --peers 4 \
    --seed Iroha \
    --peer-config ./peer_overrides.toml \
    --config-dir ./peer_config \
    --image myiroha:local \
    --build . \
    --out-file ./my-configs/docker-compose.build.yml
```

Generate the same configuration, but use an existing image pulled from Docker Hub instead. The output is printed to stdout (notice how the target path still has to be provided, as it is used to resolve the config and build directories):

```bash
kagami swarm \
    --peers 4 \
    --seed Iroha \
    --healthcheck \
    --config-dir ./peer_config \
    --image hyperledger/iroha:dev \
    --out-file ./my-configs/docker-compose.pull.yml \
    --print
```

### NPoS devnet (Docker)

1. Build or reuse an NPoS genesis manifest. For Iroha3, omit staged cutover flags (for example `kagami genesis generate --consensus-mode npos --ivm-dir <ivm> --genesis-public-key <pk>` or `kagami localnet --consensus-mode npos ...`). For Iroha2 cutovers, add `--consensus-mode permissioned --next-consensus-mode npos --mode-activation-height 5`.
2. Place `genesis.json` and peer configs in `--config-dir` (PoPs/topology can be injected at sign time with `--topology`/`--peer-pop` as described in the README).
3. Run:

```bash
kagami swarm \
    --peers 4 \
    --seed Iroha \
    --config-dir ./peer_config \
    --image hyperledger/iroha:dev \
    --consensus-mode permissioned \
    --next-consensus-mode npos \
    --mode-activation-height 5 \
    --out-file ./my-configs/docker-compose.npos.yml
```

The generated Compose file forwards the consensus mode, next mode, and activation height to the signing sidecar so the final signed genesis carries both `next_mode` and `mode_activation_height`. Use a fixed `--seed` and set `sumeragi_npos_parameters.epoch_seed` in `genesis.json` if you need deterministic VRF schedules for testing.
For Iroha3, drop `--next-consensus-mode`/`--mode-activation-height` and keep `--consensus-mode npos`.

## NPoS devnet workflow

1. Iroha2-only staged cutover: produce an NPoS-ready genesis manifest (includes `sumeragi_npos_parameters`) with a staged cutover height. For Iroha3, omit the cutover flags and use `--consensus-mode npos`.

   ```bash
   kagami genesis generate \
       --consensus-mode permissioned \
       --next-consensus-mode npos \
       --mode-activation-height 5 \
       --ivm-dir ./ivm_libs \
       --genesis-public-key <GENESIS_PK> \
       > ./cfg/genesis.json
   # Optional: set a reproducible VRF seed for testing
   jq '.transactions[0].parameters.custom.sumeragi_npos_parameters.epoch_seed = [1,2,3,4]' ./cfg/genesis.json > /tmp/genesis.npos && mv /tmp/genesis.npos ./cfg/genesis.json
   ```

2. Sign with your BLS roster and PoPs so validators carry Proofs-of-Possession in the final block:

   ```bash
   TOPOLOGY='["bls_normal:pk1","bls_normal:pk2","bls_normal:pk3"]'
   kagami genesis sign ./cfg/genesis.json \
       --consensus-mode permissioned \
       --next-consensus-mode npos \
       --mode-activation-height 5 \
       --topology "$TOPOLOGY" \
       --peer-pop "bls_normal:pk1=pop_hex1" \
       --peer-pop "bls_normal:pk2=pop_hex2" \
       --peer-pop "bls_normal:pk3=pop_hex3" \
       --private-key <GENESIS_SK_HEX> \
       --out-file ./cfg/genesis.signed.nrt
   ```

3. Render the Docker Compose file; Swarm refuses to proceed if `genesis.json` is missing `sumeragi_npos_parameters`, so the compose workflow always carries the NPoS parameters:

   ```bash
   kagami swarm \
       --peers 4 \
       --consensus-mode permissioned \
       --next-consensus-mode npos \
       --mode-activation-height 5 \
       --config-dir ./cfg \
       --image hyperledger/iroha:dev \
       --out-file ./my-configs/docker-compose.npos.yml
   ```

Use the same roster/PoPs when starting the containers to avoid mismatches between the signed genesis and node configs.

## Note on configuration structure

When using the `--build` option, the first peer in the generated configuration builds the image, while the rest of the peers depend on it. This is needed to avoid redundant building of the same image by every peer.
