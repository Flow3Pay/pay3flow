# k3s deployment

The flake renders Kubernetes resources for the services that exist in this
checkout: the Rust backend, the SvelteKit frontend, PostgreSQL, and Redis. The
upstream README mentions `fmatch`, but that service is not present in the
current source tree, so it is not deployed here.

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
export PAY3FLOW_INGRESS_HOST=pay3flow.example.com
export PAY3FLOW_INGRESS_TLS_SECRET=pay3flow-tls
export PAY3FLOW_JWT_SECRET='replace-with-a-long-random-value'
export PAY3FLOW_SECRETS_KEY='replace-with-application-key-material'
export PAY3FLOW_ADMIN_TOKEN='replace-with-a-random-admin-token'
export PAY3FLOW_POSTGRES_PASSWORD='replace-with-a-random-database-password'
```

## Deploy with in-cluster PostgreSQL and Redis

The deployment app deletes and recreates the two build Jobs on each run, then
waits for both images and the application rollouts:

```sh
nix run .#deploy
```

For inspection without applying anything:

```sh
nix run .#render-manifests > /tmp/pay3flow.yaml
kubectl apply --dry-run=client -f /tmp/pay3flow.yaml
```

The default image build Jobs clone the repository at the pinned revision in
`PAY3FLOW_SOURCE_REVISION`. Override `PAY3FLOW_SOURCE_REPOSITORY` and that
revision when deploying a fork or another commit.

## Use external PostgreSQL and Redis

Set both external URLs and switch the mode before rendering:

```sh
export PAY3FLOW_DATABASE_MODE=external
export PAY3FLOW_DATABASE_URL='postgres://user:password@db.example.com:5432/pay3flow?sslmode=require'
export PAY3FLOW_REDIS_URL='rediss://:password@redis.example.com:6379'
nix run .#deploy
```

In external mode the in-cluster PostgreSQL and Redis StatefulSets are rendered
with zero replicas. Their Services remain harmlessly present so switching
back to in-cluster mode does not require changing object names.

## TLS and routing

The Ingress routes `/api`, `/ws`, `/health`, ActivityPub paths, and `/routing`
to the backend; all other paths go to the frontend. Leaving
`PAY3FLOW_PUBLIC_API_URL` empty makes the frontend use the browser's current
origin, including WebSocket scheme conversion.

The backend creates its schema on startup. Back up external or persistent data
before changing image revisions, and do not use the development secret values
for real users or funds.
