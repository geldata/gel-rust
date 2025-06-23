{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/25.05";
    flake-parts.url = "github:hercules-ci/flake-parts";

    # provides rust toolchain
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };

    gel = {
      url = "github:geldata/packages-nix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-parts.follows = "flake-parts";
    };
  };

  outputs = inputs@{ flake-parts, fenix, gel, ... }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "x86_64-darwin" "aarch64-darwin"];
      perSystem = { config, system, pkgs, ... }:
        let
          fenix_pkgs = fenix.packages.${system};

          common = [
            pkgs.just
            pkgs.openssl
            pkgs.pkg-config

            # needed for tests
            gel.packages.${system}.gel-server
            gel.packages.${system}.gel-cli
          ]
          ++ pkgs.lib.optional pkgs.stdenv.isDarwin [
            pkgs.libiconv
            pkgs.darwin.apple_sdk.frameworks.CoreServices
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          ];
        in {

          # toolchain defined in rust-toolchain.toml
          devShells.default = pkgs.mkShell {
            buildInputs = [
              (fenix_pkgs.fromToolchainFile {
                file = ./rust-toolchain.toml;
                sha256 = "sha256-Hn2uaQzRLidAWpfmRwSRdImifGUCAb9HeAqTYFXWeQk=";
              })
            ] ++ common;
          };

          # rust beta version
          devShells.beta = pkgs.mkShell {
            buildInputs = [
              fenix_pkgs.beta.defaultToolchain
            ] ++ common;
          };
        };
    };
}
