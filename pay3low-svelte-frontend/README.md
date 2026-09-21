# Pay3Flow Svelte frontend

Независимая реализация интерфейса Pay3Flow на SvelteKit. Оригинальный Next.js-фронтенд остаётся в `../frontend` и работает на порту 3000; этот проект работает на порту 3001.

```bash
npm install
npm run dev -- --host 127.0.0.1 --port 3001
```

Для production-сборки:

```bash
npm run check
npm run build
PORT=3001 PUBLIC_API_URL=http://localhost:8080 node build
```

Оба интерфейса вместе с backend можно запустить из корня репозитория:

```bash
docker compose up --build frontend pay3low-svelte-frontend
```
