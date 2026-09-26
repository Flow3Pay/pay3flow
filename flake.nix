{
  description = "Pay3Flow k3s deployment and in-cluster image builds";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forEachSystem = f: nixpkgs.lib.genAttrs systems (system:
        f { pkgs = import nixpkgs { inherit system; }; });
    in {
      packages = forEachSystem ({ pkgs }:
        let
          applicationTemplate = ./deploy/kubernetes.yaml.in;
          servicesTemplate = ./deploy/in-cluster-services.yaml.in;
          renderScript = pkgs.writeScriptBin "render-pay3flow-k8s" ''
            #!${pkgs.ruby}/bin/ruby
            ENV["PAY3FLOW_K8S_TEMPLATE"] ||= "${applicationTemplate}"
            ENV["PAY3FLOW_K8S_SERVICES_TEMPLATE"] ||= "${servicesTemplate}"
            load "${./deploy/render.rb}"
            Pay3flow::Render.main(ARGV)
          '';
          deployK8sScript = pkgs.writeScriptBin "deploy-pay3flow-k8s" ''
            #!${pkgs.ruby}/bin/ruby
            ENV["PAY3FLOW_K8S_TEMPLATE"] ||= "${applicationTemplate}"
            ENV["PAY3FLOW_K8S_SERVICES_TEMPLATE"] ||= "${servicesTemplate}"
            load "${./deploy/render.rb}"
            load "${./deploy/deploy.rb}"
            Pay3flow::Deploy.main(ARGV)
          '';
          deployLiveScript = pkgs.writeScriptBin "deploy-pay3flow-live" ''
            #!${pkgs.ruby}/bin/ruby
            ENV["PATH"] = "${pkgs.lib.makeBinPath (with pkgs; [ curl git openssh rsync ])}:#{ENV.fetch("PATH", "")}"
            load "${./deploy/live_deploy.rb}"
            Pay3flow::LiveDeploy.main(ARGV)
          '';
          manifests = pkgs.runCommand "pay3flow-kubernetes.yaml" {
            nativeBuildInputs = [ pkgs.ruby ];
          } ''
            ${pkgs.ruby}/bin/ruby ${./deploy/render.rb} \
              --template ${applicationTemplate} \
              --services-template ${servicesTemplate} \
              --output "$out"
          '';
        in {
          kubernetes-manifests = manifests;
          render-manifests = renderScript;
          deploy = deployLiveScript;
          deploy-k8s = deployK8sScript;
          default = manifests;
        });

      apps = forEachSystem ({ pkgs }:
        let
          render = self.packages.${pkgs.system}.render-manifests;
          deploy = self.packages.${pkgs.system}.deploy;
          deploy-k8s = self.packages.${pkgs.system}.deploy-k8s;
        in {
          render-manifests = {
            type = "app";
            program = "${render}/bin/render-pay3flow-k8s";
          };
          deploy = {
            type = "app";
            program = "${deploy}/bin/deploy-pay3flow-live";
          };
          deploy-k8s = {
            type = "app";
            program = "${deploy-k8s}/bin/deploy-pay3flow-k8s";
          };
          default = self.apps.${pkgs.system}.render-manifests;
        });

      devShells = forEachSystem ({ pkgs }:
        {
          default = pkgs.mkShell {
            packages = [
              pkgs.cargo
              pkgs.clippy
              pkgs.rustc
              pkgs.rustfmt
            ];
          };
        });
    };
}
