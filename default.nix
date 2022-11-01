with import <nixpkgs> {};
stdenv.mkDerivation {
  name = "mttn";
  nativeBuildInputs = [
    cargo gdb nasm
  ];
  src = ./.;
}
