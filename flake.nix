{
  description = "rs4a developer environment";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    mkhelp = {
      url = "github:apljungquist/mkhelp-rs";
      inputs.rust-overlay.follows = "rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix4a = {
      url = "github:apljungquist/nix4a";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "github:NixOS/nixpkgs/25.11";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      flake-utils,
      mkhelp,
      nix4a,
      nixpkgs,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        rustPlatform = pkgs.makeRustPlatform {
          rustc = rustToolchain;
          cargo = rustToolchain;
        };
        buildWorkspaceMember = dir: pkgs.callPackage ./nix/default.nix { inherit rustPlatform dir; };
        # The version is whatever the pinned `nix4a` provides; keep it in sync with:
        # - .devcontainer/acap-native-sdk-12-aarch64/devcontainer.json
        # - .devcontainer/acap-native-sdk-12-armv7hf/devcontainer.json
        # - .github/workflows/fuzz.yaml
        # - .github/workflows/main.yaml
        # - bin/create-venv.sh
        acap-native-sdk = nix4a.packages.${system}.acap-native-sdk-12;
      in
      {

        formatter = pkgs.nixfmt-rfc-style;

        checks = self.packages.${system};

        packages = {
          inherit acap-native-sdk;
          cli4a = buildWorkspaceMember "cli4a";
          device-finder = buildWorkspaceMember "device-finder";
          device-inventory = buildWorkspaceMember "device-inventory";
          device-manager = buildWorkspaceMember "device-manager";
          firmware-inventory = buildWorkspaceMember "firmware-inventory";
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            clang
            fd
            git
            mkhelp.packages.${system}.default
            nixfmt-rfc-style
            rustToolchain
          ];

          shellHook = ''
            # Prevent cargo from finding programs in the default cargo home by **appending** it to the path because
            # otherwise cargo will **prepend** it to the path e.g. when looking for clippy.
            export PATH="$PATH:$HOME/.cargo/bin"
            # Tell rust-analyzer where to find the standard library
            export RUST_SRC_PATH="${rustToolchain}/lib/rustlib/src/rust/library"
            # Some aspects of the SDK are known to break when extracted to a location different from `/opt/axis`:
            # - `acap-build`
            export ACAP_SDK_LOCATION="${acap-native-sdk}"
          '';
        };
      }
    );
}
