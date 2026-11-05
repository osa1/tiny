{
  description = "tiny IRC client";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.home-manager.url = "github:nix-community/home-manager/release-26.05";

  outputs =
    {
      self,
      nixpkgs,
      home-manager,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      eachSystem = nixpkgs.lib.genAttrs systems;
    in
    {
      homeManagerModules.default = import ./nix/home-manager.nix { inherit self; };

      packages = eachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          tiny = pkgs.callPackage ./nix/package.nix { };
        in
        {
          default = tiny;
          inherit tiny;
        }
      );

      checks = eachSystem (system: {
        inherit (self.packages.${system}) tiny;
        home-manager = import ./nix/tests/home-manager.nix {
          pkgs = home-manager.inputs.nixpkgs.legacyPackages.${system};
          inherit self home-manager;
        };
      });

      devShells = eachSystem (system: {
        default = import ./shell.nix {
          pkgs = nixpkgs.legacyPackages.${system};
          inherit (self.packages.${system}) tiny;
        };
      });

      formatter = eachSystem (system: nixpkgs.legacyPackages.${system}.nixfmt-rfc-style);
    };
}
