{
  lib,
  rustPlatform,
  makeWrapper,
  cacert,
}:

let
  cargoToml = lib.importTOML ./caldir-cli/Cargo.toml;
  workspace = (lib.importTOML ./Cargo.toml).workspace;
in
rustPlatform.buildRustPackage {
  pname = "caldir";
  version = cargoToml.package.version;

  # Only Rust sources, so docs/website edits don't trigger rebuilds.
  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions (
      [
        ./Cargo.toml
        ./Cargo.lock
      ]
      ++ map (member: ./. + "/${member}") workspace.members
    );
  };

  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [ makeWrapper ];

  preCheck = ''
    export SSL_CERT_FILE="${cacert}/etc/ssl/certs/ca-bundle.crt"
  '';

  postFixup = ''
    wrapProgram "$out/bin/caldir" --prefix PATH : "$out/bin"
  '';

  meta = {
    description = "Store your calendar as a directory of ICS files, synced with cloud providers";
    homepage = "https://caldir.org";
    license = lib.licenses.mit;
    mainProgram = "caldir";
  };
}
