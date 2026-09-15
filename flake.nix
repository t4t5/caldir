{
  description = "caldir: your calendar as a directory of ICS files";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
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

      checks = forAllSystems (pkgs: rec {
        caldir = self.packages.${pkgs.stdenv.hostPlatform.system}.caldir;
        provider-discovery = pkgs.runCommand "caldir-provider-discovery" { } ''
          # Missing arguments list providers without contacting remote services.
          if PATH="" ${pkgs.lib.getExe caldir} connect > providers 2>&1; then
            echo "Expected connect to require a provider argument"
            exit 1
          fi
          cat providers
          for binary in ${caldir}/bin/caldir-provider-*; do
            test -x "$binary"
            provider="''${binary##*/caldir-provider-}"
            grep -Fx "  $provider" providers
          done
          touch "$out"
        '';
      });
    };
}
