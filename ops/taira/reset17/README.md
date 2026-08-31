# Taira reset17 authenticated release corridor

Reset17 is the fail-stop macOS rollout path for the four public Taira validators.
It accepts only a reviewed, SSH-signed public bundle, checks the five private-file
identities for each validator without reading their contents, proves the exact
four-peer topology and storage capacity, and applies one hash-addressed plan.

The Digital Kina identity is fixed throughout this corridor:

- public alias: `kina#bpng`
- raw Iroha asset definition: `839FV3NJC8NfgWQvghXU2hEFQm9a`
- owning domain: `bpng.bpng`
- scale: `2`
- logical lane: `3` / `dpn`
- physical dataspace: `10` / `bpng`

Kagemusha remains a closed Offline Cash V1 protocol with data ABI 4. The native
mobile bridge is ABI22 and the verifier catalog is the ordered Exact12 set in
`testnet_reset17_authenticated_reset.py`.

## Prepare and sign

Create a canonical preparation spec using schema
`inori.taira.reset17-preparation-spec.v1`. It has these exact top-level keys:

```text
schema, release_id, network_id, source, protocols, bpng,
deployment, artifacts, validators
```

`artifacts` maps each exact required artifact name to an absolute public source
path. Each of the four ordered validator entries has exactly:

```text
index, label, data_root, torii_url, p2p_port,
config_source, private_files, runtime_signer
```

Render the sealed unsigned bundle:

```sh
python3 -I -B ops/taira/reset17/prepare_reset17_candidate.py \
  --spec /absolute/review/reset17-preparation.json \
  --out-bundle /absolute/review/reset17-public-bundle
```

The command prints the manifest path and SHA-256. Copy the exact manifest bytes
to an owner-private signing directory and sign them separately; neither the
renderer nor controller accepts a signing key:

```sh
ssh-keygen -Y sign -f /runtime-only/operator-key \
  -n taira-reset17 /absolute/signing/manifest.json
```

The allowed-signers entry must authorize identity `taira-reset17-release`.
Record the manifest SHA-256, allowed-signers SHA-256, and clean SSH-signed source
commit out of band. The source identity must verify to the reviewed Taira release
key fingerprint `SHA256:ykCGGqELtdtBpdJ/DTT6ROwpqCCGKYACMhUfdzTxi+g`;
the retired OpenPGP fingerprint is rejected. Never place runtime secrets in the
bundle, manifest, plan, result, command line, or environment.

## Plan, review, and check

All paths below are examples; every path and digest flag is mandatory. The
control root and LaunchAgents directory are independent operator pins, not
trusted merely because they appear in the signed manifest.

```sh
python3 -I -B ops/taira/reset17/testnet_reset17_authenticated_reset.py plan \
  --bundle /absolute/review/reset17-public-bundle \
  --manifest /absolute/review/reset17-public-bundle/manifest.json \
  --signature /absolute/signing/manifest.json.sig \
  --allowed-signers /absolute/review/reset17.allowed_signers \
  --expected-manifest-sha256 MANIFEST_SHA256 \
  --expected-allowed-signers-sha256 ALLOWED_SIGNERS_SHA256 \
  --expected-source-commit CLEAN_SIGNED_COMMIT \
  --expected-control-root /Users/administrator/.taira-reset17 \
  --expected-launch-agents-dir /Users/administrator/Library/LaunchAgents \
  --out-plan /absolute/review/reset17.plan.json
```

The printed value is the exact plan SHA-256. Review the canonical plan, then
repeat every authentication and storage check without changing services:

```sh
python3 -I -B ops/taira/reset17/testnet_reset17_authenticated_reset.py check-plan \
  [same candidate and operator-pin arguments] \
  --plan /absolute/review/reset17.plan.json \
  --expected-plan-sha256 PLAN_SHA256
```

The physical storage gate uses `f_bavail`, groups copy bytes by device, requires
all four data roots on one physical volume, and reserves:

```text
4 × 68,719,476,736 bytes
+ max(32 GiB, ceil(device capacity × 10%))
+ exact release, live LaunchAgent, and predecessor-backup copy bytes
```

There is no low-space, force, skip, or no-health override.

## Apply

Apply requires the byte-exact confirmation below:

```sh
python3 -I -B ops/taira/reset17/testnet_reset17_authenticated_reset.py apply \
  [same candidate and operator-pin arguments] \
  --plan /absolute/review/reset17.plan.json \
  --expected-plan-sha256 PLAN_SHA256 \
  --confirm FAIL-STOP-TAIRA-RESET17:PLAN_SHA256 \
  --result /Users/administrator/.taira-reset17/results/RELEASE_ID.json
```

Under a nonblocking exclusive lock, apply repeats authentication, stages an
immutable release, creates generation-specific owner-private data roots,
preserves predecessor LaunchAgents, transitions all four services, and requires
`/health`, `/readyz`, converged `/status`, empty queues, and a strictly advancing
minimum height. A post-mutation failure attempts exact predecessor rollback and
writes a bounded failure receipt. New release and reset17 data remain available
for forensics; predecessor data is never deleted.
