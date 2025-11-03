{
  description = "LFI runtime for Omniglot";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.05";

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

        rustToolchain = fenix.packages."${system}".fromToolchainFile {
          file = ./rust-toolchain.toml;
          sha256 = "sha256-9qmHY60Kelk6KZIOb/cpN5LKrfNiR81CPnTkHYXmBUg=";
        };

        rustPlatform = pkgs.makeRustPlatform {
          rustc = rustToolchain;
          cargo = rustToolchain;
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain (_p: rustToolchain);

        cleanedRustSrc =
          let
            # Path comes with "/nix/store/${hash}-source/" stripped
            relSrcFilter =
              relPath: type:
              # Include the `c_src` files for the `omniglot-lfi` crate:
              (lib.hasPrefix "omniglot-lfi/c_src" relPath)
              # Include C header files and compiled artifacts for the `add` example:
              || (lib.hasPrefix "examples/add/libadd_lfi" relPath);

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

        # Common arguments shared across all individual targets:
        individualCrateArgs = baseRustBuildArgs // {
          inherit cargoArtifacts;

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

          # TODO: run all tests via cargo-nextest, as per crane's documentation:
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

        omniglot-lfi = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "omniglot-lfi";
            cargoExtraArgs = "-p omniglot-lfi";
            src = fileSetForCrate ./omniglot-lfi;
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

      in
      rec {
        packages = {
          inherit
            lfi-runtime
            omniglot-lfi-example-add
            ;
        };

        # Check formatting. `nix flake check` also builds all packages:
        checks = {
          formatting = (treefmt-nix.lib.evalModule pkgs ./treefmt.nix).config.build.check self;
        };

        formatter = (treefmt-nix.lib.evalModule pkgs ./treefmt.nix).config.build.wrapper;

        devShells.default = pkgs.mkShell {
          name = "omniglot-lfi-devshell";

          packages = with pkgs; [
            rustToolchain
            lfi-runtime
            pkg-config
            clang
            lldb
            gdb
          ];

          shellHook = ''
            export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [ lfi-runtime ]}:$LD_LIBRARY_PATH"
          '';
        };
      }
    );
}
