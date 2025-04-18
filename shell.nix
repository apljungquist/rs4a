let
  pkgs = import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/f9ebe33a928b.tar.gz") { };
  mkhelp = pkgs.callPackage ./pkgs/mkhelp { };
in

pkgs.mkShellNoCC {
  packages = with pkgs; [
    cargo
    clang
    clippy
    fd
    git
    llvmPackages.libclang
    nixfmt-rfc-style
    rsync
    rustc
    rustfmt
  ];

  LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
}
