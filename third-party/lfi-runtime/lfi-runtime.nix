{
  stdenv,
  fetchFromGitHub,
  meson,
  ninja,
  git,
  cacert,
  jc,
  jq,
  patch,
  glibc,
}:

let
  lfi-runtime-git-rev = "c5b3c77dfd029df0a3bf59d0b24d171d4a324377";

in
stdenv.mkDerivation rec {
  pname = "lfi-runtime";
  version = "git-${builtins.substring 0 14 lfi-runtime-git-rev}";

  src = fetchFromGitHub {
    owner = "lfi-project";
    repo = pname;
    rev = lfi-runtime-git-rev;

    nativeBuildInputs = [
      meson
      git
      cacert
      jc
      jq
      patch
    ];

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

  nativeBuildInputs = [
    meson
    ninja
  ];
  buildInputs = [
    glibc
    glibc.static
  ];

  doCheck = false;
}
