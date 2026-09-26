# Pay3Flow web frontend

The frontend is a SvelteKit application for creating exchange orders, viewing
quotes, confirming funding instructions, and following settlement status.
The default development port is `3000`.

## Local development

```bash
npm install
npm run dev -- --host 127.0.0.1 --port 3000
```

The frontend expects the backend at `http://localhost:8080` during local
development. Set `PUBLIC_API_URL` when the backend is hosted elsewhere.

## Live result rendering

WebSocket route snapshots can arrive while other providers are still being
searched. The result header and the ranked cards deliberately represent two
different facts:

- a venue icon means that the source status reported one or more matching
  offers, even when none of those offers produced a card in the final ranking;
- a route card means that the backend assembled and ranked a complete route.

The frontend keeps previously reported venue icons for the duration of the
current search so a provider does not appear to disappear when a later ranked
snapshot replaces its cards.

Snapshots containing up to 100 routes are rendered in one update. Larger
snapshots are rendered in batches of 100 routes. After each batch, the
frontend yields for a browser animation frame and waits 50 ms before rendering
the next batch. Starting a newer search cancels any pending frame or timer from
the older snapshot. This keeps large DOM updates responsive without delaying
normal search results.

## Verification and production build

```bash
npm run check
npm run build
PORT=3000 PUBLIC_API_URL=http://localhost:8080 node build
```

Run the complete stack from the repository root:

```bash
docker compose up --build backend pay3low-svelte-frontend
```

The frontend is not a custody or payment-processing component. It must show
the selected route, fees, timing, consent terms, and any TOKEN or crypto
settlement asset clearly before the user confirms funding.
