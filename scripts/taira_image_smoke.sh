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
timeout_seconds=120
smoke_id="${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-0}-${GITHUB_JOB:-job}-$$"
smoke_id="${smoke_id//[^a-zA-Z0-9_.-]/-}"
work_dir=".generated/taira-image-smoke-${smoke_id}"
container_prefix="taira-validator-smoke-${smoke_id}-peer"
network_name="taira-validator-smoke-${smoke_id}-net"

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

invocation_root="$(pwd -P)"
script_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
renderer_relative="scripts/render_taira_localnet_container_bundle.py"
sealed_source=0
renderer_path=""
if [[ -f "$script_root/$renderer_relative" && ! -L "$script_root/$renderer_relative" ]]; then
    execution_root="$script_root"
    renderer_path="$script_root/$renderer_relative"
elif [[ -f "$script_root/taira-workspace-source-v1.seal" ]] \
    && [[ -f "$script_root/taira-workspace-source-paths-v1.bin" ]] \
    && [[ -f "$script_root/workspace-source-manifest.sha256" ]] \
    && [[ -f "$script_root/source-archive.sha256" ]] \
    && [[ -f "$script_root/source-path-list.sha256" ]] \
    && [[ -f "$script_root/context-control.sha256" ]] \
    && [[ -x "$script_root/scripts/compute_workspace_source_manifest.py" ]]; then
    execution_root="$invocation_root"
    sealed_source=1
elif [[ -f "$invocation_root/$renderer_relative" && ! -L "$invocation_root/$renderer_relative" ]]; then
    execution_root="$invocation_root"
    renderer_path="$invocation_root/$renderer_relative"
else
    printf 'cannot find direct or sealed Taira localnet renderer from %s or %s\n' \
        "$script_root" "$invocation_root" >&2
    exit 1
fi
cd "$execution_root"

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

if [[ -L "$work_dir" ]]; then
    printf 'refusing symlinked Taira image smoke work directory: %s\n' \
        "$work_dir" >&2
    exit 1
fi
abs_work_dir="$(mkdir -p "$work_dir" && cd "$work_dir" && pwd -P)"
if [[ "$abs_work_dir" == "/" || "$abs_work_dir" == "$execution_root" ]]; then
    printf 'refusing broad Taira image smoke work directory: %s\n' \
        "$abs_work_dir" >&2
    exit 1
fi
work_marker="$abs_work_dir/.taira-image-smoke-workdir"
if [[ -e "$work_marker" || -L "$work_marker" ]]; then
    if [[ ! -f "$work_marker" || -L "$work_marker" ]] \
        || [[ "$(tr -d '\n' <"$work_marker")" != "iroha-taira-image-smoke-v1" ]]; then
        printf 'invalid Taira image smoke work-directory marker: %s\n' \
            "$work_marker" >&2
        exit 1
    fi
elif [[ -n "$(find "$abs_work_dir" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
    printf 'refusing non-empty unowned Taira image smoke work directory: %s\n' \
        "$abs_work_dir" >&2
    exit 1
else
    printf '%s\n' "iroha-taira-image-smoke-v1" >"$work_marker"
fi
find "$abs_work_dir" \
    -xdev \
    -mindepth 1 \
    -depth \
    ! -name ".taira-image-smoke-workdir" \
    -delete
chmod 0777 "$abs_work_dir"

if [[ "$sealed_source" == "1" ]]; then
    sealed_source_root="$abs_work_dir/sealed-source"
    mkdir "$sealed_source_root"
    for required_name in \
        IROHA_WORKSPACE_SOURCE_MANIFEST_SHA256 \
        TAIRA_SEALED_SOURCE_ARCHIVE_SHA256 \
        TAIRA_SEALED_SOURCE_PATH_LIST_SHA256 \
        TAIRA_SEALED_CONTEXT_CONTROL_SHA256; do
        required_value="${!required_name:-}"
        if [[ ! "$required_value" =~ ^[0-9a-f]{64}$ ]]; then
            printf 'sealed Taira image smoke requires %s as a lowercase SHA-256 digest\n' \
                "$required_name" >&2
            exit 1
        fi
    done
    python3 -I -S "$script_root/scripts/compute_workspace_source_manifest.py" \
        --validate-sealed-context "$script_root"
    actual_context_control_sha="$(
        sha256sum "$script_root/context-control.sha256" | awk '{print $1}'
    )"
    if [[ "$actual_context_control_sha" != "$TAIRA_SEALED_CONTEXT_CONTROL_SHA256" ]]; then
        printf '%s\n' 'sealed Taira image smoke context-control digest mismatch' >&2
        exit 1
    fi
    (
        cd "$script_root"
        sha256sum -c context-control.sha256
    ) >/dev/null
    expected_manifest="$(tr -d '\n' <"$script_root/workspace-source-manifest.sha256")"
    expected_archive_sha="$(awk '{print $1}' "$script_root/source-archive.sha256")"
    expected_path_list_sha="$(awk '{print $1}' "$script_root/source-path-list.sha256")"
    if [[ "$expected_manifest" != "$IROHA_WORKSPACE_SOURCE_MANIFEST_SHA256" ]] \
        || [[ "$expected_archive_sha" != "$TAIRA_SEALED_SOURCE_ARCHIVE_SHA256" ]] \
        || [[ "$expected_path_list_sha" != "$TAIRA_SEALED_SOURCE_PATH_LIST_SHA256" ]]; then
        printf '%s\n' 'sealed Taira image smoke source-control digest mismatch' >&2
        exit 1
    fi
    python3 -I -S "$script_root/scripts/compute_workspace_source_manifest.py" \
        --root "$sealed_source_root" \
        --path-list "$script_root/taira-workspace-source-paths-v1.bin" \
        --extract-sealed-archive "$script_root/taira-workspace-source-v1.seal" \
        --destination "$sealed_source_root" \
        --expected-manifest "$expected_manifest" \
        --expected-archive-sha256 "$expected_archive_sha" \
        --expected-path-list-sha256 "$expected_path_list_sha" \
        >/dev/null
    renderer_path="$sealed_source_root/$renderer_relative"
fi
if [[ ! -f "$renderer_path" || -L "$renderer_path" ]]; then
    printf 'trusted Taira localnet renderer is missing after source validation: %s\n' \
        "$renderer_path" >&2
    exit 1
fi

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

python3 "$renderer_path" \
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
        -e TAIRA_RUNTIME_PROFILE=localnet \
        -e IROHA_TAIRA_CONFIG=/config/config.toml \
        -e IROHA_TAIRA_GENESIS=/config/genesis.json \
        -e IROHA_TAIRA_SIGNED_GENESIS=/config/genesis.signed.nrt \
        -v "${host_rendered_dir}/peer${peer}/config.toml:/config/config.toml:ro" \
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
