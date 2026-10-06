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
          cargo
          rustc
          pkg-config
          nodejs_24
        ];

        buildInputs = with pkgs; [
          dbus
          openssl
          glib
          gtk3
          gdk-pixbuf
          cairo
          pango
          atk
          libsoup_3
          webkitgtk_4_1
        ];

        shellHook = ''
          export GIO_MODULE_DIR=${pkgs.glib-networking}/lib/gio/modules
        '';
      };
    };
}
