{
  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    alejandra = {
      url = "github:kamadorueda/alejandra/3.0.0";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    nix-appimage.url = "path:nix-appimage";
  };

  outputs = {
    self,
    fenix,
    flake-utils,
    nixpkgs,
    alejandra,
    nix-appimage,
    ...
  } @ inputs:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = (import nixpkgs) {inherit system;};
      lib = pkgs.lib;

      fenix' = fenix.packages.${system};

      supportedTargets = [
        "x86_64-unknown-linux-gnu"
        "x86_64-unknown-linux-musl"
        "x86_64-apple-darwin"
        "aarch64-unknown-linux-gnu"
        "aarch64-unknown-linux-musl"
        "aarch64-apple-darwin"
      ];

      allBinaries = [
        "irohad"
        "iroha"
        "kagami"
      ];

      # HACK: A hook to filter out darwin-specific flags when cross-compiling.
      # Those flags being there in the first place is either a bug in nixpkgs or me
      # completely misunderstanding how cross-compilation works
      setupHookDarwinCross =
        pkgs.makeSetupHook {name = "darwin-iroha-hook";}
        (pkgs.writeScript "darwin-iroha-hook.sh" ''
          fixupCFlagsForDarwin() {
              cflagsFilter='s|-F[^ ]*||g;s|-iframework [^ ]*||g;s|-isystem [^ ]*||g;s|  *| |g'
              ldFlagsFilter='s|/nix/store/[^-]*-apple-framework-CoreFoundation[^ ]*||g'

              echo "Filtering CFLAGS"
              export NIX_CFLAGS_COMPILE="$(sed "$cflagsFilter" <<< "$NIX_CFLAGS_COMPILE")"

              echo "Filtering LDFLAGS"
              export NIX_LDFLAGS="$(sed "$ldFlagsFilter;$cflagsFilter" <<< "$NIX_LDFLAGS")"
          }
          preBuildHooks+=(fixupCFlagsForDarwin)
        '');

      # Build an Iroha derivation
      mkIroha = {
        target ? system, # target arch to build for
        binaries ? allBinaries, # which binary to build
        name ? "iroha", # resulting derivation name
        features ? [], # feature list forwarded to cargo
        ...
      } @ args: let
        systemTriple = (lib.systems.elaborate system).config;
        targetTriple = (lib.systems.elaborate target).config;
        isCross = systemTriple != targetTriple;
        toolchainHost = fenix'.stable;
        toolchainTarget =
          fenix'.targets.${targetTriple}.stable;
        toolchain = fenix'.combine [
          toolchainHost.rustc
          toolchainHost.cargo
          toolchainHost.rustfmt
          toolchainTarget.rust-std
        ];
        pkgsCross = (import nixpkgs) {
          localSystem = system;
          crossSystem = target;
          config.allowUnsupportedSystem = true;
        };
        naersk = pkgsCross.buildPackages.callPackage inputs.naersk {
          cargo = toolchain;
          rustc = toolchain;
        };
      in
        naersk.buildPackage rec {
          pname = name;
          version =
            (builtins.fromTOML
              (builtins.readFile ./Cargo.toml))
            .workspace
            .package
            .version;

          src = ./.;

          # FIXME: Fails due to https://github.com/rust-lang/cargo/issues/10368
          # Either try a workaround, or wait for resolution
          # doDoc = true;
          # doDocFail = true;

          nativeBuildInputs = with pkgsCross.buildPackages;
            [
              pkg-config
              libiconvReal
              stdenv.cc
              binutils
            ]
            # If cross-compiling FROM darwin, need to fixup build flags
            ++ lib.optional (pkgs.stdenv.isDarwin && isCross)
            setupHookDarwinCross;

          buildInputs = with pkgsCross;
            [
              openssl.dev
              libiconvReal
              zlib
            ]
            # If building FOR darwin, need Apple frameworks
            ++ lib.optional pkgsCross.stdenv.isDarwin
            [darwin.apple_sdk.frameworks.Security];

          cargoBuildOptions = default:
            default
            ++ ["--target" targetTriple]
            ++ builtins.concatMap (binary: ["-p" binary]) binaries
            ++ (if features == [] then [] else ["--features" (builtins.concatStringsSep "," features)]);

          CARGO_BUILD_TARGET = targetTriple;

          CC =
            if isCross
            then "${pkgsCross.stdenv.cc}/bin/${target}-cc"
            else "${pkgsCross.stdenv.cc}/bin/cc";
          TARGET_CC = CC;
          RUSTFLAGS = "-C linker=${CC}";

          VERGEN_IDEMPOTENT = true;
          VERGEN_GIT_SHA = self.rev or "?dirty tree?";

        };

      mkTargets = { features, suffix }:
        builtins.listToAttrs (map (target: {
          name = target;
          value = mkIroha {
            inherit target;
            binaries = ["irohad"];
            name = "iroha${suffix}-${target}";
            features = features;
          };
        }) supportedTargets);
      in rec {
        inherit mkIroha;

      packages.iroha2 = mkIroha {
        name = "iroha2";
      };

      packages.iroha3 = mkIroha {
        name = "iroha3";
        features = [];
      };

      packages.default = packages.iroha2;

      packages.appimage_iroha2 = nix-appimage.mkappimage.${system} {
        drv = packages.iroha2;
        name = "iroha2";
      };

      packages.appimage_iroha3 = nix-appimage.mkappimage.${system} {
        drv = packages.iroha3;
        name = "iroha3";
      };

      packages.appimage = packages.appimage_iroha2;

      packages.targets = mkTargets {
        features = [];
        suffix = "2";
      };

      packages.targets_iroha3 = mkTargets {
        features = [];
        suffix = "3";
      };

      apps =
        {
          default = {
            type = "app";
            program = "${self.packages.${system}.default}/bin/irohad";
          };
          iroha3 = {
            type = "app";
            program = "${self.packages.${system}.iroha3}/bin/irohad";
          };
        }
        // builtins.listToAttrs (map (bin: {
            name = bin;
            value = {
              type = "app";
              program = "${self.packages.${system}.default}/bin/${bin}";
            };
          })
          allBinaries);

      formatter = alejandra.packages.${system}.default;

      devShells.default = let
        toolchainPkgs = fenix'.stable;
        toolchain = fenix'.combine [
          toolchainPkgs.rustc
          toolchainPkgs.cargo
          toolchainPkgs.clippy
          toolchainPkgs.rustfmt
          toolchainPkgs.rust-std
        ];
      in
        pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            pkg-config
            openssl.dev
            libiconvReal
            zlib
            toolchain
            fenix'.rust-analyzer
          ];

        };
    });
}
      checks = {
        # Shielded Merkle golden vectors check: runs the golden test for iroha_crypto
        shielded-merkle = pkgs.stdenv.mkDerivation {
          pname = "shielded-merkle-vectors-check";
          version = "1";
          src = ./.;
          nativeBuildInputs = with pkgs; [
            pkg-config
            openssl.dev
            libiconvReal
            zlib
          ] ++ [
            (fenix'.combine [
              fenix'.stable.rustc
              fenix'.stable.cargo
              fenix'.stable.rustfmt
            ])
          ];
          buildPhase = ''
            cargo test -p iroha_crypto --test merkle_shielded_golden -- -q
          '';
          installPhase = ''
            mkdir -p $out
            echo ok > $out/result
          '';
        };
      };
