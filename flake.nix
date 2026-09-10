{
  description = "ChomiamOS System Dashboard (Tauri v2 + xterm.js + Rust + Catppuccin Mocha)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        packages.default = pkgs.callPackage ./default.nix {};

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/chomiamos-dashboard";
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            pkg-config
            wrapGAppsHook3
          ];
          buildInputs = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            webkitgtk_4_1
            gtk3
            libsoup_3
            openssl
            glib
            cairo
            pango
            gdk-pixbuf
          ];
        };
      }
    );
}
