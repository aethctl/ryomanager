{
  description = "RyoManager - Ryoku-native system task manager";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      devShells.${system}.default = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [
          go
          wails
          pkg-config
          nodejs_24
        ];

        buildInputs = with pkgs; [
          gtk3
          webkitgtk_4_1
        ];
      };
    };
}
