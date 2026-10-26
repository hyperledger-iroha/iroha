{
  description = "AppImage bundler";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    flake-utils.url = "github:numtide/flake-utils";

    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };

    appimage-runtime = {
      url = "github:AppImage/type2-runtime/fedfad51acc2860b2916a1eef32ba4a500ff31ef";
      flake = false;
    };

    squashfuse = {
      url = "github:vasi/squashfuse/0.6.5";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, flake-utils, ... }@inputs:
    flake-utils.lib.eachDefaultSystem (system:
      let
        nativePkgs = import nixpkgs {
          inherit system;
        };
        pkgs = nativePkgs.pkgsStatic;

        # Same current libfuse source/hash as nixpkgs-unstable, using the
        # supported package set's static build and Linux mount policy.
        runtime-fuse = pkgs.fuse3.overrideAttrs (old: {
          version = "3.18.2";
          src = pkgs.fetchFromGitHub {
            owner = "libfuse";
            repo = "libfuse";
            rev = "fuse-3.18.2";
            hash = "sha256-QArQMSStVxwUo6CgU2JlXBdFWjzlGXfZk1AVGLGeE70=";
          };
          # Rebase the package set's install-path patch: libfuse 3.18 adds
          # lib/util.c to the adjacent Meson target. Keep all three path fixes.
          patches = builtins.filter
            (patch: !(pkgs.lib.hasSuffix "fuse3-install.patch" (toString patch)))
            (old.patches or []);
          postPatch = (old.postPatch or "") + ''
            substituteInPlace util/install_helper.sh \
              --replace-fail '"''${DESTDIR}''${udevrulesdir}/99-fuse3.rules"' '"''${sysconfdir}''${udevrulesdir}/99-fuse3.rules"' \
              --replace-fail '"''${DESTDIR}''${initscriptdir}/fuse3"' '"''${sysconfdir}''${initscriptdir}/fuse3"'
            substituteInPlace util/meson.build \
              --replace-fail "join_paths(get_option('prefix'), get_option('sysconfdir'), 'fuse.conf')" "join_paths('/', get_option('sysconfdir'), 'fuse.conf')"
          '';
        });

        # Keep the nixpkgs static dependency closure, with the current upstream
        # Squashfuse source and the private headers required by type2-runtime.
        own-squashfuse = (pkgs.squashfuse.override { fuse3 = runtime-fuse; }).overrideAttrs (old: {
          version = "0.6.5";
          src = inputs.squashfuse;
          postInstall = (old.postInstall or "") + ''
            install -m 644 ./*.h -t $out/include/squashfuse/
          '';
        });
      in
      rec {
        packages.apprun = pkgs.runCommandCC "AppRun" { } ''
          $CC ${./apprun.c} -Werror -o $out
        '';

        # Build the maintained upstream Type 2 runtime from the locked source.
        # Static linking keeps host glibc and libfuse out of the runtime contract.
        packages.runtime = pkgs.runCommandWith {
          name = "appimage-type2-runtime";
          # Upstream uses Clang: its musl static-PIE driver selects the
          # self-relocating startup object required before libc initialization.
          stdenv = pkgs.llvmPackages.stdenv;
          derivationArgs = {
            nativeBuildInputs = with pkgs; [ pkg-config ];
            buildInputs = with pkgs; [
              runtime-fuse own-squashfuse zstd zlib xz lz4 lzo mimalloc
            ];
          };
        } ''
            $CC ${inputs.appimage-runtime}/src/runtime/runtime.c -o $out \
              $($PKG_CONFIG --cflags fuse3) \
              -I${own-squashfuse}/include/squashfuse \
              -D_FILE_OFFSET_BITS=64 \
              -DGIT_COMMIT='"${inputs.appimage-runtime.rev}"' \
              -std=gnu99 -Os -ffunction-sections -fdata-sections \
              -Wl,--gc-sections -static -static-pie --rtlib=compiler-rt -Wall -Werror \
              -lsquashfuse_ll -lsquashfuse -lfuse3 \
              -lzstd -lz -llzma -llz4 -llzo2 -lmimalloc -lpthread -lm \
              -T ${inputs.appimage-runtime}/src/runtime/data_sections.ld
            printf %b '\x41\x49\x02' > magic_bytes
            dd if=magic_bytes of=$out bs=1 count=3 seek=8 conv=notrunc status=none
          '';

        mainProgram = drv:
              if drv?meta && drv.meta?mainProgram then drv.meta.mainProgram
              else (builtins.parseDrvName (builtins.unsafeDiscardStringContext drv.name)).name;

            program = drv:
              let
                # Use same auto-detect that <https://github.com/NixOS/bundlers>
                # uses. This isn't 100% accurate and might pick the wrong name
                # (e.g. nixpkgs#mesa-demos), so we do an additional check to
                # make sure the target exists
                main =
                  if drv?meta && drv.meta?mainProgram then drv.meta.mainProgram
                  else (builtins.parseDrvName (builtins.unsafeDiscardStringContext drv.name)).name;
                mainPath = "${drv}/bin/${main}";

                # builtins.pathExists mainPath doesn't work consistently (e.g.
                # for symlinks), but this does
                mainPathExists = builtins.hasAttr main (builtins.readDir "${drv}/bin");
              in
              assert pkgs.lib.assertMsg mainPathExists "main program ${mainPath} does not exist";
              mainPath;
        # Produce an (probably non-conforming) AppImage.
        #
        # The AppImage type 2 format is simply the runtime binary concatenated
        # with a squashfs. When running the AppImage, the squashfs binary is
        # extracted/mounted at an arbitrary place and the AppRun binary within
        # run.
        mkappimage = { drv, name, executable ? name }:
          let
            arch = builtins.head (builtins.split "-" system);
            closure = "${pkgs.closureInfo { rootPaths = [ drv ]; }}/store-paths";
            entrypoint = "${drv}/bin/${executable}";
            extras = [
              "AppRun f 555 0 0 cat ${packages.apprun}"
              "entrypoint s 555 0 0 ${entrypoint}"
              # Root mount directory must not be world-writable; 0755 matches AppImage tooling defaults.
              "mountroot d 755 0 0"
            ];
            extra-args = pkgs.lib.concatMapStrings (x: " -p \"${x}\"") extras;
          in
          pkgs.runCommand "${name}-${arch}.AppImage"
            {
              nativeBuildInputs = with pkgs; [ squashfsTools ];
            } ''
            mksquashfs $(cat ${closure}) $out \
              -no-strip \
              -offset $(stat -L -c%s ${packages.runtime}) \
              -comp zstd \
              -all-root \
              -noappend \
              -b 1M \
              ${extra-args}
            dd if=${packages.runtime} of=$out conv=notrunc
            chmod 755 $out
          '';
        packages.smoke = mkappimage {
          drv = (import nixpkgs { inherit system; }).hello;
          name = "hello";
        };

        checks.type2-structure = pkgs.runCommand "appimage-type2-structure"
          { nativeBuildInputs = with nativePkgs; [ binutils-unwrapped coreutils squashfsTools ]; } ''
            test "$(od -An -tx1 -j8 -N3 ${packages.runtime} | tr -d ' \n')" = 414902
            readelf -l ${packages.runtime} > program-headers
            readelf -d ${packages.runtime} > dynamic-tags
            ! grep -q INTERP program-headers
            ! grep -q NEEDED dynamic-tags
            ${packages.runtime} --appimage-version
            unsquashfs -o $(stat -c%s ${packages.runtime}) -lln ${packages.smoke} mountroot > squashfs-list
            grep -Eq '^drwxr-xr-x[[:space:]].* squashfs-root/mountroot$' squashfs-list
            ${packages.smoke} --appimage-extract
            test -x squashfs-root/AppRun
            # Upstream deliberately creates private extraction directories.
            # The mounted image retains the exact 0755 mode checked above.
            test "$(stat -c %a squashfs-root/mountroot)" = 700
            test "$(readlink squashfs-root/entrypoint)" = "${(import nixpkgs { inherit system; }).hello}/bin/hello"
            touch $out
          '';
      });
}
