{
  description = "jjdag";

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

      mkJjdag =
        pkgs:
        pkgs.rustPlatform.buildRustPackage {
          pname = "jjdag";
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
          default = mkJjdag pkgs;
          jjdag = mkJjdag pkgs;
        }
      );

      overlays.default = final: prev: {
        jjdag = mkJjdag final;
      };
    };
}
