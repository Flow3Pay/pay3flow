# Pay3Flow Svelte frontend

Основной интерфейс Pay3Flow на SvelteKit. По умолчанию работает на порту 3000.

```bash
npm install
npm run dev -- --host 127.0.0.1 --port 3000
```

Для production-сборки:

```bash
npm run check
npm run build
PORT=3000 PUBLIC_API_URL=http://localhost:8080 node build
```

Интерфейс вместе с backend можно запустить из корня репозитория:

```bash
docker compose up --build backend pay3low-svelte-frontend
```
