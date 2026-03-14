{
  description = "Aether Core - Hermetic Reproducible Build Environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, crane, fenix, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
          config = {
            allowUnfree = true;
            permittedInsecurePackages = [];
          };
        };

        toolchain = pkgs.rust-bin.nightly."2026-03-01".default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" "llvm-tools-preview" ];
          targets = [ "wasm32-wasip2" "wasm32-unknown-unknown" "x86_64-unknown-linux-gnu" ];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

        src = craneLib.cleanCargoSource (craneLib.path ./.);

        commonArgs = {
          inherit src;
          strictDeps = true;

          nativeBuildInputs = with pkgs; [
            pkg-config
            protobuf
            cmake
          ];

          buildInputs = with pkgs; [
            openssl
            zlib
          ] ++ lib.optionals stdenv.isDarwin [
            darwin.apple_sdk.frameworks.Security
            darwin.apple_sdk.frameworks.SystemConfiguration
          ];

          CARGO_BUILD_TARGET = "x86_64-unknown-linux-gnu";
          RUSTFLAGS = "--remap-path-prefix ${builtins.storeDir}=/nix/store";
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        aether-core = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          doCheck = false;
        });

        aether-core-wasm = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          CARGO_BUILD_TARGET = "wasm32-wasip2";
          doCheck = false;
        });

      in {
        packages = {
          default = aether-core;
          aether-core = aether-core;
          aether-core-wasm = aether-core-wasm;
        };

        checks = {
          cargo-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets --all-features -- -D warnings";
          });

          cargo-test = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            cargoNextestExtraArgs = "--all-features";
          });

          cargo-doc = craneLib.cargoDoc (commonArgs // {
            inherit cargoArtifacts;
          });

          cargo-fmt = craneLib.cargoFmt {
            inherit src;
          };
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ aether-core ];

          buildInputs = with pkgs; [
            toolchain
            cargo-nextest
            cargo-llvm-cov
            cargo-mutants
            cargo-audit
            cargo-deny
            wasmtime
            protobuf
            grpcurl
            dive
            trunk
            wasm-pack
            elan
            coq
          ];

          shellHook = ''
            export RUST_SRC_PATH="${toolchain}/lib/rustlib/src/rust/library"
            export PROTOC="${pkgs.protobuf}/bin/protoc"
            export AETHER_RUNTIME_MODE=development
            export SOURCE_DATE_EPOCH=1733097600
            echo "Aether Development Environment"
            echo "Rust: $(rustc --version)"
            echo "Wasmtime: $(wasmtime --version)"
          '';

          RUST_LOG = "debug";
          RUST_BACKTRACE = "full";
        };
      }
    );
}
