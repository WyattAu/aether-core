{
  description = "Project Aether: The Post-Container Application OS";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        
        # Enforce specific Rust version
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            mold        # Fast linker
            clang_18    # For FFI/Bindgen
            llvmPackages_18.llvm
            openssl     # For native-tls support
            wasmtime    # Host runtime testing
            binaryen    # For wasm-opt
            protobuf    # For mesh-protocol.proto
            protoc-gen-rust
            foundationdb # For cluster state
            firecracker  # For system actors
            cargo-nextest # Faster testing
            cargo-mutants # Mutation testing
            cargo-vet     # Supply chain security
            bacon         # Background testing
          ];

          # SOP Environment Variables
          env = {
            # Force linker to be mold for faster local dev
            RUSTFLAGS = "-C link-arg=-fuse-ld=mold";
            # Ensure the compiler finds the right C headers
            LIBCLANG_PATH = "${pkgs.llvmPackages_18.libclang.lib}/lib";
          };

          shellHook = ''
            echo "🛡️  Aether Omni-Protocol Environment Loaded."
            echo "   - Rust $(rustc --version)"
            echo "   - WASM Target: wasm32-wasi"
          '';
        };
      });
}