{ pkgs, pname, version, rust-toolchain, ... }:

pkgs.rustPlatform.buildRustPackage {
  inherit pname version;

  src = pkgs.lib.cleanSource ./.;

  buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
    pkgs.darwin.apple_sdk.frameworks.CoreServices
  ];
  nativeBuildInputs = [ rust-toolchain ];

  cargoSha256 = "sha256-iddp6u4rWW4v7OEkXLVBMbZxpmdcajj5+ysMHC0woPQ=";
}
