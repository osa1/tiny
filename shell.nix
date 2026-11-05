{
  pkgs,
  tiny,
  rustToolchain,
}:

pkgs.mkShell {
  inputsFrom = [ tiny ];

  packages =
    with pkgs;
    [
      rustToolchain
      pkg-config
    ]
    ++ lib.optionals stdenv.hostPlatform.isLinux [
      openssl
      dbus
    ];
}
