{ pkgs ? import <nixpkgs> {} }:

let
  lfi-runtime-git-rev =
    "49a562b0dcfcb435123b5025927c3355e447c740";

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
          fetchSubprojects "subprojects/lfi-verifier"
        '';

      sha256 = "sha256-vFcfMuKnwA+iJ07Q9ThCjnht3w9NaIWFES0Le4/uOWk=";
    };

    nativeBuildInputs = with pkgs; [ meson ninja ];
    buildInputs = with pkgs; [ glibc glibc.static ];

    doCheck = false;
  };

in
  pkgs.llvmPackages.stdenv.mkDerivation rec {
    name = "omniglot-lfi-devshell";

    buildInputs = with pkgs; [
      # Base dependencies
      rustup clang pkg-config

      # LFI runtime (liblfi)
      lfi-runtime
    ];

    shellHook = ''
      # Required for rust-bindgen:
      export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"

      # Required for using liblfi:
      export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [ lfi-runtime ]}:$LD_LIBRARY_PATH"
    '';
  }
