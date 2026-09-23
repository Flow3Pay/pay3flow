#!/usr/bin/env ruby

require "open3"
require "tmpdir"

require_relative "render" unless defined?(Pay3flow::Render)

module Pay3flow
  module Deploy
    module_function

    def main(_arguments = [])
      kubectl = ENV.fetch("KUBECTL", "kubectl")
      namespace = ENV.fetch("PAY3FLOW_NAMESPACE", "pay3flow")
      template = ENV.fetch("PAY3FLOW_K8S_TEMPLATE", File.expand_path("kubernetes.yaml.in", __dir__))
      services_template = ENV.fetch(
        "PAY3FLOW_K8S_SERVICES_TEMPLATE",
        File.expand_path("in-cluster-services.yaml.in", __dir__)
      )

      namespace_yaml = run!(kubectl, "create", "namespace", namespace, "--dry-run=client", "-o", "yaml")
      run!(kubectl, "apply", "-f", "-", stdin: namespace_yaml)
      run!(kubectl, "-n", namespace, "get", "secret", "pay3flow-registry")
      run!(kubectl, "-n", namespace, "delete", "job", "--ignore-not-found", "pay3flow-build-backend", "pay3flow-build-frontend")

      Dir.mktmpdir("pay3flow-deploy") do |directory|
        manifest = File.join(directory, "pay3flow.yaml")
        File.write(manifest, Pay3flow::Render.render(template, services_template: services_template))
        run!(kubectl, "apply", "-f", manifest)
      end

      puts "Waiting for Kaniko image-build Jobs..."
      run!(kubectl, "-n", namespace, "wait", "--for=condition=complete", "--timeout=30m", "job/pay3flow-build-backend", "job/pay3flow-build-frontend")
      run!(kubectl, "-n", namespace, "rollout", "restart", "deployment/pay3flow-backend", "deployment/pay3flow-frontend")
      run!(kubectl, "-n", namespace, "rollout", "status", "--timeout=10m", "deployment/pay3flow-backend", "deployment/pay3flow-frontend")
    end

    def run!(*command, stdin: nil)
      stdout, stderr, status = Open3.capture3(*command, stdin_data: stdin)
      $stdout.write(stdout)
      $stderr.write(stderr)
      abort "command failed (#{status.exitstatus}): #{command.join(" ")}" unless status.success?
      stdout
    end
    private_class_method :run!
  end
end

Pay3flow::Deploy.main(ARGV) if $PROGRAM_NAME == __FILE__
