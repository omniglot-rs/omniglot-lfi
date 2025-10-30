{ pkgs ? import <nixpkgs> {} }:

let
  lfi-runtime-git-rev =
    "c5b3c77dfd029df0a3bf59d0b24d171d4a324377";

  lfi-runtime = pkgs.stdenv.mkDerivation rec {
    pname = "lfi-runtime";
    version = "git-${builtins.substring 0 14 lfi-runtime-git-rev}";

    src = pkgs.fetchFromGitHub {
      owner = "lfi-project";
      repo = pname;
      rev = lfi-runtime-git-rev;

      nativeBuildInputs = with pkgs; [ meson git cacert jc jq patch ];

      postFetch = ''
          cd "$out"

          patch -p1 <${./lfi-runtime_subprojects-pin-dependencies-to-git-revisions.patch}

          function fetchSubprojects() {
            pushd "$1"

            for prj in ./subprojects/*.wrap; do
              prjname="$(basename "$prj" .wrap)"
              prjdir="$(cat "$prj" | jc --ini | jq -r '."wrap-git"."directory" // "'"$prjname"'"')"
              echo "=====> Fetching $prj, placed in $prjdir"
              meson subprojects download "$(basename "$prj" .wrap)"
              rm -r "./subprojects/$prjdir/.git"
            done

            popd
          }

          fetchSubprojects ""
          # Required to work around "patch directory does not exist: libargp" issue
          mkdir -p "subprojects/lfi-verifier/subprojects/packagefiles/libargp"
          fetchSubprojects "subprojects/lfi-verifier"
        '';

      sha256 = "sha256-/abOAknwwKsaa6wHZQoFGEUUKmmu4YAUo72PKSc8It4=";
    };

    nativeBuildInputs = with pkgs; [ meson ninja ];
    buildInputs = with pkgs; [ glibc glibc.static ];

    doCheck = false;
  };

in
  pkgs.llvmPackages.stdenv.mkDerivation rec {
    name = "omniglot-lfi-devshell";

    buildInputs = with pkgs; [
      rustup
      lfi-runtime
    ];

    nativeBuildInputs = with pkgs; [
      pkg-config
      clang
    ];

    shellHook = ''
      export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"
      export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [ lfi-runtime ]}:$LD_LIBRARY_PATH"
    '';
  }
