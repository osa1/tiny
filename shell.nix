{ pkgs, tiny }:

pkgs.mkShell {
  inputsFrom = [ tiny ];

  packages =
    with pkgs;
    [
      cargo
      rustc
      rustfmt
      clippy
      pkg-config
    ]
    ++ lib.optionals stdenv.hostPlatform.isLinux [
      openssl
      dbus
    ];
}
