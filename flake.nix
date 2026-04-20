{
  description = "Majjit: A TUI to manipulate the Jujutsu DAG";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];

      forAllSystems = nixpkgs.lib.genAttrs systems;

      mkMajjit =
        pkgs:
        pkgs.rustPlatform.buildRustPackage {
          pname = "majjit";
          version = "0.1.0";
          src = self;
          cargoLock.lockFile = ./Cargo.lock;
        };
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = mkMajjit pkgs;
          majjit = mkMajjit pkgs;
        }
      );

      overlays.default = final: prev: {
        majjit = mkMajjit final;
      };
    };
}
