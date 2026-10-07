#!/usr/bin/env fish

function die
    echo $argv >&2
    exit 1
end

function usage
    echo 'Deploy the current Pay3Flow commit to the production k3s host.'
    echo ''
    echo 'Usage:'
    echo '  nix run .#deploy'
    echo ''
    echo 'Optional environment variables:'
    echo '  PAY3FLOW_DEPLOY_HOST           SSH host (default: 82.118.18.183)'
    echo '  PAY3FLOW_DEPLOY_USER           SSH user (default: xila)'
    echo '  PAY3FLOW_DEPLOY_IDENTITY_FILE  SSH private key (default: SSH configuration)'
    echo '  PAY3FLOW_NAMESPACE             Kubernetes namespace (default: pay3flow)'
    echo '  PAY3FLOW_PUBLIC_URL            URL verified after rollout'
    echo '  PAY3FLOW_IMAGE_TAG             Override the commit-derived image tag'
    echo ''
    echo 'The cluster, Caddy route, pay3flow-secrets Secret, PostgreSQL, and Redis'
    echo 'must already be bootstrapped. backend/.env is never copied to the server.'
end

if test (count $argv) -gt 0
    switch $argv[1]
        case --help -h
            test (count $argv) -eq 1; or die 'Unexpected arguments.'
            usage
            exit 0
        case '*'
            usage >&2
            exit 1
    end
end

set repo_root (git rev-parse --show-toplevel 2>/dev/null | string collect)
or die 'This command must run inside the Pay3Flow git worktree.'

# Allow the deployment implementation and its flake entry point to be changed
# locally while still refusing to deploy unrelated application changes.
set dirty (git -C $repo_root status --porcelain --untracked-files=all -- \
    . ':(exclude)deploy/live_deploy.fish' ':(exclude)deploy/live_deploy.rb' \
    ':(exclude)deploy/README.md' ':(exclude)flake.nix' | string collect)
test -z "$dirty"; or die "Refusing to deploy a dirty application worktree:\n$dirty"

set revision (git -C $repo_root rev-parse HEAD | string trim)
set short_revision (string sub --start 1 --length 7 -- $revision)
set image_tag $PAY3FLOW_IMAGE_TAG
if test -z "$image_tag"
    set image_tag live-(date -u +%Y%m%d)-$short_revision
end
set deploy_host $PAY3FLOW_DEPLOY_HOST
test -n "$deploy_host"; or set deploy_host 82.118.18.183
set deploy_user $PAY3FLOW_DEPLOY_USER
test -n "$deploy_user"; or set deploy_user xila
set namespace $PAY3FLOW_NAMESPACE
test -n "$namespace"; or set namespace pay3flow
set public_url $PAY3FLOW_PUBLIC_URL
test -n "$public_url"; or set public_url https://pay3flow.lefine.pro
set public_url (string replace --regex '/+$' '' -- $public_url | string collect)
set release_name pay3flow-release-$short_revision
set remote $deploy_user@$deploy_host

string match -rq '^[A-Za-z0-9._-]+$' -- $image_tag; or die "Invalid PAY3FLOW_IMAGE_TAG: $image_tag"
string match -rq '^[a-z0-9](?:[-a-z0-9]*[a-z0-9])?$' -- $namespace; or die "Invalid PAY3FLOW_NAMESPACE: $namespace"

set ssh_options -o BatchMode=yes -o StrictHostKeyChecking=yes
if test -n "$PAY3FLOW_DEPLOY_IDENTITY_FILE"
    set ssh_options $ssh_options -i $PAY3FLOW_DEPLOY_IDENTITY_FILE
end

function remote
    ssh $ssh_options $remote $argv
end

set kubectl /usr/local/bin/kubectl
set k3s /usr/local/bin/k3s
set podman /usr/bin/podman
set backend_image localhost/pay3flow/backend:$image_tag
set frontend_image localhost/pay3flow/frontend:$image_tag
set remote_home (remote 'printf %s "$HOME"' | string collect)
test -n "$remote_home"; or die 'Could not determine the remote home directory.'
set release_dir "$remote_home/$release_name"
set backend_log /tmp/pay3flow-backend-$short_revision.log
set frontend_log /tmp/pay3flow-frontend-$short_revision.log

echo "Deploying Pay3Flow revision $revision with image tag $image_tag"
echo "Target: $remote, namespace: $namespace"

for executable in $kubectl $k3s $podman
    remote "test -x '$executable'"; or die "Required executable is missing on the server: $executable"
end
remote 'sudo -n true'; or die 'Passwordless sudo is required on the deployment host.'
remote "sudo '$kubectl' get namespace '$namespace' >/dev/null"; or die "Kubernetes namespace is unavailable: $namespace"
remote "sudo '$kubectl' -n '$namespace' get deployment/pay3flow-backend deployment/pay3flow-frontend configmap/pay3flow-config secret/pay3flow-secrets >/dev/null"; or die 'Pay3Flow Kubernetes resources are not bootstrapped.'

