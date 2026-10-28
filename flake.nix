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

      perSystem = { pkgs, lib, system, config, ... }: {
        packages = {
          default = config.packages.mkdmg;

          mkdmg =
            let manifest = lib.importTOML ./Cargo.toml;
            in pkgs.rustPlatform.buildRustPackage (final: {
              pname = manifest.package.name;
              version = manifest.package.version;

              src = lib.cleanSource ./.;
              cargoLock.lockFile = ./Cargo.lock;
            });

          mkdmg-bin =
            let manifest = lib.importTOML ./Cargo.toml;
            in pkgs.stdenvNoCC.mkDerivation (final: {
              pname = manifest.package.name;
              version = manifest.package.version;

              src = let
                prefix = "https://github.com/balthild/mkdmg/releases/download/${final.version}";
                sources = {
                  "x86_64-darwin" = {
                    url = "${prefix}/mkdmg-x86_64-apple-darwin";
                    hash = lib.fakeHash;
                  };
                  "aarch64-darwin" = {
                    url = "${prefix}/mkdmg-aarch64-apple-darwin";
                    hash = lib.fakeHash;
                  };
                };
                in pkgs.fetchurl sources.${system};
            });

          __fetch_release_hash = pkgs.writeShellApplication {
            name = "__fetch_release_hash";
            runtimeInputs = with pkgs; [ toml-cli gh jq xxd coreutils ];
            text = ''
              version=$(toml get Cargo.toml package.version --raw)
              echo "balthild/mkdmg $version"

              info=$(gh api "repos/balthild/mkdmg/releases/$version" \
                --jq '.assets[] | { name, digest }')

              printf '%s\n' "$info" \
                | jq -r 'select(.name == "mkdmg-x86_64-apple-darwin") | .digest' \
                | sed 's/^sha256://' \
                | xxd -r -p | base64 \
                | xargs printf "x86_64-darwin: sha256-%s\n"

              printf '%s\n' "$info" \
                | jq -r 'select(.name == "mkdmg-aarch64-apple-darwin") | .digest' \
                | sed 's/^sha256://' \
                | xxd -r -p | base64 \
                | xargs printf "aarch64-darwin: sha256-%s\n"
            '';
          };
        };

        devShells.default = pkgs.mkShellNoCC {
          buildInputs = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            config.packages.__fetch_release_hash
          ];
        };
      };
    };
}
