{
  description = "tiny IRC client";

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
      eachSystem = nixpkgs.lib.genAttrs systems;
    in
    {
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
