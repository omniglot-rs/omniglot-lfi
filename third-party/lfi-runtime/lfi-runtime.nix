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
  # `abort-callback` branch:
  lfi-runtime-git-rev = "d912973d674e0c5080f9000e085f5516b3d1d5dc";

in
stdenv.mkDerivation rec {
  pname = "lfi-runtime";
  version = "git-${builtins.substring 0 14 lfi-runtime-git-rev}";

  src = fetchFromGitHub {
    # owner = "lfi-project";
    owner = "lschuermann";
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

      fetchSubprojects "./"
      fetchSubprojects "./subprojects/lfi-verifier"
    '';

    sha256 = "sha256-GTSIe0THN2YS5WghH1wv2o0OGM1KpBMZo0R8VLCcaqk=";
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
