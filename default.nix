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
    xorg.libX11
    xorg.libXcursor
    xorg.libXi
    xorg.libXrandr
  ];

  postInstall = ''
    wrapProgram $out/bin/chomiamos-dashboard \
      --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath [
        pkgs.wayland
        pkgs.libxkbcommon
        pkgs.libGL
        pkgs.fontconfig
        pkgs.xorg.libX11
        pkgs.xorg.libXcursor
        pkgs.xorg.libXi
        pkgs.xorg.libXrandr
      ]}
  '';

  meta = with pkgs.lib; {
    description = "Tableau de bord système officiel natif pour ChomiamOS (Slint GUI & Catppuccin Mocha)";
    homepage = "https://github.com/Chomiam/dashboard-chomiamos";
    license = licenses.mit;
    maintainers = [ "chomiam" ];
    platforms = platforms.linux;
    mainProgram = "chomiamos-dashboard";
  };
}
