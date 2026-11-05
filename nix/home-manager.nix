{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.tiny;
  system = pkgs.stdenv.hostPlatform.system;
  configDir =
    if pkgs.stdenv.isDarwin then
      "Library/Application Support/tiny"
    else
      "${config.xdg.configHome}/tiny";
in
{
  options.programs.tiny.config = lib.mkOption {
    type = lib.types.nullOr lib.types.str;
    default = null;
    description = ''
      Raw YAML configuration, written without modification. Cannot be used
      together with programs.tiny.settings. The contents are stored in the
      Nix store and must not contain secrets.
    '';
  };

  config = lib.mkIf cfg.enable {
    programs.tiny.package = lib.mkDefault (
      self.packages.${system}.default
        or (throw "tiny: unsupported system ${system}; set programs.tiny.package")
    );

    assertions = [
      {
        assertion = cfg.config == null || cfg.settings == { };
        message = "programs.tiny.config and programs.tiny.settings are mutually exclusive";
      }
    ];

    home.file."${configDir}/config.yml" = lib.mkIf (cfg.config != null) {
      text = cfg.config;
    };
  };
}
