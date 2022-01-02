{
  description = "Project for syncing various home-system configuration files.";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-21.11";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
        devShell = with pkgs; mkShell {
          buildInputs = [
            cargo
            rls
            libiconv
            rustc
            rustfmt
          ] ++ lib.optionals stdenv.isDarwin (
            with darwin.apple_sdk.frameworks; [ CoreServices ]
          ) ++ lib.optionals stdenv.isLinux [
              pkgs.openssl
              pkgs.zlib
          ];
        };
      });
}
