{
  lib,
  rustPlatform,
  cacert,
}:

rustPlatform.buildRustPackage {
  pname = "tiny";
  version = (builtins.fromTOML (builtins.readFile ../crates/tiny/Cargo.toml)).package.version;

  src = lib.cleanSource ../.;
  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [ "--package=tiny" ];
  cargoTestFlags = [ "--workspace" ];

  preCheck = ''
    export SSL_CERT_FILE=${cacert}/etc/ssl/certs/ca-bundle.crt
  '';

  meta = {
    description = "A terminal IRC client";
    homepage = "https://github.com/osa1/tiny";
    license = lib.licenses.mit;
    mainProgram = "tiny";
    platforms = lib.platforms.unix;
  };
}
