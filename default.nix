with import <nixpkgs> {};
stdenv.mkDerivation {
  name = "mttn";
  src = ./.;

  nativeBuildInputs = [
    cargo
  ];
  buildInputs = [
    gdb nasm rustfmt git clippy
  ];
}
