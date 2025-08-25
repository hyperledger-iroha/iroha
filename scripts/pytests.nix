# This is for development convenience, to make setting up and running pytests straightforward.
# This is not a final product, so feel free to edit manually to adjust tests running or do whatever is convenient
#   for you right now.
#
# Build binaries: `cargo build --release --bins`
# Run from the repo root: `nix-shell ./scripts/pytests.nix`
# That's it
with import <nixpkgs> {};
  pkgs.mkShell {
    buildInputs = [
      poetry
      python313Packages.tomli-w

      cowsay
    ];

    BIN_IROHAD = "target/release/irohad";
    BIN_IROHA = "target/release/iroha";
    BIN_KAGAMI = "target/release/kagami";

    TMP_DIR = "../../test";
    IROHA_CLI_BINARY = "iroha";
    IROHA_CLI_CONFIG = "client.toml";

    shellHook = ''
      ./scripts/test_env.py setup
      echo "Set up test env"

      cd pytests/iroha_cli_tests
      poetry install --no-root
      poetry run pytest

      cd ../iroha_torii_tests
      poetry install --no-root
      poetry run pytest

      cd ../../

      ./scripts/test_env.py cleanup
      echo "Cleaned up"

      cowsay "Done!"
      exit
    '';
  }
