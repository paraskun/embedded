{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };

          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            targets = [ "thumbv6m-none-eabi" "riscv32imc-unknown-none-elf" ];
            extensions = [ "rust-src" "llvm-tools" ];
          };
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustToolchain
              espflash
              elf2uf2-rs
              picotool
              python3
              python3Packages.pip
              python3Packages.virtualenv
              stdenv.cc.cc.lib
              libGL
              xorg.libX11
              expat
              zlib
            ];

            shellHook = ''
              export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [
                pkgs.stdenv.cc.cc.lib
                pkgs.libGL
                pkgs.xorg.libX11
                pkgs.expat
                pkgs.zlib
              ]}:$LD_LIBRARY_PATH"
            '';
          };
        });
    };
}

