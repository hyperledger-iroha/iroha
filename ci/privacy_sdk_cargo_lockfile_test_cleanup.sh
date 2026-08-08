#!/usr/bin/env bash
# Cleanup support sourced after the lockfile self-test creates its private root.

cleanup() {
  local status=$?
  trap - EXIT HUP INT TERM
  if [[ -n "${TEST_ROOT:-}" && -d "${TEST_ROOT}" ]]; then
    rm -rf -- "${TEST_ROOT}"
  fi
  exit "${status}"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
