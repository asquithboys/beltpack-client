let
    pkgs = import <nixpkgs> {};

in
pkgs.mkShell{
    name = "beltpacks";
    propagatedBuildInputs = with pkgs; [
        rustup
        cargo-make
        cargo
        SDL2
        iw
        pkg-config
        gcc
        zlib
        xorg.libX11
    ];

}
