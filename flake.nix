{
  inputs = {
    nixpkgs.url = "github:/NixOS/nixpkgs/nixos-24.05";
    rust-overlay.url = "github:oxalica/rust-overlay";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay, naersk }:
    let
      system = "x86_64-linux";
      overlays = [ (import rust-overlay) ];
      pkgs = import nixpkgs { inherit overlays system; };
      rust-bin = (pkgs.rust-bin.stable.latest.default.override {
        extensions = [ "rust-src" ];
        targets = [ "wasm32-unknown-unknown" ];
      });
      naersk-lib = naersk.lib.${system}.override {
        cargo = rust-bin;
        rustc = rust-bin;
      };
      dev-deps = with pkgs; [
        rust-bin
        rust-analyzer
        rustfmt
        lldb
        cargo-geiger
        wireshark
        cargo-flamegraph
      ];
      build-deps = with pkgs; [ pkg-config ];
      runtime-deps = with pkgs; [ rtmidi alsa-lib.dev ];
    in {
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = dev-deps ++ build-deps ++ runtime-deps;
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.alsa-lib ];
      };

      packages.${system}.midimaxe = naersk-lib.buildPackage {
        name = "midimaxe";
        root = ./.;
        buildInputs = runtime-deps;
        nativeBuildInputs = build-deps;
      };
      defaultPackage.${system} = self.packages.${system}.midimaxe;
    };
}