for key in DATABASE_URL REDIS_URL JWT_SECRET SECRETS_KEY ADMIN_TOKEN NEAR_INTENTS_JWT BESTCHANGE_API_KEY
    remote "sudo '$kubectl' -n '$namespace' get secret pay3flow-secrets -o jsonpath='{.data.$key}' | grep -q ."; or die "Secret pay3flow-secrets is missing key: $key"
end

remote "test -d $release_dir || mkdir -m 0750 $release_dir"; or die 'Could not prepare the remote release directory.'
printf '%s\n' $revision | remote "cat > $release_dir/.source-revision"; or die 'Could not write the source revision marker.'

set rsync_ssh (string join -- ' ' ssh $ssh_options)
rsync -az --delete \
    --exclude .git/ \
    --exclude backend/.env \
    --exclude node_modules/ \
    --exclude target/ \
    -e "$rsync_ssh" \
    "$repo_root/" "$remote:$release_dir/"; or die 'Could not synchronize the release source.'

set backend_log_command "cd $release_dir && '$podman' build --tag '$backend_image' --file backend/Dockerfile . >'$backend_log' 2>&1"
set frontend_log_command "cd $release_dir && '$podman' build --tag '$frontend_image' --file pay3low-svelte-frontend/Dockerfile pay3low-svelte-frontend >'$frontend_log' 2>&1"
remote "rm -f '$backend_log' '$frontend_log'"; or die 'Could not clear remote build logs.'
echo 'Building backend and frontend images in parallel...'
remote $backend_log_command &
set backend_pid $last_pid
remote $frontend_log_command &
set frontend_pid $last_pid
wait $backend_pid
set backend_status $status
wait $frontend_pid
set frontend_status $status
if test $backend_status -ne 0 -o $frontend_status -ne 0
    echo 'Backend build log:' >&2
    remote "tail -80 '$backend_log'" >&2
    echo 'Frontend build log:' >&2
    remote "tail -80 '$frontend_log'" >&2
    die 'Image build failed.'
end
remote "tail -8 '$backend_log'"
remote "tail -8 '$frontend_log'"

echo 'Importing images into k3s...'
set backend_archive /tmp/pay3flow-backend-image-$short_revision.tar
set frontend_archive /tmp/pay3flow-frontend-image-$short_revision.tar
remote "rm -f '$backend_archive' '$frontend_archive'"; or die 'Could not clear image archives.'
remote "'$podman' save --output '$backend_archive' '$backend_image' && '$podman' save --output '$frontend_archive' '$frontend_image'"; or die 'Could not save built images.'
remote "sudo '$k3s' ctr images import '$backend_archive' >/dev/null && sudo '$k3s' ctr images import '$frontend_archive' >/dev/null"; or die 'Could not import images into k3s.'
remote "rm -f '$backend_archive' '$frontend_archive'"

set source_backend_id (remote "'$podman' image inspect --format '{{.Id}}' '$backend_image'" | string replace --regex '^sha256:' '' | string trim)
set source_frontend_id (remote "'$podman' image inspect --format '{{.Id}}' '$frontend_image'" | string replace --regex '^sha256:' '' | string trim)
set runtime_backend_id (remote "sudo '$k3s' crictl images | grep -E '^localhost/pay3flow/backend[[:space:]]+$image_tag' | head -1 | tr -s ' ' | cut -d ' ' -f3" | string trim)
set runtime_frontend_id (remote "sudo '$k3s' crictl images | grep -E '^localhost/pay3flow/frontend[[:space:]]+$image_tag' | head -1 | tr -s ' ' | cut -d ' ' -f3" | string trim)
test (string sub --start 1 --length 12 -- $source_backend_id) = (string sub --start 1 --length 12 -- $runtime_backend_id); or die 'Imported backend image ID does not match the build output.'
test (string sub --start 1 --length 12 -- $source_frontend_id) = (string sub --start 1 --length 12 -- $runtime_frontend_id); or die 'Imported frontend image ID does not match the build output.'
test (string sub --start 1 --length 12 -- $runtime_backend_id) != (string sub --start 1 --length 12 -- $runtime_frontend_id); or die 'Backend and frontend unexpectedly resolve to the same image ID.'

set old_backend_image (remote "sudo '$kubectl' -n '$namespace' get deployment pay3flow-backend -o jsonpath='{.spec.template.spec.containers[0].image}'" | string trim)
set old_frontend_image (remote "sudo '$kubectl' -n '$namespace' get deployment pay3flow-frontend -o jsonpath='{.spec.template.spec.containers[0].image}'" | string trim)
set rollout_started 0

