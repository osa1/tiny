{
  pkgs,
  self,
  home-manager,
}:
let
  inherit (pkgs) lib;
  system = pkgs.stdenv.hostPlatform.system;
  inlineYaml = ''
    # tiny home-manager test
    servers: []
    defaults:
      nicks: [nixcheck]
      realname: "Nix User"
    key_map:
      ctrl_e:
        command: quit
  '';
  settings = {
    servers = [ ];
    defaults = {
      nicks = [ "nixcheck" ];
      realname = "Nix User";
    };
  };
  mkHome =
    homePkgs: extra:
    home-manager.lib.homeManagerConfiguration {
      pkgs = homePkgs;
      modules = [
        self.homeManagerModules.default
        {
          home = {
            username = "tiny-test";
            homeDirectory = if homePkgs.stdenv.isDarwin then "/Users/tiny-test" else "/home/tiny-test";
            stateVersion = "26.05";
          };
        }
        extra
      ];
    };
  disabled = mkHome pkgs {
    programs.tiny = {
      enable = false;
      config = inlineYaml;
      inherit settings;
    };
  };
  enabled = mkHome pkgs { programs.tiny.enable = true; };
  overridden = mkHome pkgs {
    programs.tiny = {
      enable = true;
      package = pkgs.hello;
    };
  };
  inline = mkHome pkgs {
    programs.tiny = {
      enable = true;
      config = inlineYaml;
    };
  };
  noFinalNewline = mkHome pkgs {
    programs.tiny = {
      enable = true;
      config = lib.removeSuffix "\n" inlineYaml;
    };
  };
  file = mkHome pkgs {
    programs.tiny = {
      enable = true;
      config = builtins.readFile ./config.yml;
    };
  };
  structured = mkHome pkgs {
    programs.tiny = {
      enable = true;
      inherit settings;
    };
  };
  conflicting = mkHome pkgs {
    programs.tiny = {
      enable = true;
      config = inlineYaml;
      inherit settings;
    };
  };

  linuxPkgs =
    if pkgs.stdenv.isLinux then pkgs else home-manager.inputs.nixpkgs.legacyPackages.x86_64-linux;
  darwinPkgs =
    if pkgs.stdenv.isDarwin then pkgs else home-manager.inputs.nixpkgs.legacyPackages.aarch64-darwin;
  linux = mkHome linuxPkgs {
    programs.tiny = {
      enable = true;
      config = inlineYaml;
    };
  };
  customLinux = mkHome linuxPkgs {
    xdg.configHome = "/home/tiny-test/.custom-config";
    programs.tiny = {
      enable = true;
      config = inlineYaml;
    };
  };
  darwin = mkHome darwinPkgs {
    programs.tiny = {
      enable = true;
      config = inlineYaml;
    };
  };
  linuxKey = "/home/tiny-test/.config/tiny/config.yml";
  customLinuxKey = "/home/tiny-test/.custom-config/tiny/config.yml";
  darwinKey = "Library/Application Support/tiny/config.yml";
  target = if pkgs.stdenv.isDarwin then darwinKey else ".config/tiny/config.yml";
in
assert pkgs.lib.trivial.release == "26.05";
assert enabled.config.home.version.release == "26.05";
assert enabled.config.programs.tiny.package.outPath == self.packages.${system}.default.outPath;
assert overridden.config.programs.tiny.package.outPath == pkgs.hello.outPath;
assert !(builtins.tryEval conflicting.activationPackage.drvPath).success;
assert linux.config.home.file.${linuxKey}.target == ".config/tiny/config.yml";
assert customLinux.config.home.file.${customLinuxKey}.target == ".custom-config/tiny/config.yml";
assert darwin.config.home.file.${darwinKey}.target == darwinKey;
pkgs.runCommand "tiny-home-manager-check" { } ''
  test ! -e "${disabled.activationPackage}/home-path/bin/tiny"
  test ! -e "${disabled.activationPackage}/home-files/${target}"

  "${enabled.activationPackage}/home-path/bin/tiny" --version
  test ! -e "${enabled.activationPackage}/home-files/${target}"

  test -x "${overridden.activationPackage}/home-path/bin/hello"
  test ! -e "${overridden.activationPackage}/home-path/bin/tiny"

  cmp ${./config.yml} "${inline.activationPackage}/home-files/${target}"
  cmp ${./config.yml} "${file.activationPackage}/home-files/${target}"
  cmp ${pkgs.writeText "tiny-config-no-final-newline" (lib.removeSuffix "\n" inlineYaml)} \
    "${noFinalNewline.activationPackage}/home-files/${target}"
  grep -F 'nixcheck' "${structured.activationPackage}/home-files/${target}"
  grep -F 'Nix User' "${structured.activationPackage}/home-files/${target}"
  ${lib.optionalString pkgs.stdenv.isLinux ''
    cmp ${./config.yml} "${customLinux.activationPackage}/home-files/.custom-config/tiny/config.yml"
  ''}
  touch "$out"
''
