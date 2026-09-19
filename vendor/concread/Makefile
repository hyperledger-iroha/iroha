.DEFAULT: help
.PHONY: help
help:
	@grep -E -h '\s##\s' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

test: ## run tests across a variety of features
test:
	cargo test
	cargo test --no-default-features --features arcache-is-hashmap,asynch,ahash,ebr,maps
	cargo test --features hashtrie_skinny
	cargo test --features skinny
	cargo test --features tcache

clippy: ## run clippy across a variety of features
clippy:
	cargo clippy --no-default-features --features arcache-is-hashmap,asynch,ahash,ebr,maps
	cargo clippy
	cargo clippy --features tcache
	cargo clippy --features arcache

check: ## check all the things
check: test clippy codespell
	cargo fmt --check
	cargo outdated -R
	cargo audit


.PHONY: coverage/run
coverage/run: ## Run coverage tests
coverage/run:
	rm -rf "$(PWD)/target/profile/coverage-*.profraw"
	LLVM_PROFILE_FILE="$(PWD)/target/profile/coverage-%p-%m.profraw" RUSTFLAGS="-C instrument-coverage" cargo test
	LLVM_PROFILE_FILE="$(PWD)/target/profile/coverage-%p-%m.profraw" RUSTFLAGS="-C instrument-coverage" cargo test --no-default-features --features arcache-is-hashmap,asynch,ahash,ebr,maps
	LLVM_PROFILE_FILE="$(PWD)/target/profile/coverage-%p-%m.profraw" RUSTFLAGS="-C instrument-coverage" cargo test --features hashtrie_skinny
	LLVM_PROFILE_FILE="$(PWD)/target/profile/coverage-%p-%m.profraw" RUSTFLAGS="-C instrument-coverage" cargo test --features skinny
	LLVM_PROFILE_FILE="$(PWD)/target/profile/coverage-%p-%m.profraw" RUSTFLAGS="-C instrument-coverage" cargo test --features tcache

.PHONY: coverage/grcov
coverage/grcov: ## Run grcov
coverage/grcov:
	rm -rf ./target/coverage/html
	grcov . --binary-path ./target/debug/deps/ \
		-s . \
		-t html \
		--branch \
		--llvm \
		--ignore-not-existing \
		--ignore '../*' \
		--ignore "/*" \
		--ignore "target/*" \
		-o target/coverage/html

.PHONY: coverage
coverage: ## Run all the coverage tests
coverage: coverage/run coverage/grcov
	echo "Coverage report is in ./target/coverage/html/index.html"

.PHONY: codespell
codespell: ## spell-check things.
codespell:
	codespell -c \
	--ignore-words .codespell_ignore \
	--skip='./target,dhat-heap.json'
