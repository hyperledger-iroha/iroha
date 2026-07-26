doc:
	cargo doc --no-deps

.PHONY: doc

# Since the master branch is protected, the current workflow is to create a PR with the version changes,
# and once the PR is merged, run the `make VERSION=<version> release` to publish the new crates.
release:
ifndef VERSION
	$(error VERSION is not set. Run with `make VERSION=<version> release`)
endif
ifeq (, $(shell cargo --list|grep release))
	$(error "Please, install cargo-release in order to be able to use this rule")
endif
	git pull
	cargo update
	git tag v$(VERSION) 
	@TRACKING_BRANCH=$$(git rev-parse --abbrev-ref --symbolic-full-name @{u} 2> /dev/null) ;\
	if [ "$$TRACKING_BRANCH" == "" ]; then \
		echo "Error: Current branch does not have an upstream tracking branch." ;\
		exit 1 ;\
	fi ;\
	git push --tags $$(echo $$TRACKING_BRANCH | sed 's!/.*!!') v$(VERSION)
	cargo release publish --execute  --verbose