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
              buildFeatures = [ "cli" ];
              cargoBuildFlags = [ "--bin" "mkdmg" ];
              cargoInstallFlags = [ "--bin" "mkdmg" ];
            });

          mkdmg-bin =
            let release = import ./release.nix;
            in pkgs.stdenvNoCC.mkDerivation (final: {
              pname = "mkdmg";
              version = release.version;

              src = pkgs.fetchurl release.sources.${system};
              dontUnpack = true;

              installPhase = ''
                mkdir -p $out/bin
                install -m755 $src $out/bin/mkdmg
              '';
            });

          __mkdmg_update_release = pkgs.writeShellApplication {
            name = "__mkdmg_update_release";
            runtimeInputs = with pkgs; [ gh jq xxd coreutils ];
            text = ''
              repo="balthild/mkdmg"
              release=$(gh release view -R "$repo" --json tagName,assets)
              version=$(printf '%s' "$release" | jq -r '.tagName')

              function asset_url() {
                local name="$1"
                printf '%s\n' "$release" \
                  | jq -r ".assets[] | select(.name == \"$name\") | .url"
              }

              function asset_sha256() {
                local name="$1"
                printf '%s\n' "$release" \
                  | jq -r ".assets[] | select(.name == \"$name\") | .digest" \
                  | sed 's/^sha256://' \
                  | xxd -r -p | base64 \
                  | xargs printf "sha256-%s\n"
              }

              echo "Updating $repo $version"
              cat <<-EOF > release.nix
              {
                version = "$version";
                sources = {
                  "x86_64-darwin" = {
                    url = "$(asset_url "mkdmg-x86_64-apple-darwin")";
                    sha256 = "$(asset_sha256 "mkdmg-x86_64-apple-darwin")";
                  };
                  "aarch64-darwin" = {
                    url = "$(asset_url "mkdmg-aarch64-apple-darwin")";
                    sha256 = "$(asset_sha256 "mkdmg-aarch64-apple-darwin")";
                  };
                };
              }
              EOF
            '';
          };
        };

        devShells.default = pkgs.mkShellNoCC {
          buildInputs = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer
            cargo-release
            config.packages.__mkdmg_update_release
          ];

          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        };
      };
    };
}
