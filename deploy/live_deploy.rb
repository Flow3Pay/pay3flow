#!/usr/bin/env ruby

require "open3"
require "shellwords"
require "time"

module Pay3flow
  module LiveDeploy
    module_function

    HELP = <<~TEXT.freeze
      Deploy the current Pay3Flow commit to the production k3s host.

      Usage:
        nix run .#deploy

      Optional environment variables:
        PAY3FLOW_DEPLOY_HOST           SSH host (default: 82.118.18.183)
        PAY3FLOW_DEPLOY_USER           SSH user (default: xila)
        PAY3FLOW_DEPLOY_IDENTITY_FILE  SSH private key (default: SSH configuration)
        PAY3FLOW_NAMESPACE             Kubernetes namespace (default: pay3flow)
        PAY3FLOW_PUBLIC_URL            URL verified after rollout
        PAY3FLOW_IMAGE_TAG             Override the commit-derived image tag

      The cluster, Caddy route, pay3flow-secrets Secret, PostgreSQL, and Redis
      must already be bootstrapped. backend/.env is never copied to the server.
    TEXT

    REMOTE_SCRIPT = <<~'BASH'.freeze
      set -Eeuo pipefail

      image_tag=$1
      revision=$2
      namespace=$3
      release_name=$4
      release_dir="$HOME/$release_name"
      kubectl=/usr/local/bin/kubectl
      k3s=/usr/local/bin/k3s
      podman=/usr/bin/podman
      backend_image="localhost/pay3flow/backend:$image_tag"
      frontend_image="localhost/pay3flow/frontend:$image_tag"
      backend_log="/tmp/pay3flow-backend-${revision:0:7}.log"
      frontend_log="/tmp/pay3flow-frontend-${revision:0:7}.log"

      for executable in "$kubectl" "$k3s" "$podman"; do
        if [[ ! -x $executable ]]; then
          echo "Required executable is missing on the server: $executable" >&2
          exit 1
        fi
      done

      sudo -n true
      sudo "$kubectl" get namespace "$namespace" >/dev/null
      sudo "$kubectl" -n "$namespace" get \
        deployment/pay3flow-backend \
        deployment/pay3flow-frontend \
        configmap/pay3flow-config \
        secret/pay3flow-secrets >/dev/null

      secret_keys=$(sudo "$kubectl" -n "$namespace" get secret pay3flow-secrets \
        -o go-template='{{range $key,$value := .data}}{{$key}}{{"\n"}}{{end}}')
      for key in \
        DATABASE_URL REDIS_URL JWT_SECRET SECRETS_KEY ADMIN_TOKEN \
        NEAR_INTENTS_JWT BESTCHANGE_API_KEY SYMBIOSIS_PARTNER_ID; do
        if ! grep -Fxq "$key" <<<"$secret_keys"; then
          echo "Secret pay3flow-secrets is missing key: $key" >&2
          exit 1
        fi
      done

      printf '%s\n' "$revision" >"$release_dir/.source-revision"
      cd "$release_dir"
      rm -f "$backend_log" "$frontend_log"

      echo "Building backend and frontend images in parallel..."
      "$podman" build --tag "$backend_image" --file backend/Dockerfile . \
        >"$backend_log" 2>&1 &
      backend_pid=$!
      "$podman" build --tag "$frontend_image" \
        --file pay3low-svelte-frontend/Dockerfile pay3low-svelte-frontend \
        >"$frontend_log" 2>&1 &
      frontend_pid=$!

      build_status=0
      wait "$backend_pid" || build_status=1
      wait "$frontend_pid" || build_status=1
      if ((build_status != 0)); then
        echo "Backend build log:" >&2
        tail -80 "$backend_log" >&2
        echo "Frontend build log:" >&2
        tail -80 "$frontend_log" >&2
        exit 1
      fi
      tail -8 "$backend_log"
      tail -8 "$frontend_log"

      echo "Importing images into k3s..."
      # Import separately. Combined archives can assign both tags to one image
      # with the containerd version currently running on the production host.
      # Temporary archives also avoid a pipefail/SIGPIPE race when containerd
      # recognizes an already-cached image and closes stdin before podman exits.
      backend_archive=$(mktemp /tmp/pay3flow-backend-image.XXXXXX.tar)
      frontend_archive=$(mktemp /tmp/pay3flow-frontend-image.XXXXXX.tar)
      cleanup_archives() {
        rm -f "$backend_archive" "$frontend_archive"
      }
      trap cleanup_archives EXIT
      "$podman" save --output "$backend_archive" "$backend_image"
      "$podman" save --output "$frontend_archive" "$frontend_image"
      sudo "$k3s" ctr images import "$backend_archive" >/dev/null
      sudo "$k3s" ctr images import "$frontend_archive" >/dev/null
      cleanup_archives
      trap - EXIT

      source_backend_id=$("$podman" image inspect --format '{{.Id}}' "$backend_image")
      source_frontend_id=$("$podman" image inspect --format '{{.Id}}' "$frontend_image")
      source_backend_id=${source_backend_id#sha256:}
      source_frontend_id=${source_frontend_id#sha256:}
      runtime_backend_id=$(sudo "$k3s" crictl images | awk -v tag="$image_tag" \
        '$1 == "localhost/pay3flow/backend" && $2 == tag { print $3; exit }')
      runtime_frontend_id=$(sudo "$k3s" crictl images | awk -v tag="$image_tag" \
        '$1 == "localhost/pay3flow/frontend" && $2 == tag { print $3; exit }')

      if [[ ${source_backend_id:0:12} != "${runtime_backend_id:0:12}" ]]; then
        echo "Imported backend image ID does not match the build output." >&2
        exit 1
      fi
      if [[ ${source_frontend_id:0:12} != "${runtime_frontend_id:0:12}" ]]; then
        echo "Imported frontend image ID does not match the build output." >&2
        exit 1
      fi
      if [[ ${runtime_backend_id:0:12} == "${runtime_frontend_id:0:12}" ]]; then
        echo "Backend and frontend unexpectedly resolve to the same image ID." >&2
        exit 1
      fi

      old_backend_image=$(sudo "$kubectl" -n "$namespace" get deployment \
        pay3flow-backend -o jsonpath='{.spec.template.spec.containers[0].image}')
      old_frontend_image=$(sudo "$kubectl" -n "$namespace" get deployment \
        pay3flow-frontend -o jsonpath='{.spec.template.spec.containers[0].image}')
      rollout_started=0

      rollback() {
        status=$?
        trap - ERR
        if ((rollout_started == 1)); then
          echo "Rollout failed; restoring the previous images." >&2
          sudo "$kubectl" -n "$namespace" set image deployment/pay3flow-backend \
            "backend=$old_backend_image" || true
          sudo "$kubectl" -n "$namespace" set image deployment/pay3flow-frontend \
            "frontend=$old_frontend_image" || true
          sudo "$kubectl" -n "$namespace" rollout status --timeout=5m \
            deployment/pay3flow-backend || true
          sudo "$kubectl" -n "$namespace" rollout status --timeout=5m \
            deployment/pay3flow-frontend || true
        fi
        exit "$status"
      }
      trap rollback ERR

      deployed_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
      rollout_started=1
      sudo "$kubectl" -n "$namespace" patch deployment pay3flow-backend \
        --type=strategic --patch-file=/dev/stdin <<PATCH
      {"spec":{"template":{"metadata":{"annotations":{"pay3flow.io/deployed-at":"$deployed_at","pay3flow.io/source-revision":"$revision"}},"spec":{"containers":[{"name":"backend","image":"$backend_image","imagePullPolicy":"IfNotPresent","env":[{"name":"BESTCHANGE_API_KEY","valueFrom":{"secretKeyRef":{"name":"pay3flow-secrets","key":"BESTCHANGE_API_KEY"}}},{"name":"SYMBIOSIS_PARTNER_ID","valueFrom":{"secretKeyRef":{"name":"pay3flow-secrets","key":"SYMBIOSIS_PARTNER_ID"}}}] }]}}}}
      PATCH

      sudo "$kubectl" -n "$namespace" patch deployment pay3flow-frontend \
        --type=strategic --patch-file=/dev/stdin <<PATCH
      {"spec":{"template":{"metadata":{"annotations":{"pay3flow.io/deployed-at":"$deployed_at","pay3flow.io/source-revision":"$revision"}},"spec":{"containers":[{"name":"frontend","image":"$frontend_image","imagePullPolicy":"IfNotPresent"}]}}}}
      PATCH

      sudo "$kubectl" -n "$namespace" patch configmap pay3flow-config \
        --type=merge --patch-file=/dev/stdin >/dev/null <<PATCH
      {"data":{"IMAGE_TAG":"$image_tag","SOURCE_REVISION":"$revision"}}
      PATCH

      sudo "$kubectl" -n "$namespace" rollout status --timeout=10m \
        deployment/pay3flow-backend
      sudo "$kubectl" -n "$namespace" rollout status --timeout=10m \
        deployment/pay3flow-frontend
      trap - ERR

      sudo "$kubectl" -n "$namespace" get deployment \
        pay3flow-backend pay3flow-frontend \
        -o custom-columns=NAME:.metadata.name,READY:.status.readyReplicas,IMAGE:.spec.template.spec.containers[0].image
    BASH

    def main(arguments = ARGV)
      if arguments == ["--help"] || arguments == ["-h"]
        puts HELP
        return
      end
      abort HELP unless arguments.empty?

      repository_root = capture!("git", "rev-parse", "--show-toplevel").strip
      unless capture!("git", "status", "--porcelain", chdir: repository_root).empty?
        abort "Refusing to deploy a dirty worktree. Commit or stash all changes first."
      end

      revision = capture!("git", "rev-parse", "HEAD", chdir: repository_root).strip
      short_revision = revision[0, 7]
      image_tag = ENV.fetch("PAY3FLOW_IMAGE_TAG", "live-#{Time.now.utc.strftime('%Y%m%d')}-#{short_revision}")
      deploy_host = ENV.fetch("PAY3FLOW_DEPLOY_HOST", "82.118.18.183")
      deploy_user = ENV.fetch("PAY3FLOW_DEPLOY_USER", "xila")
      namespace = ENV.fetch("PAY3FLOW_NAMESPACE", "pay3flow")
      public_url = ENV.fetch("PAY3FLOW_PUBLIC_URL", "https://pay3flow.lefine.pro").sub(%r{/+\z}, "")
      release_name = "pay3flow-release-#{short_revision}"
      remote = "#{deploy_user}@#{deploy_host}"

      abort "Invalid PAY3FLOW_IMAGE_TAG: #{image_tag}" unless image_tag.match?(/\A[A-Za-z0-9._-]+\z/)
      unless namespace.match?(/\A[a-z0-9](?:[-a-z0-9]*[a-z0-9])?\z/)
        abort "Invalid PAY3FLOW_NAMESPACE: #{namespace}"
      end

      ssh_options = ["-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes"]
      identity = ENV["PAY3FLOW_DEPLOY_IDENTITY_FILE"]
      ssh_options.concat(["-i", identity]) if identity && !identity.empty?
      ssh = ["ssh", *ssh_options]

      puts "Deploying Pay3Flow #{revision} as #{image_tag} to #{remote}/#{namespace}"
      run!(*ssh, remote, "test -d #{release_name} || mkdir -m 0750 #{release_name}")

      # rsync parses -e itself rather than through a shell, so shell-escaped
      # equals signs (for example BatchMode\=yes) are treated literally.
      rsync_ssh = ssh.join(" ")
      run!(
        "rsync", "-az", "--delete",
        "--exclude=.git/", "--exclude=backend/.env",
        "--exclude=node_modules/", "--exclude=target/",
        "-e", rsync_ssh, "./", "#{remote}:#{release_name}/",
        chdir: repository_root
      )

      run!(
        *ssh, remote, "/bin/bash", "-s", "--",
        image_tag, revision, namespace, release_name,
        stdin: REMOTE_SCRIPT
      )

      puts "Verifying public endpoints..."
      capture!("curl", "--fail", "--silent", "--show-error", "--max-time", "30", public_url)
      health = capture!("curl", "--fail", "--silent", "--show-error", "--max-time", "30", "#{public_url}/health")
      abort "Unexpected health response: #{health.inspect}" unless health == "ok"

      providers = capture!("curl", "--fail", "--silent", "--show-error", "--max-time", "30", "#{public_url}/api/providers")
      %w[bestchange symbiosis].each do |provider|
        abort "Provider #{provider} is missing from the public catalog." unless providers.downcase.include?(%Q{"#{provider}"})
      end

      puts "Pay3Flow #{revision} is live at #{public_url}"
    end

    def capture!(*command, chdir: nil)
      options = chdir ? { chdir: chdir } : {}
      stdout, stderr, status = Open3.capture3(*command, **options)
      return stdout if status.success?

      warn stderr
      abort "Command failed (#{status.exitstatus}): #{Shellwords.join(command)}"
    end
    private_class_method :capture!

    def run!(*command, stdin: nil, chdir: nil)
      options = chdir ? { chdir: chdir } : {}
      status = nil
      Open3.popen2e(*command, **options) do |input, output, wait_thread|
        writer = if stdin
                   Thread.new do
                     input.write(stdin)
                     input.close
                   end
                 else
                   input.close
                   nil
                 end
        output.each { |line| $stdout.write(line) }
        writer&.join
        status = wait_thread.value
      end
      return if status.success?

      abort "Command failed (#{status.exitstatus}): #{Shellwords.join(command)}"
    end
    private_class_method :run!
  end
end

Pay3flow::LiveDeploy.main if $PROGRAM_NAME == __FILE__
