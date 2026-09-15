{ lib, rustPlatform }:

let
  cargoToml = lib.importTOML ./caldir-cli/Cargo.toml;
in
rustPlatform.buildRustPackage {
  pname = "caldir";
  version = cargoToml.package.version;

  # Only Rust sources, so docs/website edits don't trigger rebuilds.
  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./Cargo.toml
      ./Cargo.lock
      ./caldir-cli
      ./caldir-core
      ./caldir-provider-caldav
      ./caldir-provider-google
      ./caldir-provider-icloud
      ./caldir-provider-outlook
      ./caldir-provider-webcal
    ];
  };

  cargoLock.lockFile = ./Cargo.lock;

  meta = {
    description = "Store your calendar as a directory of ICS files, synced with cloud providers";
    homepage = "https://caldir.org";
    license = lib.licenses.mit;
    mainProgram = "caldir";
  };
}
