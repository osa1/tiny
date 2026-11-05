{
  description = "tiny IRC client";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.home-manager.url = "github:nix-community/home-manager/release-26.05";
  inputs.rust-overlay.url = "github:oxalica/rust-overlay";
  inputs.rust-overlay.inputs.nixpkgs.follows = "nixpkgs";

  outputs =
    {
      self,
      nixpkgs,
      home-manager,
      rust-overlay,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      eachSystem = nixpkgs.lib.genAttrs systems;
      pkgsBySystem = eachSystem (
        system:
        import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        }
      );
      rustToolchains = eachSystem (
        system: pkgsBySystem.${system}.rust-bin.fromRustupToolchainFile ./rust-toolchain
      );
    in
    {
      homeManagerModules.default = import ./nix/home-manager.nix { inherit self; };

      packages = eachSystem (
        system:
        let
          pkgs = pkgsBySystem.${system};
          toolchain = rustToolchains.${system};
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
          tiny = pkgs.callPackage ./nix/package.nix { inherit rustPlatform; };
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
          pkgs = pkgsBySystem.${system};
          rustToolchain = rustToolchains.${system};
          inherit (self.packages.${system}) tiny;
        };
      });

      formatter = eachSystem (system: nixpkgs.legacyPackages.${system}.nixfmt-rfc-style);
    };
}