function rollback_images
    if test $rollout_started -eq 1
        echo 'Rollout failed; restoring the previous images.' >&2
        remote "sudo '$kubectl' -n '$namespace' set image deployment/pay3flow-backend backend='$old_backend_image'" >/dev/null 2>&1; or true
        remote "sudo '$kubectl' -n '$namespace' set image deployment/pay3flow-frontend frontend='$old_frontend_image'" >/dev/null 2>&1; or true
        remote "sudo '$kubectl' -n '$namespace' rollout status --timeout=5m deployment/pay3flow-backend" >/dev/null 2>&1; or true
        remote "sudo '$kubectl' -n '$namespace' rollout status --timeout=5m deployment/pay3flow-frontend" >/dev/null 2>&1; or true
    end
end

set deployed_at (date -u +%Y-%m-%dT%H:%M:%SZ)
set rollout_started 1
set backend_patch "{\"spec\":{\"template\":{\"metadata\":{\"annotations\":{\"pay3flow.io/deployed-at\":\"$deployed_at\",\"pay3flow.io/source-revision\":\"$revision\"}},\"spec\":{\"containers\":[{\"name\":\"backend\",\"image\":\"$backend_image\",\"imagePullPolicy\":\"IfNotPresent\",\"env\":[{\"name\":\"BESTCHANGE_API_KEY\",\"valueFrom\":{\"secretKeyRef\":{\"name\":\"pay3flow-secrets\",\"key\":\"BESTCHANGE_API_KEY\"}}},{\"name\":\"SYMBIOSIS_PARTNER_ID\",\"valueFrom\":{\"secretKeyRef\":{\"name\":\"pay3flow-secrets\",\"key\":\"SYMBIOSIS_PARTNER_ID\"}}}] }]}}}}"
# Resolve public API names before Kubernetes search suffixes, and use separate
# sockets for A/AAAA lookups to avoid five-second resolver stalls.
set backend_patch (string replace '"containers":' '"dnsConfig":{"options":[{"name":"ndots","value":"1"},{"name":"single-request-reopen"}]},"containers":' -- $backend_patch)
printf '%s' $backend_patch | remote "sudo '$kubectl' -n '$namespace' patch deployment pay3flow-backend --type=strategic --patch-file=/dev/stdin"; or begin
    rollback_images
    exit 1
end

set frontend_patch "{\"spec\":{\"template\":{\"metadata\":{\"annotations\":{\"pay3flow.io/deployed-at\":\"$deployed_at\",\"pay3flow.io/source-revision\":\"$revision\"}},\"spec\":{\"containers\":[{\"name\":\"frontend\",\"image\":\"$frontend_image\",\"imagePullPolicy\":\"IfNotPresent\"}]}}}}"
set frontend_patch (string replace '"containers":' '"dnsConfig":{"options":[{"name":"ndots","value":"1"},{"name":"single-request-reopen"}]},"containers":' -- $frontend_patch)
printf '%s' $frontend_patch | remote "sudo '$kubectl' -n '$namespace' patch deployment pay3flow-frontend --type=strategic --patch-file=/dev/stdin"; or begin
    rollback_images
    exit 1
end
set config_patch "{\"data\":{\"IMAGE_TAG\":\"$image_tag\",\"SOURCE_REVISION\":\"$revision\"}}"
printf '%s' $config_patch | remote "sudo '$kubectl' -n '$namespace' patch configmap pay3flow-config --type=merge --patch-file=/dev/stdin" >/dev/null; or begin
    rollback_images
    exit 1
end

remote "sudo '$kubectl' -n '$namespace' rollout status --timeout=10m deployment/pay3flow-backend"; or begin
    rollback_images
    exit 1
end
remote "sudo '$kubectl' -n '$namespace' rollout status --timeout=10m deployment/pay3flow-frontend"; or begin
    rollback_images
    exit 1
end
set rollout_started 0
remote "sudo '$kubectl' -n '$namespace' get deployment pay3flow-backend pay3flow-frontend -o custom-columns=NAME:.metadata.name,READY:.status.readyReplicas,IMAGE:.spec.template.spec.containers[0].image"

echo 'Checking the public route...'
curl --fail --silent --show-error --max-time 30 "$public_url/" >/dev/null; or die 'Public root route check failed.'
set health (curl --fail --silent --show-error --max-time 30 "$public_url/health" | string trim)
test "$health" = ok; or die "Health check returned unexpected content: $health"
set providers (curl --fail --silent --show-error --max-time 30 "$public_url/api/providers" | string lower | string collect); or die 'Provider route check failed.'
string match -q '*"bestchange"*' -- $providers; or die 'Provider response does not include bestchange.'
string match -q '*"symbiosis"*' -- $providers; or die 'Provider response does not include symbiosis.'

echo "Pay3Flow deployment completed: $image_tag"
