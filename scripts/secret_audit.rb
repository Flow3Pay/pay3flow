#!/usr/bin/env ruby

require "open3"

root = File.expand_path("..", __dir__)
patterns = "-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----|AKIA[0-9A-Z]{16}|sk_live_[A-Za-z0-9]{20,}|ghp_[A-Za-z0-9]{30,}|[0-9]{3}-[0-9]{2}-[0-9]{4}"
paths = [":!docs/**", ":!scripts/secret_audit.rb"]

stdout, stderr, status = Dir.chdir(root) do
  Open3.capture3("git", "grep", "-nEI", "-e", patterns, "--", *paths)
end

if status.success?
  puts stdout
  warn "possible production secret found in tracked files"
  exit 1
elsif status.exitstatus > 1
  warn stderr unless stderr.empty?
  exit status.exitstatus
end

puts "secret audit passed (high-signal tracked-file patterns)"
