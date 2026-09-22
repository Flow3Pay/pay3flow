{
  description = "Pay3Flow k3s deployment and in-cluster image builds";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forEachSystem = f: nixpkgs.lib.genAttrs systems (system:
        f {
          pkgs = import nixpkgs { inherit system; };
        });
    in {
      packages = forEachSystem ({ pkgs }:
        let
          template = ./deploy/kubernetes.yaml.in;

          renderScript = pkgs.writeShellApplication {
            name = "render-pay3flow-k8s";
            runtimeInputs = [ pkgs.coreutils pkgs.gettext ];
            text = ''
              set -eu

              : "''${PAY3FLOW_NAMESPACE:=pay3flow}"
              : "''${PAY3FLOW_IMAGE_REGISTRY:=registry.example.invalid/pay3flow}"
              : "''${PAY3FLOW_IMAGE_TAG:=dev}"
              : "''${PAY3FLOW_SOURCE_REPOSITORY:=https://github.com/Flow3Pay/pay3flow.git}"
              : "''${PAY3FLOW_SOURCE_REVISION:=c3ce63cc06209638e47bcf9c94e65ad66f802da9}"
              : "''${PAY3FLOW_INGRESS_HOST:=pay3flow.example.invalid}"
              : "''${PAY3FLOW_INGRESS_CLASS:=traefik}"
              : "''${PAY3FLOW_INGRESS_TLS_SECRET:=}"
              : "''${PAY3FLOW_STORAGE_CLASS:=local-path}"
              : "''${PAY3FLOW_DATABASE_MODE:=in-cluster}"
              : "''${PAY3FLOW_DATABASE_URL:=}"
              : "''${PAY3FLOW_REDIS_URL:=}"
              : "''${PAY3FLOW_POSTGRES_PASSWORD:=change-me-postgres}"
              : "''${PAY3FLOW_JWT_SECRET:=change-me-jwt}"
              : "''${PAY3FLOW_SECRETS_KEY:=change-me-secrets}"
              : "''${PAY3FLOW_ADMIN_TOKEN:=change-me-admin}"
              : "''${PAY3FLOW_AP_ORIGIN:=https://$PAY3FLOW_INGRESS_HOST}"
              : "''${PAY3FLOW_AP_HANDLE:=pay3flow}"
              : "''${PAY3FLOW_AP_REQUIRE_SIGNATURES:=false}"
              : "''${PAY3FLOW_FMATCH_INBOX:=http://fmatch:7277/inbox/actra}"
              : "''${PAY3FLOW_FMATCH_ACTOR_ID:=http://fmatch:7277/actor/actra}"
              : "''${PAY3FLOW_PUBLIC_API_URL:=}"
              : "''${PAY3FLOW_FX_SOURCE:=mock}"
              : "''${PAY3FLOW_P2P_SEARCH_ENABLED:=true}"
              : "''${PAY3FLOW_P2P_BINANCE_ENABLED:=false}"
              : "''${PAY3FLOW_P2P_BYBIT_ENABLED:=false}"
              : "''${PAY3FLOW_P2P_OKX_ENABLED:=false}"
              : "''${PAY3FLOW_P2P_BITGET_ENABLED:=false}"
              : "''${PAY3FLOW_P2P_RAPIRA_ENABLED:=false}"

              if [ "$PAY3FLOW_DATABASE_MODE" = external ] && [ -z "$PAY3FLOW_DATABASE_URL" ]; then
                echo "PAY3FLOW_DATABASE_URL is required when PAY3FLOW_DATABASE_MODE=external" >&2
                exit 1
              fi
              if [ "$PAY3FLOW_DATABASE_MODE" = external ]; then
                PAY3FLOW_STATEFUL_REPLICAS=0
              else
                PAY3FLOW_STATEFUL_REPLICAS=1
              fi
              if [ -z "$PAY3FLOW_DATABASE_URL" ]; then
                PAY3FLOW_DATABASE_URL="postgres://pay3flow:$PAY3FLOW_POSTGRES_PASSWORD@pay3flow-postgres:5432/pay3flow"
              fi
              if [ "$PAY3FLOW_DATABASE_MODE" = external ] && [ -z "$PAY3FLOW_REDIS_URL" ]; then
                echo "PAY3FLOW_REDIS_URL is required when PAY3FLOW_DATABASE_MODE=external" >&2
                exit 1
              fi
              if [ -z "$PAY3FLOW_REDIS_URL" ]; then
                PAY3FLOW_REDIS_URL="redis://pay3flow-redis:6379"
              fi
              export PAY3FLOW_NAMESPACE PAY3FLOW_IMAGE_REGISTRY PAY3FLOW_IMAGE_TAG
              export PAY3FLOW_SOURCE_REPOSITORY PAY3FLOW_SOURCE_REVISION
              export PAY3FLOW_INGRESS_HOST PAY3FLOW_INGRESS_CLASS PAY3FLOW_INGRESS_TLS_SECRET
              export PAY3FLOW_STORAGE_CLASS PAY3FLOW_DATABASE_MODE PAY3FLOW_DATABASE_URL PAY3FLOW_REDIS_URL
              export PAY3FLOW_STATEFUL_REPLICAS
              export PAY3FLOW_POSTGRES_PASSWORD PAY3FLOW_JWT_SECRET PAY3FLOW_SECRETS_KEY PAY3FLOW_ADMIN_TOKEN
              export PAY3FLOW_AP_ORIGIN PAY3FLOW_AP_HANDLE PAY3FLOW_AP_REQUIRE_SIGNATURES
              export PAY3FLOW_FMATCH_INBOX PAY3FLOW_FMATCH_ACTOR_ID PAY3FLOW_PUBLIC_API_URL
              export PAY3FLOW_FX_SOURCE PAY3FLOW_P2P_SEARCH_ENABLED PAY3FLOW_P2P_BINANCE_ENABLED
              export PAY3FLOW_P2P_BYBIT_ENABLED PAY3FLOW_P2P_OKX_ENABLED PAY3FLOW_P2P_BITGET_ENABLED
              export PAY3FLOW_P2P_RAPIRA_ENABLED

              output=
              if [ "''${1:-}" = "--output" ]; then
                if [ "$#" -ne 2 ]; then
                  echo "usage: render-pay3flow-k8s [--output FILE]" >&2
                  exit 2
                fi
                output=$2
              elif [ "$#" -ne 0 ]; then
                echo "usage: render-pay3flow-k8s [--output FILE]" >&2
                exit 2
              fi

              if [ -n "$PAY3FLOW_INGRESS_TLS_SECRET" ]; then
                PAY3FLOW_INGRESS_TLS_BLOCK=$(printf '  tls:\n    - hosts:\n        - %s\n      secretName: %s' "$PAY3FLOW_INGRESS_HOST" "$PAY3FLOW_INGRESS_TLS_SECRET")
              else
                PAY3FLOW_INGRESS_TLS_BLOCK=
              fi
              export PAY3FLOW_INGRESS_TLS_BLOCK

              # shellcheck disable=SC2016
              variables='$PAY3FLOW_NAMESPACE $PAY3FLOW_IMAGE_REGISTRY $PAY3FLOW_IMAGE_TAG $PAY3FLOW_SOURCE_REPOSITORY $PAY3FLOW_SOURCE_REVISION $PAY3FLOW_INGRESS_HOST $PAY3FLOW_INGRESS_CLASS $PAY3FLOW_INGRESS_TLS_BLOCK $PAY3FLOW_STORAGE_CLASS $PAY3FLOW_DATABASE_MODE $PAY3FLOW_DATABASE_URL $PAY3FLOW_REDIS_URL $PAY3FLOW_STATEFUL_REPLICAS $PAY3FLOW_POSTGRES_PASSWORD $PAY3FLOW_JWT_SECRET $PAY3FLOW_SECRETS_KEY $PAY3FLOW_ADMIN_TOKEN $PAY3FLOW_AP_ORIGIN $PAY3FLOW_AP_HANDLE $PAY3FLOW_AP_REQUIRE_SIGNATURES $PAY3FLOW_FMATCH_INBOX $PAY3FLOW_FMATCH_ACTOR_ID $PAY3FLOW_PUBLIC_API_URL $PAY3FLOW_FX_SOURCE $PAY3FLOW_P2P_SEARCH_ENABLED $PAY3FLOW_P2P_BINANCE_ENABLED $PAY3FLOW_P2P_BYBIT_ENABLED $PAY3FLOW_P2P_OKX_ENABLED $PAY3FLOW_P2P_BITGET_ENABLED $PAY3FLOW_P2P_RAPIRA_ENABLED'
              if [ -n "$output" ]; then
                envsubst "$variables" < ${template} > "$output"
              else
                envsubst "$variables" < ${template}
              fi
            '';
          };

          deployScript = pkgs.writeShellApplication {
            name = "deploy-pay3flow-k8s";
            runtimeInputs = [ pkgs.coreutils pkgs.kubectl ];
            text = ''
              set -eu
              workdir=$(mktemp -d)
              trap 'rm -rf "$workdir"' EXIT

              namespace="''${PAY3FLOW_NAMESPACE:-pay3flow}"
              kubectl create namespace "$namespace" --dry-run=client -o yaml | kubectl apply -f -
              kubectl -n "$namespace" get secret pay3flow-registry >/dev/null || {
                echo "missing $namespace/pay3flow-registry; create the registry credential first" >&2
                exit 1
              }
              kubectl -n "$namespace" delete job --ignore-not-found \
                pay3flow-build-backend pay3flow-build-frontend
              ${renderScript}/bin/render-pay3flow-k8s --output "$workdir/pay3flow.yaml"
              kubectl apply -f "$workdir/pay3flow.yaml"

              echo "Waiting for Kaniko image-build Jobs..."
              kubectl -n "$namespace" wait --for=condition=complete \
                --timeout=30m job/pay3flow-build-backend job/pay3flow-build-frontend
              kubectl -n "$namespace" rollout status --timeout=10m \
                deployment/pay3flow-backend deployment/pay3flow-frontend
            '';
          };
        in {
          kubernetes-manifests = pkgs.runCommand "pay3flow-kubernetes.yaml" {
            nativeBuildInputs = [ pkgs.gettext ];
          } ''
            export PAY3FLOW_NAMESPACE=pay3flow
            export PAY3FLOW_IMAGE_REGISTRY=registry.example.invalid/pay3flow
            export PAY3FLOW_IMAGE_TAG=dev
            export PAY3FLOW_SOURCE_REPOSITORY=https://github.com/Flow3Pay/pay3flow.git
            export PAY3FLOW_SOURCE_REVISION=c3ce63cc06209638e47bcf9c94e65ad66f802da9
            export PAY3FLOW_INGRESS_HOST=pay3flow.example.invalid
            export PAY3FLOW_INGRESS_CLASS=traefik
            export PAY3FLOW_INGRESS_TLS_SECRET=
            export PAY3FLOW_STORAGE_CLASS=local-path
            export PAY3FLOW_DATABASE_MODE=in-cluster
            export PAY3FLOW_STATEFUL_REPLICAS=1
            export PAY3FLOW_DATABASE_URL=
            export PAY3FLOW_REDIS_URL=
            export PAY3FLOW_POSTGRES_PASSWORD=change-me-postgres
            export PAY3FLOW_JWT_SECRET=change-me-jwt
            export PAY3FLOW_SECRETS_KEY=change-me-secrets
            export PAY3FLOW_ADMIN_TOKEN=change-me-admin
            export PAY3FLOW_AP_ORIGIN=https://pay3flow.example.invalid
            export PAY3FLOW_AP_HANDLE=pay3flow
            export PAY3FLOW_AP_REQUIRE_SIGNATURES=false
            export PAY3FLOW_FMATCH_INBOX=http://fmatch:7277/inbox/actra
            export PAY3FLOW_FMATCH_ACTOR_ID=http://fmatch:7277/actor/actra
            export PAY3FLOW_PUBLIC_API_URL=
            export PAY3FLOW_FX_SOURCE=mock
            export PAY3FLOW_P2P_SEARCH_ENABLED=true
            export PAY3FLOW_P2P_BINANCE_ENABLED=false
            export PAY3FLOW_P2P_BYBIT_ENABLED=false
            export PAY3FLOW_P2P_OKX_ENABLED=false
            export PAY3FLOW_P2P_BITGET_ENABLED=false
            export PAY3FLOW_P2P_RAPIRA_ENABLED=false
            export PAY3FLOW_DATABASE_URL=postgres://pay3flow:$PAY3FLOW_POSTGRES_PASSWORD@pay3flow-postgres:5432/pay3flow
            export PAY3FLOW_REDIS_URL=redis://pay3flow-redis:6379
            export PAY3FLOW_INGRESS_TLS_BLOCK=
            envsubst < ${template} > $out
          '';

          render-manifests = renderScript;
          deploy = deployScript;
          default = self.packages.${pkgs.system}.kubernetes-manifests;
        });

      apps = forEachSystem ({ pkgs }:
        let
          render = self.packages.${pkgs.system}.render-manifests;
          deploy = self.packages.${pkgs.system}.deploy;
        in {
          render-manifests = {
            type = "app";
            program = "${render}/bin/render-pay3flow-k8s";
          };
          deploy = {
            type = "app";
            program = "${deploy}/bin/deploy-pay3flow-k8s";
          };
          default = self.apps.${pkgs.system}.render-manifests;
        });
    };
}
