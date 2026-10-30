{ pkgs }:
let
  verifyHttp = ''
    verify_http() {
      curl --fail --silent --show-error --retry 3 --retry-delay 3 --max-time 30 "$public_url/health/ready" >/dev/null
      for currency in RUB AMD BYN UAH KZT; do
        curl --fail --silent --show-error --retry 3 --retry-delay 3 --max-time 30 \
          "$public_url/api/reference-rates?from=USD&to=$currency&market=noncash" \
          | jq -e '.rates | length > 0' >/dev/null
      done
      curl --fail --silent --show-error --max-time 30 \
        "$public_url/api/reference-rates?from=RUB&to=AMD&market=noncash" \
        | jq -e '(.rates | length == 2) and (.unavailable_sources | length == 0)' >/dev/null
      curl --fail --silent --show-error --max-time 30 "$public_url/" >/dev/null
      echo "Verified $public_url: readiness, fiat benchmarks and frontend"
    }
  '';
  common = ''
    host="''${PAY3FLOW_DEPLOY_HOST:-195.123.218.200}"
    user="''${PAY3FLOW_DEPLOY_USER:-root}"
    namespace="''${PAY3FLOW_NAMESPACE:-pay3flow}"
    public_url="''${PAY3FLOW_PUBLIC_URL:-https://pay3flow.lefine.pro}"
    image_tag="''${PAY3FLOW_IMAGE_TAG:-}"
    revision="''${PAY3FLOW_SOURCE_REVISION:-}"
    reuse_images=false
    usage() {
      echo "Usage: nix run .#deploy -- [--reuse-images] [--revision COMMIT] [--image-tag TAG]"
      echo "       nix run .#verify"
      echo "       nix run .#rollback -- --image-tag TAG"
      echo "SSH uses the normal agent/key configuration. Container transport:"
      echo "  PAY3FLOW_SSH_CONTAINER=IMAGE PAY3FLOW_SSH_KEY_DIRECTORY=/path/to/.ssh"
    }
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --reuse-images) reuse_images=true; shift ;;
        --revision) revision="$2"; shift 2 ;;
        --image-tag) image_tag="$2"; shift 2 ;;
        --help|-h) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
      esac
    done
    [[ "$host" =~ ^[A-Za-z0-9.:-]+$ && "$user" =~ ^[a-z_][a-z0-9_-]*$ ]]
    [[ "$namespace" =~ ^[a-z0-9-]+$ && "$public_url" =~ ^https://[A-Za-z0-9.:-]+$ ]]
    [[ -z "$revision" || "$revision" =~ ^[a-f0-9]{7,40}$ ]]
    [[ -z "$image_tag" || "$image_tag" =~ ^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$ ]]
    remote() {
      if [[ -n "''${PAY3FLOW_SSH_CONTAINER:-}" ]]; then
        local key_directory="''${PAY3FLOW_SSH_KEY_DIRECTORY:?Set the SSH key directory}"
        docker run --rm -i --user 0 --entrypoint ssh --network host \
          -v "$key_directory:/deployment-ssh:ro" "$PAY3FLOW_SSH_CONTAINER" \
          -i /deployment-ssh/id_ed25519 -o BatchMode=yes -o ConnectTimeout=10 \
          -o UserKnownHostsFile=/deployment-ssh/known_hosts -o StrictHostKeyChecking=yes \
          "$user@$host" "$@"
      else
        ssh -o BatchMode=yes -o ConnectTimeout=10 "$user@$host" "$@"
      fi
    }
    ${verifyHttp}
  '';
  make = action: body: pkgs.writeShellApplication {
    name = "pay3flow-${action}";
    runtimeInputs = with pkgs; [ coreutils curl docker-client git gnutar jq openssh ];
    excludeShellChecks = [ "SC2034" ];
    text = common + body;
  };
in {
  deploy = make "deploy" ''
    repo=$(git rev-parse --show-toplevel)
    revision=$(git -C "$repo" rev-parse "''${revision:-HEAD}")
    [[ "$revision" =~ ^[a-f0-9]{40}$ ]]
    [[ -z "$(git -C "$repo" status --porcelain --untracked-files=no)" ]] || {
      echo "Commit tracked changes before deploying." >&2; exit 1;
    }
    # Deployment documentation can advance after a build; application source must match.
    git -C "$repo" diff --quiet "$revision" HEAD -- backend pay3low-svelte-frontend config.toml
    image_tag="''${image_tag:-live-$(date -u +%Y%m%d)-''${revision:0:7}}"
    release="/root/pay3flow-releases/$revision"
    if ! "$reuse_images"; then
      remote "mkdir -p '$release'"
      git -C "$repo" archive "$revision" | remote "tar -xf - -C '$release'"
      remote "cd '$release' && nerdctl --address /run/k3s/containerd/containerd.sock --namespace k8s.io build --progress plain -f backend/Dockerfile -t 'localhost/pay3flow/backend:$image_tag' ."
      remote "cd '$release/pay3low-svelte-frontend' && nerdctl --address /run/k3s/containerd/containerd.sock --namespace k8s.io build --progress plain -f '../deploy/frontend-runtime/Dockerfile' -t 'localhost/pay3flow/frontend:$image_tag' ."
    fi
    remote "namespace='$namespace' public_url='$public_url' image_tag='$image_tag' revision='$revision' bash -s" <<'REMOTE'
    set -euo pipefail
    umask 077
    ${verifyHttp}
    kube() { k3s kubectl --request-timeout=30s -n "$namespace" "$@"; }
    for service in backend frontend; do
      nerdctl --address /run/k3s/containerd/containerd.sock --namespace k8s.io image inspect "localhost/pay3flow/$service:$image_tag" >/dev/null
    done
    backup="/var/lib/pay3flow/backups/$image_tag"
    test ! -e "$backup/backend-before.json" || { echo "Backup already exists: $backup" >&2; exit 1; }
    install -d -m 700 "$backup"
    for service in backend frontend; do kube get deployment "pay3flow-$service" -o json > "$backup/$service-before.json"; done
    kube get configmap pay3flow-config -o json > "$backup/config-before.json"
    restore() {
      status=$?
      if [[ "$status" -ne 0 ]]; then
        echo "Rollout failed; restoring both previous deployment specifications." >&2
        for service in backend frontend; do
          jq '{spec:.spec}' "$backup/$service-before.json" > "$backup/$service-rollback.json"
          kube patch deployment "pay3flow-$service" --type=merge --patch-file "$backup/$service-rollback.json" || true
        done
        jq '{data:.data}' "$backup/config-before.json" > "$backup/config-rollback.json"
        kube patch configmap pay3flow-config --type=merge --patch-file "$backup/config-rollback.json" || true
        for service in backend frontend; do kube rollout status "deployment/pay3flow-$service" --timeout=5m || true; done
      fi
      exit "$status"
    }
    trap restore EXIT
    for service in backend frontend; do
      patch=$(jq -n --arg service "$service" --arg tag "$image_tag" --arg revision "$revision" \
        '{spec:{template:{metadata:{annotations:{"pay3flow.io/source-revision":$revision}},spec:{containers:[{name:$service,image:("localhost/pay3flow/"+$service+":"+$tag),imagePullPolicy:"IfNotPresent"}]}}}}')
      kube patch deployment "pay3flow-$service" --type=strategic -p "$patch"
      kube rollout status "deployment/pay3flow-$service" --timeout=8m
      curl --fail --silent --show-error --retry 3 --retry-delay 3 --max-time 30 "$public_url/health/ready" >/dev/null
    done
    verify_http
    patch=$(jq -n --arg tag "$image_tag" --arg revision "$revision" '{data:{IMAGE_TAG:$tag,SOURCE_REVISION:$revision}}')
    kube patch configmap pay3flow-config --type=merge -p "$patch"
    kube get deployments -o custom-columns=NAME:.metadata.name,READY:.status.readyReplicas,IMAGE:.spec.template.spec.containers[0].image
    trap - EXIT
    echo "Rollback snapshot: $backup"
    REMOTE
  '';
  verify = make "verify" ''
    verify_http
    remote "k3s kubectl --request-timeout=30s -n '$namespace' get deployments -o custom-columns=NAME:.metadata.name,READY:.status.readyReplicas,IMAGE:.spec.template.spec.containers[0].image"
  '';
  rollback = make "rollback" ''
    [[ -n "$image_tag" ]] || { echo "--image-tag is required for rollback." >&2; exit 2; }
    remote "namespace='$namespace' image_tag='$image_tag' bash -s" <<'REMOTE'
    set -euo pipefail
    kube() { k3s kubectl --request-timeout=30s -n "$namespace" "$@"; }
    backup="/var/lib/pay3flow/backups/$image_tag"
    for service in backend frontend; do test -s "$backup/$service-before.json"; done
    for service in backend frontend; do
      jq '{spec:.spec}' "$backup/$service-before.json" > "$backup/$service-rollback.json"
      kube patch deployment "pay3flow-$service" --type=merge --patch-file "$backup/$service-rollback.json"
    done
    if test -s "$backup/config-before.json"; then
      jq '{data:.data}' "$backup/config-before.json" > "$backup/config-rollback.json"
      kube patch configmap pay3flow-config --type=merge --patch-file "$backup/config-rollback.json"
    fi
    for service in backend frontend; do kube rollout status "deployment/pay3flow-$service" --timeout=5m; done
    REMOTE
  '';
}
