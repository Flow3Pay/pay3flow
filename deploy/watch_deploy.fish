#!/usr/bin/env fish

# Deploy Pay3Flow and show the build as it happens. The deploy command writes
# the backend and frontend build output to /tmp/pay3flow-*.log on the host, so
# this wrapper follows those files over SSH while the deploy itself runs.

function die
    echo $argv >&2
    exit 1
end

function usage
    echo 'Deploy Pay3Flow and stream the build output in real time.'
    echo ''
    echo 'Usage:'
    echo '  nix run .#deploy-watch'
    echo '  nix run .#deploy-watch -- [deploy arguments]'
    echo ''
    echo 'Runs `deploy/live_deploy.fish` for the current commit and follows'
    echo '/tmp/pay3flow-backend-REV.log and /tmp/pay3flow-frontend-REV.log on the'
    echo 'deployment host, so the Rust and frontend builds, verification, rollout'
    echo 'and cleanup are visible while they run.'
    echo ''
    echo 'Optional environment variables:'
    echo '  PAY3FLOW_DEPLOY_HOST           SSH host (default: 195.123.218.200)'
    echo '  PAY3FLOW_DEPLOY_USER           SSH user (default: root)'
    echo '  PAY3FLOW_DEPLOY_IDENTITY_FILE  SSH private key (default: SSH configuration)'
    echo '  PAY3FLOW_WATCH_LOG_DIR         Remote log directory (default: /tmp)'
    echo '  PAY3FLOW_WATCH_DEPLOY_COMMAND  Command to run instead of the deployment'
    echo '                                 script, for example: nix run .#deploy'
end

if test (count $argv) -gt 0
    switch $argv[1]
        case --help -h
            usage
            exit 0
    end
end

set repo_root (git rev-parse --show-toplevel 2>/dev/null | string collect)
or die 'This command must run inside the Pay3Flow git worktree.'

set revision (git -C $repo_root rev-parse HEAD | string trim)
set short_revision (string sub --start 1 --length 7 -- $revision)

set deploy_host $PAY3FLOW_DEPLOY_HOST
test -n "$deploy_host"; or set deploy_host 195.123.218.200
set deploy_user $PAY3FLOW_DEPLOY_USER
test -n "$deploy_user"; or set deploy_user root
set remote $deploy_user@$deploy_host

set ssh_options -o BatchMode=yes -o StrictHostKeyChecking=yes
if test -n "$PAY3FLOW_DEPLOY_IDENTITY_FILE"
    set ssh_options $ssh_options -i $PAY3FLOW_DEPLOY_IDENTITY_FILE
end

set log_dir $PAY3FLOW_WATCH_LOG_DIR
test -n "$log_dir"; or set log_dir /tmp
set backend_log "$log_dir/pay3flow-backend-$short_revision.log"
set frontend_log "$log_dir/pay3flow-frontend-$short_revision.log"

ssh $ssh_options $remote true; or die "Cannot reach $remote over SSH."

# Start from empty logs so only this deploy is streamed.
ssh $ssh_options $remote "rm -f '$backend_log' '$frontend_log'" 2>/dev/null

# One SSH session follows both logs. -n keeps the background ssh from reading
# the terminal, and sed -u keeps the prefixed lines unbuffered.
set stream_command "stdbuf -oL tail -n +1 -F '$backend_log' 2>/dev/null | sed -u 's/^/[backend] /' & stdbuf -oL tail -n +1 -F '$frontend_log' 2>/dev/null | sed -u 's/^/[frontend] /' & wait"
ssh -n $ssh_options $remote $stream_command &
set stream_pid $last_pid

function stop_stream
    test "$stream_pid" -gt 0; and kill $stream_pid 2>/dev/null
end

function stop_on_signal --on-signal INT --on-signal TERM
    stop_stream
    exit 130
end

echo "[$(date +%H:%M:%S)] Deploying $short_revision to $remote"
echo "[$(date +%H:%M:%S)] Streaming $backend_log and $frontend_log"

set started (date +%s)
if set -q PAY3FLOW_WATCH_DEPLOY_COMMAND
    eval $PAY3FLOW_WATCH_DEPLOY_COMMAND $argv
else
    fish "$repo_root/deploy/live_deploy.fish" $argv
end
set deploy_status $status

stop_stream
wait $stream_pid 2>/dev/null
set elapsed (math (date +%s) - $started)

if test $deploy_status -eq 0
    echo "[$(date +%H:%M:%S)] Deploy finished in "$elapsed"s."
else
    echo "[$(date +%H:%M:%S)] Deploy failed after "$elapsed"s (exit $deploy_status)." >&2
end
echo "Build logs remain on $remote at $backend_log and $frontend_log."

exit $deploy_status
