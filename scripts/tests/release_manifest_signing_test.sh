#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tmp_dir_raw="$(mktemp -d "${TMPDIR:-/tmp}/iroha-release-manifest-signing.XXXXXX")"
tmp_dir="$(cd "$tmp_dir_raw" && pwd -P)"
trap 'rm -rf -- "$tmp_dir"' EXIT

manifest="$tmp_dir/release_manifest.json"
public_raw="$tmp_dir/public.raw"
signer="$tmp_dir/external-signer"
verifier="$tmp_dir/sorafs-validate"
signature="$tmp_dir/release_manifest.json.sig"
public_output="$tmp_dir/release_manifest.json.pub"

printf '%s\n' \
  '{' \
  '  "arch": "x86_64",' \
  '  "artifacts": [],' \
  '  "commit": "abcdef0",' \
  '  "version": "1.0.0"' \
  '}' \
  > "$manifest"

python3 - "$public_raw" "$signer" "$verifier" <<'PY'
import os
import sys
from pathlib import Path

public_path, signer_path, verifier_path = map(Path, sys.argv[1:])
manifest = bytes.fromhex(
    "7b0a20202261726368223a20227838365f3634222c0a2020226172746966616374"
    "73223a205b5d2c0a202022636f6d6d6974223a202261626364656630222c0a2020"
    "2276657273696f6e223a2022312e302e30220a7d0a"
)
public_key = bytes.fromhex(
    "2152f8d19b791d24453242e15f2eab6cb7cffa7b6a5ed30097960e069881db12"
)
signature = bytes.fromhex(
    "5a9e89b16ce487ecf4667ac0cf84ea794b4730d440f3c2ca64143267204e0ccb"
    "e818d9f87a9e0be8bab2d7ba31f19afa4553ba8427bb493e24c2c5edd90a020e"
)
public_path.write_bytes(public_key)
signer_path.write_text(
    "#!/usr/bin/env python3\n"
    "import sys\n"
    "from pathlib import Path\n"
    f"expected = bytes.fromhex({manifest.hex()!r})\n"
    f"signature = bytes.fromhex({signature.hex()!r})\n"
    "if Path(sys.argv[1]).read_bytes() != expected:\n"
    "    raise SystemExit(2)\n"
    "Path(sys.argv[2]).write_bytes(signature)\n",
    encoding="utf-8",
)
verifier_path.write_text(
    "#!/usr/bin/env python3\n"
    "import hashlib\n"
    "import sys\n"
    "from pathlib import Path\n"
    f"expected_manifest = bytes.fromhex({manifest.hex()!r})\n"
    f"expected_key = bytes.fromhex({public_key.hex()!r})\n"
    f"expected_signature = bytes.fromhex({signature.hex()!r})\n"
    "args = sys.argv[1:]\n"
    "if len(args) != 9 or args[0] != 'release-manifest':\n"
    "    raise SystemExit(4)\n"
    "options = dict(zip(args[1::2], args[2::2]))\n"
    "manifest = Path(options['--manifest']).read_bytes()\n"
    "public_key = Path(options['--public-key']).read_bytes()\n"
    "signature = Path(options['--signature']).read_bytes()\n"
    "if manifest != expected_manifest or public_key != expected_key:\n"
    "    raise SystemExit(2)\n"
    "if hashlib.sha256(public_key).hexdigest() != "
    "options['--public-key-fingerprint']:\n"
    "    raise SystemExit(2)\n"
    "if signature != expected_signature:\n"
    "    raise SystemExit(2)\n",
    encoding="utf-8",
)
os.chmod(public_path, 0o600)
os.chmod(signer_path, 0o700)
os.chmod(verifier_path, 0o700)
PY

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

fingerprint="$(sha256_file "$public_raw")"
verifier_digest="$(sha256_file "$verifier")"

python3 "$repo_root/scripts/release_manifest_signing.py" sign \
  --manifest "$manifest" \
  --external-signer "$signer" \
  --signing-public-key "$public_raw" \
  --trusted-signing-fingerprint "$fingerprint" \
  --signature-output "$signature" \
  --public-key-output "$public_output" \
  --release-manifest-verifier "$verifier" \
  --trusted-release-manifest-verifier-sha256 "$verifier_digest" \
  >/dev/null
cmp "$public_raw" "$public_output"
python3 "$repo_root/scripts/release_manifest_signing.py" verify \
  --manifest "$manifest" \
  --signature "$signature" \
  --public-key "$public_output" \
  --trusted-signing-fingerprint "$fingerprint" \
  --release-manifest-verifier "$verifier" \
  --trusted-release-manifest-verifier-sha256 "$verifier_digest" \
  >/dev/null

python3 "$repo_root/scripts/publish_plan.py" generate \
  --manifest "$manifest" \
  --manifest-signature "$signature" \
  --manifest-public-key "$public_output" \
  --trusted-signing-fingerprint "$fingerprint" \
  --release-manifest-verifier "$verifier" \
  --trusted-release-manifest-verifier-sha256 "$verifier_digest" \
  --artifacts-dir "$tmp_dir" \
  --target 'sorafs://release-test' \
  --output-dir "$tmp_dir/signed-plan"
if python3 "$repo_root/scripts/publish_plan.py" validate \
  --plan "$tmp_dir/signed-plan/publish_plan.json" \
  >/dev/null 2>&1; then
  echo "signed plan validated without independent trust inputs" >&2
  exit 1
fi
python3 "$repo_root/scripts/publish_plan.py" validate \
  --plan "$tmp_dir/signed-plan/publish_plan.json" \
  --trusted-signing-fingerprint "$fingerprint" \
  --release-manifest-verifier "$verifier" \
  --trusted-release-manifest-verifier-sha256 "$verifier_digest" \
  >/dev/null

if python3 "$repo_root/scripts/publish_plan.py" generate \
  --manifest "$manifest" \
  --artifacts-dir "$tmp_dir" \
  --target 'sorafs://release-test' \
  --output-dir "$tmp_dir/unsigned-production-plan" \
  >/dev/null 2>&1; then
  echo "unsigned production publish plan unexpectedly succeeded" >&2
  exit 1
fi

python3 "$repo_root/scripts/publish_plan.py" generate \
  --manifest "$manifest" \
  --artifacts-dir "$tmp_dir" \
  --target 'sorafs://release-test' \
  --development-allow-unsigned-manifest \
  --output-dir "$tmp_dir/development-plan"
if python3 "$repo_root/scripts/publish_plan.py" validate \
  --plan "$tmp_dir/development-plan/publish_plan.json" \
  >/dev/null 2>&1; then
  echo "development unsigned plan validated without the explicit escape hatch" >&2
  exit 1
fi
python3 "$repo_root/scripts/publish_plan.py" validate \
  --plan "$tmp_dir/development-plan/publish_plan.json" \
  --development-allow-unsigned-manifest \
  >/dev/null

if python3 "$repo_root/scripts/release_manifest_signing.py" verify \
  --manifest "$manifest" \
  --signature "$signature" \
  --public-key "$public_output" \
  --trusted-signing-fingerprint "$fingerprint" \
  --release-manifest-verifier "$verifier" \
  --trusted-release-manifest-verifier-sha256 \
    '0000000000000000000000000000000000000000000000000000000000000000' \
  >/dev/null 2>&1; then
  echo "wrong native verifier digest unexpectedly succeeded" >&2
  exit 1
fi

echo "release manifest native Ed25519 verification checks passed"
