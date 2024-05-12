{
  inputs = {
    nixpkgs.url = "github:/NixOS/nixpkgs/nixos-23.11";
  };

  outputs = { self, nixpkgs }:
  let 
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
    python_env = pkgs.python3.withPackages (ps: with ps; [
      mido
      packaging
      ipython
    ]);
  in
  {
    devShells.${system}.default = pkgs.mkShell {
      buildInputs = with pkgs; [
        wireshark
        python_env
      ];
    };
  };
}