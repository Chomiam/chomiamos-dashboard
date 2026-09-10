{ pkgs ? import <nixpkgs> {} }:

pkgs.rustPlatform.buildRustPackage rec {
  pname = "chomiamos-dashboard";
  version = "0.2.0";

  src = ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = with pkgs; [
    pkg-config
    makeWrapper
  ];

  buildInputs = with pkgs; [
    fontconfig
    wayland
    libxkbcommon
    libGL
    libx11
    libxcursor
    libxi
    libxrandr
  ];

  postInstall = ''
    install -Dm644 chomiamos-dashboard.desktop $out/share/applications/chomiamos-dashboard.desktop
    install -Dm644 frontend/assets/logo.png $out/share/icons/hicolor/scalable/apps/chomiamos-dashboard.png
    install -Dm644 frontend/assets/logo.png $out/share/pixmaps/chomiamos-dashboard.png
    install -Dm755 scripts/sudo-stdin $out/share/chomiamos-dashboard/scripts/sudo-stdin

    wrapProgram $out/bin/chomiamos-dashboard \
      --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath [
        pkgs.wayland
        pkgs.libxkbcommon
        pkgs.libGL
        pkgs.fontconfig
        pkgs.libx11
        pkgs.libxcursor
        pkgs.libxi
        pkgs.libxrandr
      ]}
  '';

  meta = with pkgs.lib; {
    description = "Tableau de bord système officiel natif pour ChomiamOS (Slint GUI & Catppuccin Mocha)";
    homepage = "https://github.com/Chomiam/chomiamos-dashboard";
    license = licenses.mit;
    maintainers = [ "chomiam" ];
    platforms = platforms.linux;
    mainProgram = "chomiamos-dashboard";
  };
}
