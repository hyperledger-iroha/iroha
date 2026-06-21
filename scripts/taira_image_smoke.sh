#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/taira_image_smoke.sh [--image IMAGE] [--work-dir DIR] [--timeout SECONDS]

Build-time smoke for the Taira validator image. The image must contain both
`irohad` and `kagami`.
EOF
}

image="local/taira-validator:smoke"
work_dir=".generated/taira-image-smoke"
timeout_seconds=120
container_prefix="taira-validator-smoke-peer"
network_name="taira-validator-smoke-net"

while (($# > 0)); do
    case "$1" in
        --image)
            image="${2:?missing value for --image}"
            shift 2
            ;;
        --work-dir)
            work_dir="${2:?missing value for --work-dir}"
            shift 2
            ;;
        --timeout)
            timeout_seconds="${2:?missing value for --timeout}"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

resolve_host_path() {
    local path="$1"
    local container_id="${HOSTNAME:-}"
    local best_source=""
    local best_destination=""
    local source destination

    if [[ -z "$container_id" ]]; then
        container_id="$(hostname)"
    fi

    while IFS=$'\t' read -r source destination; do
        if [[ -z "$source" || -z "$destination" ]]; then
            continue
        fi
        if [[ "$path" == "$destination" || "$path" == "$destination/"* ]]; then
            if ((${#destination} > ${#best_destination})); then
                best_source="$source"
                best_destination="$destination"
            fi
        fi
    done < <(
        docker inspect "$container_id" \
            --format '{{range .Mounts}}{{printf "%s\t%s\n" .Source .Destination}}{{end}}' \
            2>/dev/null || true
    )

    if [[ -n "$best_source" ]]; then
        printf '%s%s\n' "$best_source" "${path#"$best_destination"}"
    else
        printf '%s\n' "$path"
    fi
}

abs_work_dir="$(mkdir -p "$work_dir" && cd "$work_dir" && pwd -P)"
rm -rf "$abs_work_dir"
mkdir -p "$abs_work_dir"
chmod 0777 "$abs_work_dir"

host_work_dir="$(resolve_host_path "$abs_work_dir")"
bundle_dir="$abs_work_dir/bundle"
rendered_dir="$abs_work_dir/rendered"
host_bundle_dir="$host_work_dir/bundle"
host_rendered_dir="$host_work_dir/rendered"

echo "Generating Taira localnet smoke bundle with ${image}"
docker run --rm \
    --user 0:0 \
    -v "${host_work_dir}:/smoke" \
    --entrypoint kagami \
    "$image" \
    localnet \
    --peers 4 \
    --seed taira-image-smoke \
    --sora-profile nexus \
    --consensus-mode npos \
    --out-dir /smoke/bundle

python3 scripts/render_taira_localnet_container_bundle.py \
    --bundle-dir "$bundle_dir" \
    --output-dir "$rendered_dir" \
    --image "$image" \
    --network "$network_name" \
    --container-prefix "$container_prefix" \
    >/dev/null

chmod -R a+rwx "$abs_work_dir"

containers=()
for peer in 0 1 2 3; do
    containers+=("${container_prefix}${peer}")
done

cleanup() {
    local status=$?
    if [[ $status -ne 0 ]]; then
        docker ps -a --filter "name=${container_prefix}" || true
        for container in "${containers[@]}"; do
            echo "--- ${container} logs ---" >&2
            docker logs "$container" >&2 || true
        done
    fi
    for container in "${containers[@]}"; do
        docker rm -f "$container" >/dev/null 2>&1 || true
    done
    docker network rm "$network_name" >/dev/null 2>&1 || true
    return "$status"
}
trap cleanup EXIT

for container in "${containers[@]}"; do
    docker rm -f "$container" >/dev/null 2>&1 || true
done
docker network rm "$network_name" >/dev/null 2>&1 || true
docker network create "$network_name" >/dev/null

for peer in 0 1 2 3; do
    container="${container_prefix}${peer}"
    docker run -d \
        --name "$container" \
        --network "$network_name" \
        -e IROHA_TAIRA_GENESIS=/config/genesis.json \
        -e IROHA_TAIRA_SIGNED_GENESIS=/config/genesis.signed.nrt \
        -v "${host_rendered_dir}/peer${peer}.toml:/config/config.toml:ro" \
        -v "${host_bundle_dir}/genesis.json:/config/genesis.json:ro" \
        -v "${host_bundle_dir}/genesis.signed.nrt:/config/genesis.signed.nrt:ro" \
        -v "${host_rendered_dir}/peer${peer}-storage:/storage" \
        "$image" \
        >/dev/null
done

deadline=$((SECONDS + timeout_seconds))
while ((SECONDS < deadline)); do
    for container in "${containers[@]}"; do
        if [[ "$(docker inspect -f '{{.State.Running}}' "$container" 2>/dev/null || true)" != "true" ]]; then
            echo "${container} exited before smoke endpoint became ready" >&2
            exit 1
        fi
    done

    if docker exec "${containers[0]}" curl -fsS http://127.0.0.1:8080/v1/mcp >/dev/null; then
        docker exec "${containers[0]}" curl -fsS -H 'Accept: application/json' http://127.0.0.1:8080/status >/dev/null
        echo "Taira image smoke reached /v1/mcp on ${containers[0]}"
        exit 0
    fi
    sleep 2
done

echo "timed out waiting for /v1/mcp on Taira image smoke cluster" >&2
exit 1
