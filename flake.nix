{
  description = "Pay3Flow Nix release, verification and rollback commands";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forEachSystem = f: nixpkgs.lib.genAttrs systems (system:
        f { pkgs = import nixpkgs { inherit system; }; });
    in {
      packages = forEachSystem ({ pkgs }:
        let release = import ./deploy/release.nix { inherit pkgs; };
        in release // {
          deploy-watch = release.deploy;
          default = release.verify;
        });

      apps = forEachSystem ({ pkgs }:
        let packages = self.packages.${pkgs.stdenv.hostPlatform.system};
        in {
          deploy = { type = "app"; program = "${packages.deploy}/bin/pay3flow-deploy"; };
          verify = { type = "app"; program = "${packages.verify}/bin/pay3flow-verify"; };
          rollback = { type = "app"; program = "${packages.rollback}/bin/pay3flow-rollback"; };
          deploy-watch = self.apps.${pkgs.stdenv.hostPlatform.system}.deploy;
          default = self.apps.${pkgs.stdenv.hostPlatform.system}.verify;
        });

      devShells = forEachSystem ({ pkgs }: {
        default = pkgs.mkShell {
          packages = [ pkgs.cargo pkgs.clippy pkgs.rustc pkgs.rustfmt ];
        };
      });
    };
}
