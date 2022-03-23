{
  description = "Project for syncing various home-system configuration files.";

  inputs = {
    cargo2nix.url = "github:cargo2nix/cargo2nix/master";
    flake-compat = {
      url = github:edolstra/flake-compat;
      flake = false;
    };
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-21.11";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, cargo2nix, flake-compat, flake-utils, nixpkgs, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            (import "${cargo2nix}/overlay") rust-overlay.overlay
          ];
        };

        rustPkgs = pkgs.rustBuilder.makePackageSet' {
          rustChannel = "1.56.1";
          packageFun = import ./Cargo.nix;
        };
      in rec {
        packages = {
          homesync = (rustPkgs.workspace.homesync {}).bin;
        };

        defaultPackage = packages.homesync;

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
