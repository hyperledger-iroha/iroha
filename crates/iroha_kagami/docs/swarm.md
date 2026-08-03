# Kagami Docker Compose

Tools for generating Docker Compose configuration for Iroha.

## Usage

```bash
kagami docker [OPTIONS] --peers <COUNT> --config-dir <DIR> --image <NAME> --out-file <FILE>
```

### Options

- `-p, --peers <COUNT>`: Specifies an exact Sumeragi `3f + 1` validator
  committee in the supported range 4 through 31.

- `-s, --seed <SEED>`: Enables deterministic development mode.
  - This mode generates validator identities from the public seed and requires the three
    `IROHA_GENESIS_*_FILE` source paths when Compose is evaluated.
  - Omit this option for the normal prepared-bundle workflow. There is no implicit random
    identity generation.

- `-H, --healthcheck`: Includes a healthcheck for every service in the configuration. 
  - Healthchecks use predefined settings. 
  - For more details on healthcheck configuration in Docker Compose files, see: [Docker Compose Healthchecks](https://docs.docker.com/compose/compose-file/compose-file-v3/#healthcheck).

- `    --peer-config <FILE>`: Loads peer overrides from a TOML file.
  - This option is development-only and requires `--seed`.
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

- `-c, --config-dir <DIR>`: Selects the authoritative prepared bundle.
  - Normal mode requires `genesis.json`, exactly `peer0.toml` through `peerN.toml`,
    `genesis.signed.nrt`, `genesis.public_key`, and `genesis.expected_hash`.
  - Kagami verifies the canonical signed body and signer, checks that every instruction batch
    exactly realizes the bound `genesis.json` (including the staging-derived consensus
    commitment), then requires every peer config's chain, verifier key, exact hash, trusted
    roster, validator identity, and PoP map to agree exactly with the signed
    `RegisterPeerWithPop` roster.
  - With `--seed`, only `genesis.json` is used for policy validation.
  - Kagami derives a content-addressed, container-safe projection from each
    validated `peerN.toml` and mounts it as the `/config/peer.toml` Compose
    secret. Before publication it reparses the projection without ambient
    environment overrides and proves that the Sumeragi fingerprint,
    deterministic execution policy, and Nexus/AMX consensus context are
    unchanged.
  - Projection rewrites host-only storage and public auxiliary-file paths to
    fixed container locations. Torii account-onboarding and faucet sections are
    omitted because their local signer and client credentials are not part of
    the validator container trust boundary. The source peer configs, bundle
    directory, source manifest, genesis private signing key, and client
    credentials are never mounted.

The generated Compose manifest is validator-only and contains no genesis
signing key or signing service. In normal mode it reuses the exact validator
identities from the prepared bundle and embeds relative read-only paths for the
validated content-addressed config projection, verifier key, independently
approved exact hash, and signed body. Validator private keys remain inside the
file-backed config secret and are not serialized into Compose YAML or
environment variables. Each validator receives an anonymous `/storage` volume;
public rANS tables, lane manifests, and optional SoraFS site bindings are
bounded and materialized as Compose configs. The generator cannot silently
produce a roster different from the one signed into genesis:

```bash
kagami localnet --fresh-random-keys --peers 4 --out-dir ./localnet
kagami docker \
    --peers 4 \
    --config-dir ./localnet \
    --image hyperledger/iroha:dev \
    --out-file ./my-configs/docker-compose.yml
docker compose -f ./my-configs/docker-compose.yml up
```

`kagami localnet` emits and cross-checks the complete prepared bundle. An
equivalent manually prepared bundle may use
`kagami genesis sign --expected-hash-out`, but its `peerN.toml` identities and
PoPs must match the signed roster exactly. Generated launchers reject an empty
body, non-canonical one-line inputs, or a hash without Iroha's marker bit before
invoking `irohad`; `irohad` then repeats body, signature, verifier-key, and exact
hash validation.

The explicit `--seed` development mode is for relocatable samples such as the
checked-in Compose fixtures. It requires source paths at evaluation time:

```bash
export IROHA_GENESIS_PUBLIC_KEY_FILE="$PWD/dev-bundle/genesis.public_key"
export IROHA_GENESIS_SIGNED_FILE="$PWD/dev-bundle/genesis.signed.nrt"
export IROHA_GENESIS_EXPECTED_HASH_FILE="$PWD/dev-bundle/genesis.expected_hash"
docker compose -f ./my-configs/docker-compose.dev.yml up
```

Those files must have been prepared by the same Kagami revision for the exact
same seed, peer count, and consensus/profile policy. Port and service-name
overrides affect launcher endpoints but do not enter the index-based validator
key derivation. This mode is deterministic and intentionally not a production
custody path.

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

Generate a normal configuration from an authoritative prepared bundle and build
the image locally:

```bash
kagami docker \
    --peers 4 \
    --config-dir ./localnet \
    --image myiroha:local \
    --build . \
    --out-file ./my-configs/docker-compose.build.yml
```

Generate an explicit deterministic development manifest using an existing image.
The output is printed to stdout; the target path is still required for relative
path resolution:

```bash
kagami docker \
    --peers 4 \
    --seed Iroha \
    --healthcheck \
    --config-dir ./defaults \
    --image hyperledger/iroha:dev \
    --out-file ./my-configs/docker-compose.pull.yml \
    --print
```

## NPoS devnet workflow

1. Produce one random-custody NPoS bundle. The generator records `npos`, includes
   `sumeragi_npos_parameters`, signs the exact validator/PoP roster, and writes
   every peer config against the same exact hash.

   ```bash
   kagami localnet \
       --fresh-random-keys \
       --peers 4 \
       --consensus-mode npos \
       --out-dir ./cfg
   ```

2. Render Compose from that bundle. Do not pass `--seed`: prepared mode refuses
   missing, extra, or mismatched validator files and cryptographic artifacts.

   ```bash
   kagami docker \
       --peers 4 \
       --config-dir ./cfg \
       --image hyperledger/iroha:dev \
       --out-file ./my-configs/docker-compose.npos.yml
   docker compose -f ./my-configs/docker-compose.npos.yml up
   ```

Kagami derives the runtime identities from the validated configs, so there is no
separate “use the same roster” operator step to get wrong. The source localnet
configs still retain account-onboarding and faucet services for bare-metal
operation; generated validator-only Compose deliberately disables those
host-credential-backed services.

## Note on configuration structure

When using the `--build` option, the first validator declares the image build
and the remaining validators reuse that image. Compose completes project image
builds before starting the validator-only service set, avoiding redundant
builds without introducing a runtime bootstrap service.
