{
  inputs.nixpkgs.url = "nixpkgs";
  description = "EyeTrackVR Phoenix";

  outputs = inputs@{self, nixpkgs}: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {
      inherit system;
    };
  in {
    devShells = {
      x86_64-linux.default = pkgs.mkShell rec {
        buildInputs = with pkgs; [
          rustup mold curl

          # Serialport
          pkg-config udev

          # OpenCV
          (opencv.override { enableGtk2 = true; })
          pkg-config llvm clang libclang stdenv.cc.cc.lib

          # Tauri
          pkg-config dbus
          librsvg libsoup_3
          gobject-introspection
          at-spi2-atk atkmm harfbuzz udev
          gtk3 glib gdk-pixbuf glib-networking
          cairo openssl.dev pango webkitgtk_4_1
        ];

        shellHook = ''
          rustup default stable
          rustup component add rust-src rust-std
          rustup component add rust-docs rust-analyzer
          # TODO: make sure this works, might need full pkgs path
          export XDG_DATA_DIRS=$GSETTINGS_SCHEMAS_PATH:$XDG_DATA_DIRS
          export GIO_MODULE_DIR="${pkgs.glib-networking}/lib/gio/modules/"
          export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${builtins.toString (pkgs.lib.makeLibraryPath buildInputs)}";
          export RUSTFLAGS="$RUSTFLAGS -C linker=${pkgs.clang}/bin/clang -C link-arg=-fuse-ld=${pkgs.mold}/bin/mold"
        '';
      };
    };
  };
}
