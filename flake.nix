{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    flake-parts.inputs.nixpkgs-lib.follows = "nixpkgs";
  };

  outputs = inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      perSystem = { pkgs, lib, config, ... }: {
        packages = {
          default = config.packages.mkdmg;

          mkdmg =
            let manifest = lib.importTOML ./Cargo.toml;
            in pkgs.rustPlatform.buildRustPackage (final: {
              pname = manifest.package.name;
              version = manifest.package.version;

              src = lib.cleanSource ./.;

              cargoLock.lockFile = ./Cargo.lock;
              buildFeatures = [ "cli" ];
              cargoBuildFlags = [ "--bin" "mkdmg" ];
              cargoInstallFlags = [ "--bin" "mkdmg" ];
            });
        };

        devShells.default = pkgs.mkShellNoCC {
          buildInputs = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            cargo-release
          ];
        };
      };
    };
}
