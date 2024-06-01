{
  inputs = {
    nixpkgs.url = "github:/NixOS/nixpkgs";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }:
  let 
    system = "x86_64-linux";
    overlays = [ (import rust-overlay) ];
    pkgs = import nixpkgs {
      inherit overlays system;
    };
    python_env = pkgs.python3.withPackages (ps: with ps; [
      python-rtmidi
      mido
      ipython
    ]);
  in
  {
    devShells.${system}.default = pkgs.mkShell {
      buildInputs = with pkgs; [
        (rust-bin.selectLatestNightlyWith (toolchain:
          toolchain.default.override { extensions = [ "rust-src" ]; targets = [ "wasm32-unknown-unknown" ]; }))
        rust-analyzer
        rustfmt        
        wireshark
        python_env
        rtmidi
        pkg-config
        alsa-lib.dev
      ];
      LD_LIBRARY_PATH=pkgs.lib.makeLibraryPath [ pkgs.alsa-lib ];
    };
  };
}