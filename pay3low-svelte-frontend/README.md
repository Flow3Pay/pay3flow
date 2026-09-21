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
