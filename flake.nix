# -*- fill-column: 80; -*-

{
  description = "LFI runtime for Omniglot";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";

    treefmt-nix.url = "github:numtide/treefmt-nix";

    flake-utils.url = "github:numtide/flake-utils";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      treefmt-nix,
      fenix,
      crane,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages."${system}";
        inherit (pkgs) lib;

        lfi-runtime = pkgs.callPackage ./third-party/lfi-runtime/lfi-runtime.nix { };

        rustToolchain = fenix.packages."${system}".fromToolchainFile (
          let
            # Serves as a sanity check, to not silently change the toml
            # file without updating the Nix hash in this file.
            rustToolchainTomlExpectedHash = "17a3092000cfdbe5bf26ccb122cbe7017fc71a7d03851dc1cce5b4b22223b499";
            rustToolchainTomlHash = builtins.hashString "sha256" (builtins.readFile ./rust-toolchain.toml);

            rustToolchainTomlNixHash = "sha256-Di+IXIUa+MEPYM7pUUjYmgR25SLFbGF3SEsK4DSoY6c=";
          in
          {
            file = ./rust-toolchain.toml;
            sha256 =
              if (rustToolchainTomlHash != rustToolchainTomlExpectedHash) then
                builtins.throw (
                  "Unexpected rustToolchainTomlHash \"${rustToolchainTomlHash}\", "
                  + "expecting \"${rustToolchainTomlExpectedHash}\". If you "
                  + "intended to change the rust-toolchain.toml file, copy this "
                  + "new hash into the rustToolchainTomlExpectedHash variable, and "
                  + "update the rustToolchainTomlNixHash variable by clearing it "
                  + "and setting it to the value produced by Nix."
                )
              else
                rustToolchainTomlNixHash;
          }
        );

        treefmt =
          (treefmt-nix.lib.evalModule (pkgs.extend (
            self: super: {
              rustfmt = rustToolchain;
            }
          )) ./treefmt.nix).config.build;

        craneLib = (crane.mkLib pkgs).overrideToolchain (_p: rustToolchain);

        cleanedRustSrc =
          let
            # Path comes with "/nix/store/${hash}-source/" stripped
            relSrcFilter =
              relPath: type:
              # Include the `c_src` files for the `omniglot-lfi` crate:
              (lib.hasPrefix "omniglot-lfi/c_src" relPath)
              # Include C header files and compiled artifacts for the `tests` crate:
              || (lib.hasPrefix "tests/liboglfitests_lfi" relPath)
              # Include C header files and compiled artifacts for the `microbenchmarks` crate:
              || (lib.hasPrefix "microbenchmarks/liboglfiubench_lfi" relPath)
              # Include C header files and compiled artifacts for the `add` example:
              || (lib.hasPrefix "examples/add/libadd_lfi" relPath)
              # Include C header files for the `brotli` example:
              || (lib.hasPrefix "examples/brotli/og_brotli_lfi/og_brotli_lfi.h" relPath)
              # Include the prebuilt archive manifest for the brotli example:
              || (relPath == "examples/brotli/og_brotli_lfi_prebuilt.json")
              # Include the vendored compression test source for the brotli example:
              || (relPath == "examples/brotli/vanity_fair.txt")
              # Include C header files for the `libpng` example:
              || (lib.hasPrefix "examples/libpng/og_libpng_lfi/libpng_nojmp.h" relPath)
              # Include the prebuilt archive manifest for the libpng example:
              || (relPath == "examples/libpng/og_libpng_lfi_prebuilt.json")
              # Include C header files for the `sodium` example:
              || (lib.hasPrefix "examples/sodium/og_sodium_lfi/og_sodium_lfi.h" relPath)
              # Include the prebuilt archive manifest for the sodium example:
              || (relPath == "examples/sodium/og_sodium_lfi_prebuilt.json")
              # Include C header files for the `llhttp` example:
              || (lib.hasPrefix "examples/llhttp/og_llhttp_lfi/og_llhttp_lfi.h" relPath)
              # Include the prebuilt archive manifest for the llhttp example:
              || (relPath == "examples/llhttp/og_llhttp_lfi_prebuilt.json")
              # Include the vendored HTTP request sample for the llhttp example:
              || (relPath == "examples/llhttp/get_wikipedia_org_req.txt");

            # Strip "/nix/store/${hash}-source/" prefix:
            trimStorePathPrefix =
              path: builtins.head (builtins.match "^\/nix\/store\/[a-zA-Z0-9]+\-source\/(.*)" path);

            # Combine crane's Cargo source filter and a custom one operating on
            # relative paths, with "/nix/store/${hash}-source/" stripped.
            srcFilter =
              path: type:
              (craneLib.filterCargoSources path type) || (relSrcFilter (trimStorePathPrefix path) type);
          in
          lib.cleanSourceWith {
            src = ./.;
            filter = srcFilter;
            # Be reproducible, regardless of the directory name
            name = "omniglot-lfi-src";
          };

        baseRustBuildArgs = {
          src = cleanedRustSrc;
          strictDeps = true;
        };

        # Build *just* the cargo dependencies (of the entire workspace), so we
        # can reuse all of that work (e.g. via cachix) when running in CI:
        cargoArtifacts = craneLib.buildDepsOnly baseRustBuildArgs;

        # Common arguments for all packages that depend on LFI and/or
        # rust-bindgen:
        lfiBindgenBuildArgs = {
          buildInputs = with pkgs; [
            lfi-runtime

            # Provides `libgcc_s.so.1`:
            stdenv.cc.cc
          ];

          nativeBuildInputs = with pkgs; [
            pkg-config
            clang

            # Set the rpath to include the absolute path to `liblfi.so` and
            # `libgcc_s.so.1` for targets that require it:
            autoPatchelfHook
          ];

          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
        };

        # Common arguments shared across all individual targets:
        individualCrateArgs =
          baseRustBuildArgs
          // lfiBindgenBuildArgs
          // {
            inherit cargoArtifacts;

            # Run all tests via cargo-nextest, as per crane's documentation:
            # https://crane.dev/examples/quick-start-workspace.html
            doCheck = false;
          };

        # Ideally we'd like to avoid rebuilding unchanged
        # packages, and thus only include the sources necessary
        # for building any given crate. However, that requires
        # us to use wildcards in our `Cargo.toml`, which is
        # incompatible with the current workspace layout.
        #
        # fileSetForCrate =
        #   crate:
        #   lib.fileset.toSource {
        #     root = ./.;
        #     fileset = lib.fileset.unions [
        #       ./Cargo.toml
        #       ./Cargo.lock
        #       (craneLib.fileset.commonCargoSources ./omniglot-lfi)
        #       (craneLib.fileset.commonCargoSources crate)
        #     ];
        #   };
        #
        # So, instead, use the whole rustSrc as input:
        fileSetForCrate = _crate: cleanedRustSrc;

        # Environment variables pointing to prebuilt LFI binary archives in the
        # Nix store, to prevent the crates' build script from downloading them:
        prebuiltArchives = {
          OG_BROTLI_LFI_PREBUILT_ARCHIVE = builtins.fetchurl (
            builtins.fromJSON (builtins.readFile ./examples/brotli/og_brotli_lfi_prebuilt.json)
          );

          OG_LIBPNG_LFI_PREBUILT_ARCHIVE = builtins.fetchurl (
            builtins.fromJSON (builtins.readFile ./examples/libpng/og_libpng_lfi_prebuilt.json)
          );

          OG_SODIUM_LFI_PREBUILT_ARCHIVE = builtins.fetchurl (
            builtins.fromJSON (builtins.readFile ./examples/sodium/og_sodium_lfi_prebuilt.json)
          );

          OG_LLHTTP_LFI_PREBUILT_ARCHIVE = builtins.fetchurl (
            builtins.fromJSON (builtins.readFile ./examples/llhttp/og_llhttp_lfi_prebuilt.json)
          );
        };

        omniglot-lfi = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "omniglot-lfi";
            cargoExtraArgs = "-p omniglot-lfi";
            src = fileSetForCrate ./omniglot-lfi;
          }
        );

        omniglot-lfi-microbenchmarks = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "omniglot-lfi-microbenchmarks";
            cargoExtraArgs = "-p omniglot-lfi-microbenchmarks";
            src = fileSetForCrate ./microbenchmarks;
          }
        );

        omniglot-lfi-example-add = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "omniglot-lfi-example-add";
            cargoExtraArgs = "-p omniglot-lfi-example-add";
            src = fileSetForCrate ./examples/add;
          }
        );

        omniglot-lfi-example-brotli = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "omniglot-lfi-example-brotli";
            cargoExtraArgs = "-p omniglot-lfi-example-brotli";
            src = fileSetForCrate ./examples/brotli;

            # Prevent the build script from attempting to download the prebuilt
            # `og_brotli_lfi` library from GitHub releases:
            inherit (prebuiltArchives) OG_BROTLI_LFI_PREBUILT_ARCHIVE;
          }
        );

        omniglot-lfi-example-libpng = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "omniglot-lfi-example-libpng";
            cargoExtraArgs = "-p omniglot-lfi-example-libpng";
            src = fileSetForCrate ./examples/libpng;

            # Prevent the build script from attempting to download the prebuilt
            # `og_libpng_lfi` library from GitHub releases:
            inherit (prebuiltArchives) OG_LIBPNG_LFI_PREBUILT_ARCHIVE;
          }
        );

        omniglot-lfi-example-sodium = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "omniglot-lfi-example-sodium";
            cargoExtraArgs = "-p omniglot-lfi-example-sodium";
            src = fileSetForCrate ./examples/sodium;

            # Prevent the build script from attempting to download the prebuilt
            # `og_sodium_lfi` library from GitHub releases:
            inherit (prebuiltArchives) OG_SODIUM_LFI_PREBUILT_ARCHIVE;
          }
        );

        omniglot-lfi-example-llhttp = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "omniglot-lfi-example-llhttp";
            cargoExtraArgs = "-p omniglot-lfi-example-llhttp";
            src = fileSetForCrate ./examples/llhttp;

            # Prevent the build script from attempting to download the prebuilt
            # `og_llhttp_lfi` library from GitHub releases:
            inherit (prebuiltArchives) OG_LLHTTP_LFI_PREBUILT_ARCHIVE;
          }
        );

        # Run tests with cargo-nextest. We set `doCheck = false` on
        # other crate derivations so we do not the tests twice.
        omniglot-lfi-workspace-nextest = craneLib.cargoNextest (
          baseRustBuildArgs
          // lfiBindgenBuildArgs
          // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
            cargoNextestPartitionsExtraArgs = "--no-tests=pass";

            # Otherwise, nextest can't find `liblfi.so`:
            LD_LIBRARY_PATH = lib.makeLibraryPath [ lfi-runtime ];
          }
          // (
            # Include environment variables pointing to prebuilt archives, to
            # prevent the example build scripts from attempting to download them
            # from GitHub releases:
            prebuiltArchives
          )
        );

      in
      rec {
        packages = {
          inherit
            lfi-runtime
            omniglot-lfi
            omniglot-lfi-microbenchmarks
            omniglot-lfi-example-add
            omniglot-lfi-example-brotli
            omniglot-lfi-example-libpng
            omniglot-lfi-example-sodium
            omniglot-lfi-example-llhttp
            ;
        };

        checks = {
          formatting = treefmt.check self;
          inherit omniglot-lfi-workspace-nextest;
        }
        // packages;

        formatter = treefmt.wrapper;

        devShells =
          let
            commonPackages = with pkgs; [
              rustToolchain
              pkg-config
              clang
              lldb
              gdb
            ];
          in
          {
            default = pkgs.mkShell {
              name = "omniglot-lfi-devshell";

              packages = commonPackages ++ [
                lfi-runtime
              ];

              shellHook = ''
                if [ -n "''${LFI_RUNTIME_INSTALL_PREFIX}" ]; then
                    echo "ERROR: This development shell does not respect the" \
                      "\''${LFI_RUNTIME_INSTALL_PREFIX} environment variable." \
                      "Either clear this variable, or use the" \
                      "\"external-lfi-runtime\" development shell instead." >&2
                    exit 1
                fi

                export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"
                export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [ lfi-runtime ]}:''${LD_LIBRARY_PATH}"
              '';
            };

            external-lfi-runtime = pkgs.mkShell {
              name = "omniglot-lfi-external-lfi-runtime-devshell";

              packages = commonPackages;

              shellHook = ''
                if [ ! -n "''${LFI_RUNTIME_INSTALL_PREFIX}" ]; then
                    echo "ERROR: You must set the \''${LFI_RUNTIME_INSTALL_PREFIX}" \
                      "environment variable for this devshell derivation." >&2
                    exit 1
                fi

                LFI_RUNTIME_PKG_CONFIG_PATH="''${LFI_RUNTIME_INSTALL_PREFIX}/lib/pkgconfig"
                if [ ! -f "''${LFI_RUNTIME_PKG_CONFIG_PATH}/lfi.pc" ]; then
                    echo "WARNING: \''${LFI_RUNTIME_PKG_CONFIG_PATH}/lfi.pc" \
                     "does not exist. Are you sure you've set the" \
                     "\''${LFI_RUNTIME_INSTALL_PREFIX} enviroment variable to" \
                     "the correct path, and built / installed the LFI runtime" \
                     "into this prefix?" >&2
                    echo "LFI_RUNTIME_PKG_CONFIG_PATH=''${LFI_RUNTIME_PKG_CONFIG_PATH}" >&2
                fi

                export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"
                export LD_LIBRARY_PATH="''${LFI_RUNTIME_INSTALL_PREFIX}/lib:''${LD_LIBRARY_PATH:-}"
                export PKG_CONFIG_PATH="''${LFI_RUNTIME_PKG_CONFIG_PATH}:''${PKG_CONFIG_PATH:-}"
              '';
            };
          };
      }
    );
}
