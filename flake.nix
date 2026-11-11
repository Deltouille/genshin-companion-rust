{
  description = "Genshin Companion - dev shell Rust + GPUI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    # Toolchain Rust stable à jour (GPUI suit de près la dernière version stable)
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, rust-overlay, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      forAllSystems =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            }
          )
        );
    in
    {
      devShells = forAllSystems (
        pkgs:
        let
          inherit (pkgs) lib stdenv;

          # rustc, cargo, clippy, rustfmt + sources de la std et rust-analyzer
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "rust-src"
              "rust-analyzer"
            ];
          };

          # Bibliothèques dont GPUI a besoin sous Linux
          # (mêmes dépendances que le paquet zed-editor de nixpkgs)
          linuxLibs = with pkgs; [
            fontconfig # découverte des polices
            freetype # rendu des polices
            libxkbcommon # gestion du clavier
            wayland # fenêtres sous Wayland
            libx11 # fenêtres sous X11
            libxcb
            vulkan-loader # rendu GPU
            libGL # repli OpenGL
          ];
        in
        {
          default = pkgs.mkShell {
            packages = [
              rustToolchain
              pkgs.bacon # recompile et affiche les erreurs à chaque sauvegarde
              pkgs.sqlite-interactive # CLI `sqlite3` pour inspecter la base
            ];

            nativeBuildInputs = [ pkgs.pkg-config ];

            buildInputs = [ pkgs.sqlite ] ++ lib.optionals stdenv.hostPlatform.isLinux linuxLibs;

            # GPUI charge Vulkan, Wayland et X11 dynamiquement au lancement :
            # sans ça, `cargo run` compile mais la fenêtre ne s'ouvre pas.
            LD_LIBRARY_PATH = lib.optionalString stdenv.hostPlatform.isLinux (
              lib.makeLibraryPath linuxLibs
            );

            shellHook = ''
              echo "$(rustc --version) - $(cargo --version)"
            '';
          };
        }
      );
    };
}

