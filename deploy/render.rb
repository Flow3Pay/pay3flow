#!/usr/bin/env ruby

require "optparse"

module Pay3flow
  module Render
    DEFAULTS = {
      "PAY3FLOW_NAMESPACE" => "pay3flow",
      "PAY3FLOW_IMAGE_REGISTRY" => "registry.example.invalid/pay3flow",
      "PAY3FLOW_IMAGE_TAG" => "dev",
      "PAY3FLOW_SOURCE_REPOSITORY" => "https://github.com/Flow3Pay/pay3flow.git",
      "PAY3FLOW_SOURCE_REVISION" => "master",
      "PAY3FLOW_INGRESS_HOST" => "pay3flow.lefine.pro",
      "PAY3FLOW_INGRESS_CLASS" => "traefik",
      "PAY3FLOW_INGRESS_TLS_SECRET" => "",
      "PAY3FLOW_STORAGE_CLASS" => "local-path",
      "PAY3FLOW_DATABASE_MODE" => "in-cluster",
      "PAY3FLOW_DATABASE_URL" => "",
      "PAY3FLOW_REDIS_URL" => "",
      "PAY3FLOW_POSTGRES_PASSWORD" => "change-me-postgres",
      "PAY3FLOW_JWT_SECRET" => "change-me-jwt",
      "PAY3FLOW_SECRETS_KEY" => "change-me-secrets",
      "PAY3FLOW_ADMIN_TOKEN" => "change-me-admin",
      "PAY3FLOW_NEAR_INTENTS_JWT" => "",
      "PAY3FLOW_BESTCHANGE_API_KEY" => "",
      "PAY3FLOW_SYMBIOSIS_PARTNER_ID" => "",
      "PAY3FLOW_AP_HANDLE" => "pay3flow",
      "PAY3FLOW_AP_REQUIRE_SIGNATURES" => "true",
      "PAY3FLOW_FMATCH_INBOX" => "https://lefine.pro/inbox/actra",
      "PAY3FLOW_FMATCH_ACTOR_ID" => "https://lefine.pro/actors/actra",
      "PAY3FLOW_PUBLIC_API_URL" => "",
      "PAY3FLOW_FX_SOURCE" => "mock",
      "PAY3FLOW_P2P_SEARCH_ENABLED" => "true",
      "PAY3FLOW_P2P_BINANCE_ENABLED" => "true",
      "PAY3FLOW_P2P_BYBIT_ENABLED" => "true",
      "PAY3FLOW_P2P_OKX_ENABLED" => "true",
      "PAY3FLOW_P2P_BITGET_ENABLED" => "true",
      "PAY3FLOW_P2P_RAPIRA_ENABLED" => "true"
    }.freeze

    module_function

    def values(environment)
      values = DEFAULTS.each_with_object({}) do |(key, default), result|
        result[key] = environment.fetch(key, default)
      end
      values["PAY3FLOW_AP_ORIGIN"] = environment.fetch(
        "PAY3FLOW_AP_ORIGIN",
        "https://#{values.fetch("PAY3FLOW_INGRESS_HOST")}"
      )

      if values.fetch("PAY3FLOW_DATABASE_MODE") == "external"
        require_value!(values, "PAY3FLOW_DATABASE_URL")
        require_value!(values, "PAY3FLOW_REDIS_URL")
      else
        values["PAY3FLOW_DATABASE_URL"] =
          "postgres://pay3flow:#{values.fetch("PAY3FLOW_POSTGRES_PASSWORD")}@pay3flow-postgres:5432/pay3flow" if values["PAY3FLOW_DATABASE_URL"].empty?
        values["PAY3FLOW_REDIS_URL"] = "redis://pay3flow-redis:6379" if values["PAY3FLOW_REDIS_URL"].empty?
      end

      tls_secret = values.fetch("PAY3FLOW_INGRESS_TLS_SECRET")
      values["PAY3FLOW_INGRESS_TLS_BLOCK"] = if tls_secret.empty?
        ""
      else
        <<~YAML.chomp
            tls:
              - hosts:
                  - #{values.fetch("PAY3FLOW_INGRESS_HOST")}
                secretName: #{tls_secret}
        YAML
      end

      values
    end

    def render(application_template, services_template: nil, environment: ENV.to_h)
      values = values(environment)
      templates = [application_template]
      templates << services_template if services_template && values.fetch("PAY3FLOW_DATABASE_MODE") == "in-cluster"

      templates.map { |template| substitute(File.read(template), values) }.join("---\n")
    end

    def main(arguments)
      options = {
        template: ENV.fetch("PAY3FLOW_K8S_TEMPLATE", File.expand_path("kubernetes.yaml.in", __dir__)),
        services_template: ENV.fetch("PAY3FLOW_K8S_SERVICES_TEMPLATE", File.expand_path("in-cluster-services.yaml.in", __dir__))
      }
      parser = OptionParser.new do |opts|
        opts.banner = "Usage: render-pay3flow-k8s [--output FILE]"
        opts.on("--template FILE") { |value| options[:template] = value }
        opts.on("--services-template FILE") { |value| options[:services_template] = value }
        opts.on("--output FILE") { |value| options[:output] = value }
      end
      parser.parse!(arguments)
      abort parser.to_s unless arguments.empty?

      output = render(options.fetch(:template), services_template: options.fetch(:services_template))
      if options[:output]
        File.write(options[:output], output)
      else
        $stdout.write(output)
      end
    rescue KeyError => error
      abort error.message
    end

    def require_value!(values, key)
      abort "#{key} is required when PAY3FLOW_DATABASE_MODE=external" if values.fetch(key).empty?
    end
    private_class_method :require_value!

    def substitute(template, values)
      template.gsub(/\$([A-Z][A-Z0-9_]*)/) do |match|
        values.fetch(Regexp.last_match(1), match)
      end
    end
    private_class_method :substitute
  end
end

Pay3flow::Render.main(ARGV) if $PROGRAM_NAME == __FILE__
