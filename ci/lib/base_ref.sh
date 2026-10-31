# Comparison base shared by the contributor guards. Source this file, then call resolve_base.
#
# AGENTS_BASE_REF wins when it names a real ref. Otherwise the branch named in
# ci/default_base_ref is used, because that is where contributors actually integrate. Forks and
# clones without it fall back to the usual default branches, then to HEAD^.

BASE_REF_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"

resolve_base() {
	if [[ -n "${AGENTS_BASE_REF:-}" ]]; then
		if git rev-parse --verify --quiet "${AGENTS_BASE_REF}" >/dev/null; then
			echo "${AGENTS_BASE_REF}"
			return
		fi
		echo "warning: AGENTS_BASE_REF=${AGENTS_BASE_REF} not found; falling back to defaults." >&2
	fi

	local configured=""
	if [[ -r "${BASE_REF_ROOT}/ci/default_base_ref" ]]; then
		configured="$(sed -e 's/#.*$//' -e '/^[[:space:]]*$/d' "${BASE_REF_ROOT}/ci/default_base_ref" | sed -n '1p' | tr -d '[:space:]')"
	fi
	local candidate
	for candidate in "${configured}" origin/main origin/master main master; do
		if [[ -n "${candidate}" ]] && git rev-parse --verify --quiet "${candidate}" >/dev/null; then
			echo "${candidate}"
			return
		fi
	done

	echo "HEAD^"
}
