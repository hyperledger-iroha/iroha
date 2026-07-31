# Kagami Docker Compose

Tools for generating Docker Compose configuration for Iroha.

## Usage

```bash
kagami docker [OPTIONS] --peers <COUNT> --config-dir <DIR> --image <NAME> --out-file <FILE>
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

The generated Compose manifest intentionally contains no genesis signing key.
Every service reads the verifier key from a Docker Compose secret, and only
`irohad0` receives the signing-key secret. Before evaluating the manifest, set
both source paths:

```bash
export IROHA_GENESIS_PUBLIC_KEY_FILE="$PWD/localnet/genesis.public_key"
export IROHA_GENESIS_PRIVATE_KEY_FILE="$PWD/localnet/genesis.private_key"
docker compose -f ./my-configs/docker-compose.yml up
```

`kagami localnet` emits both files and protects the private file with
owner-only permissions. If you prepare genesis manually, create the private
file as one canonical private-key multihash plus a final newline with mode
`0600`, and create the public file as its matching public-key multihash plus a
final newline. Compose refuses to evaluate when either path is unset. The
in-container signer also rejects a private key that does not derive the
supplied public key. Never commit the private file.

- `-i, --image <NAME>`: Specifies the Docker image used by the peer services. 
  - By default, the image is pulled from Docker Hub if not cached. 
  - Pass the `--build` option to build the image from a Dockerfile instead. 
  - **Note:** Kagami only guarantees that the Docker Compose configuration it generates is compatible with the same Git revision it is built from itself. Therefore, if the specified image is not compatible with the version of Kagami you are running, the generated configuration might not work.

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

## Examples

Generate a configuration with 4 peers, using `Iroha` as the cryptographic seed, using `./peer_config` as a directory with configuration, and using `.` as a directory with the Iroha `Dockerfile` to build a `myiroha:local` image, saving the Compose config to `./my-configs/docker-compose.build.yml` in the current directory: 

```bash
kagami docker \
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
kagami docker \
    --peers 4 \
    --seed Iroha \
    --healthcheck \
    --config-dir ./peer_config \
    --image hyperledger/iroha:dev \
    --out-file ./my-configs/docker-compose.pull.yml \
    --print
```

### NPoS devnet (Docker)

1. Build or reuse a genesis manifest whose signed consensus mode is `npos` (for example `kagami genesis generate --consensus-mode npos --vrf-seed-hex <64_HEX_DIGITS> --ivm-dir <ivm> --genesis-public-key <pk>` or `kagami localnet --consensus-mode npos ...`).
2. Place `genesis.json` and peer configs in `--config-dir` (PoPs/topology can be injected at sign time with `--topology`/`--peer-pop` as described in the README).
3. Run:

```bash
kagami docker \
    --peers 4 \
    --seed Iroha \
    --config-dir ./peer_config \
    --image hyperledger/iroha:dev \
    --out-file ./my-configs/docker-compose.npos.yml
```

The generated Compose file reads and reports the immutable consensus mode from `genesis.json`.
Use a fixed `--seed` and a fixed `--vrf-seed-hex` when generating the manifest if you need deterministic VRF schedules for testing.

## NPoS devnet workflow

1. Produce an NPoS genesis manifest. The generator records `npos` as the immutable consensus mode and includes `sumeragi_npos_parameters`.

   ```bash
   kagami genesis generate \
       --consensus-mode npos \
       --vrf-seed-hex <64_HEX_DIGITS> \
       --ivm-dir ./ivm_libs \
       --genesis-public-key <GENESIS_PK> \
       > ./cfg/genesis.json
   ```

2. Sign with your BLS roster and PoPs so validators carry Proofs-of-Possession in the final block:

   ```bash
   TOPOLOGY='["bls_normal:pk1","bls_normal:pk2","bls_normal:pk3"]'
   kagami genesis sign ./cfg/genesis.json \
       --topology "$TOPOLOGY" \
       --peer-pop "bls_normal:pk1=pop_hex1" \
       --peer-pop "bls_normal:pk2=pop_hex2" \
       --peer-pop "bls_normal:pk3=pop_hex3" \
       --private-key-file <MODE_0600_GENESIS_KEY_FILE> \
       --expected-public-key <GENESIS_PUBLIC_KEY> \
       --out-file ./cfg/genesis.signed.nrt
   ```

3. Render the Docker Compose file; Kagami refuses to proceed if `genesis.json` is missing `sumeragi_npos_parameters`, so the compose workflow always carries the NPoS parameters:

   ```bash
   kagami docker \
       --peers 4 \
       --config-dir ./cfg \
       --image hyperledger/iroha:dev \
       --out-file ./my-configs/docker-compose.npos.yml
   export IROHA_GENESIS_PUBLIC_KEY_FILE="$PWD/cfg/genesis.public_key"
   export IROHA_GENESIS_PRIVATE_KEY_FILE="$PWD/cfg/genesis.private_key"
   docker compose -f ./my-configs/docker-compose.npos.yml up
   ```

Use the same roster/PoPs when starting the containers to avoid mismatches between the signed genesis and node configs.

## Note on configuration structure

When using the `--build` option, the first peer in the generated configuration builds the image, while the rest of the peers depend on it. This is needed to avoid redundant building of the same image by every peer.
