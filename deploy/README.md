# k3s deployment

The flake renders Kubernetes resources for the services that exist in this
checkout: the Rust backend and SvelteKit frontend, plus optional in-cluster
PostgreSQL and Redis. The upstream README mentions `fmatch`, but that service
is not present in the current source tree, so it is not deployed here.

## Prerequisites

- Nix with flakes enabled.
- A configured `kubectl` context pointing at the target k3s cluster.
- An OCI registry reachable by the cluster and by the Kaniko Jobs.
- A dynamic storage provisioner. k3s's default `local-path` provisioner is
  used by default.
- An Ingress controller, normally the Traefik controller bundled with k3s.

Create the namespace and registry pull/push secret before using the deployment
app. The same registry secret is mounted by Kaniko and used as an image pull
secret by the application Pods:

```sh
kubectl create namespace pay3flow
kubectl -n pay3flow create secret docker-registry pay3flow-registry \
  --docker-server=registry.example.com \
  --docker-username="$REGISTRY_USERNAME" \
  --docker-password="$REGISTRY_PASSWORD"
```

The rendered manifests use `registry.example.invalid/pay3flow` and
`pay3flow.example.invalid` as safe placeholders. Set the deployment variables
before rendering or deploying:

```sh
export PAY3FLOW_IMAGE_REGISTRY=registry.example.com/team/pay3flow
export PAY3FLOW_IMAGE_TAG="$(git rev-parse --short=12 HEAD)"
export PAY3FLOW_INGRESS_HOST=pay3flow.lefine.pro
# Leave empty when Caddy terminates TLS on the production host.
export PAY3FLOW_INGRESS_TLS_SECRET=''
export PAY3FLOW_JWT_SECRET='replace-with-a-long-random-value'
export PAY3FLOW_SECRETS_KEY='replace-with-application-key-material'
export PAY3FLOW_ADMIN_TOKEN='replace-with-a-random-admin-token'
export PAY3FLOW_NEAR_INTENTS_JWT='replace-with-the-NEAR-Intents-JWT'
export PAY3FLOW_BESTCHANGE_API_KEY='replace-with-the-BestChange-API-key'
# Optional partner credential; leave empty for the public Symbiosis API tier.
export PAY3FLOW_SYMBIOSIS_PARTNER_ID=''
export PAY3FLOW_POSTGRES_PASSWORD='replace-with-a-random-database-password'
export PAY3FLOW_AP_REQUIRE_SIGNATURES='true'

# Lefine's production Fmatch actor. The dedicated Pay3Flow deployment does not
# run a second Fmatch instance.
export PAY3FLOW_FMATCH_INBOX='https://lefine.pro/inbox/actra'
export PAY3FLOW_FMATCH_ACTOR_ID='https://lefine.pro/actors/actra'
```

The `PAY3FLOW_*` values above are inputs to the manifest renderer. The backend
Pod does not receive these non-secret settings as environment variables; the
renderer writes them into the `config.toml` ConfigMap mounted at `/app/config.toml`.
Database, Redis, JWT, encryption, admin, BestChange, Symbiosis, and NEAR Intents credentials remain
Kubernetes Secret environment variables.

## Deploy with in-cluster PostgreSQL and Redis

The deployment app deletes and recreates the two build Jobs on each run, then
waits for both images, restarts both Deployments, and waits for their rollouts.
Application Pods always pull the requested image tag, so rebuilding a reused
development tag cannot leave an older backend running:

```sh
nix run .#deploy
```

For inspection without applying anything:

```sh
nix run .#render-manifests > /tmp/pay3flow.yaml
kubectl apply --dry-run=client -f /tmp/pay3flow.yaml
```

The image build Jobs clone `master` by default. Set
`PAY3FLOW_SOURCE_REVISION="$(git rev-parse HEAD)"` for a reproducible deployment,
or override `PAY3FLOW_SOURCE_REPOSITORY` and the revision when deploying a fork
or another commit.

## Use external PostgreSQL and Redis

Use the separate external configuration example in
[`deploy/external.env.example`](external.env.example), or export the same
variables before rendering:

```sh
export PAY3FLOW_DATABASE_MODE=external
export PAY3FLOW_DATABASE_URL='postgres://user:password@db.example.com:5432/pay3flow?sslmode=require'
export PAY3FLOW_REDIS_URL='rediss://:password@redis.example.com:6379'
nix run .#deploy
```

In external mode the application manifest does not include PostgreSQL or Redis
resources at all. In-cluster services are rendered only when
`PAY3FLOW_DATABASE_MODE=in-cluster`.

## TLS and routing

The Ingress routes `/api`, `/ws`, `/health`, ActivityPub paths, and `/routing`
to the backend; all other paths go to the frontend. Leaving
`PAY3FLOW_PUBLIC_API_URL` empty makes the frontend use the browser's current
origin, including WebSocket scheme conversion.

On the Lefine production host, k3s runs without Traefik and Caddy terminates
TLS. Apply [`Caddyfile`](Caddyfile) to the host's Caddy configuration after
deploying the manifests. The backend and frontend Services are NodePorts
`30081` and `30080`, respectively, so Caddy can route the public hostname to
the correct service.

The backend creates its schema on startup. Back up external or persistent data
before changing image revisions, and do not use the development secret values
for real users or funds.
