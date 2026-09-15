{
  description = "caldir: your calendar as a directory of ICS files";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      overlays.default = final: prev: { caldir = final.callPackage ./package.nix { }; };

      packages = forAllSystems (pkgs: rec {
        caldir = pkgs.callPackage ./package.nix { };
        default = caldir;
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.caldir ];
          packages = with pkgs; [
            clippy
            rustfmt
            just
          ];
        };
      });

      checks = forAllSystems (pkgs: {
        caldir = self.packages.${pkgs.stdenv.hostPlatform.system}.caldir;
      });
    };
}
