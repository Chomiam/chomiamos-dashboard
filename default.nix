{ pkgs ? import <nixpkgs> {} }:

pkgs.rustPlatform.buildRustPackage rec {
  pname = "chomiamos-dashboard";
  version = "0.1.0";

  src = ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = with pkgs; [
    pkg-config
  ];

  buildInputs = with pkgs; [
    openssl
  ];

  meta = with pkgs.lib; {
    description = "Tableau de bord système officiel pour ChomiamOS (Catppuccin Mocha & Rust)";
    homepage = "https://github.com/Chomiam/dashboard-chomiamos";
    license = licenses.mit;
    maintainers = [ "chomiam" ];
    platforms = platforms.linux;
    mainProgram = "chomiamos-dashboard";
  };
}
