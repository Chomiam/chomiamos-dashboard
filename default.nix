{ pkgs ? import <nixpkgs> {} }:

pkgs.rustPlatform.buildRustPackage rec {
  pname = "chomiamos-dashboard";
  version = "0.5.4";

  src = ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = with pkgs; [
    pkg-config
    wrapGAppsHook3
  ];

  buildInputs = with pkgs; [
    webkitgtk_4_1
    gtk3
    libsoup_3
    openssl
    glib
    glib-networking
    gsettings-desktop-schemas
    cairo
    pango
    gdk-pixbuf
    harfbuzz
  ];

  postInstall = ''
    install -Dm644 chomiamos-dashboard.desktop $out/share/applications/chomiamos-dashboard.desktop
    install -Dm644 frontend/assets/logo.png $out/share/icons/hicolor/scalable/apps/chomiamos-dashboard.png
    install -Dm644 frontend/assets/logo.png $out/share/pixmaps/chomiamos-dashboard.png
  '';

  preFixup = ''
    gappsWrapperArgs+=(
      --set WEBKIT_DISABLE_DMABUF_RENDERER "1"
    )
  '';

  meta = with pkgs.lib; {
    description = "Tableau de bord système officiel natif pour ChomiamOS (Tauri v2 + xterm.js & Catppuccin Mocha)";
    homepage = "https://github.com/Chomiam/chomiamos-dashboard";
    license = licenses.mit;
    maintainers = [ "chomiam" ];
    platforms = platforms.linux;
    mainProgram = "chomiamos-dashboard";
  };
}
