{
  description = "Rust flake";
  inputs =
    {
      nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    };
  
  outputs = { self, nixpkgs, ... }@inputs:
    let
     system = "x86_64-linux";
     pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      devShells.${system}.default = pkgs.mkShell
      {
        packages = with pkgs; [
          rustc
          cargo
          rust-analyzer
          openssl
          dbus
          libclang
          clang
          binutils 
          gcc
          glibc
          glibc.dev
        ];
        PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig:${pkgs.dbus.dev}/lib/pkgconfig";
        LIBCLANG_PATH="${pkgs.libclang.lib}/lib";
        C_INCLUDE_PATH="${pkgs.glibc.dev}/include:${pkgs.gcc}/include";
        CPLUS_INCLUDE_PATH="${pkgs.glibc.dev}/include:${pkgs.gcc}/include";
      };
    };
}