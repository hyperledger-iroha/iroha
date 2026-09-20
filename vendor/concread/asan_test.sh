#!/bin/sh

#shellcheck disable=SC2068
RUSTC_BOOTSTRAP=1  RUSTFLAGS="-Z sanitizer=address" cargo test $@

