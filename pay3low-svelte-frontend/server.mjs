import http from "node:http";
import compression from "compression";
import { handler } from "./build/handler.js";

const host = process.env.HOST ?? "0.0.0.0";
const port = Number(process.env.PORT ?? 3000);
const compress = compression();

const server = http.createServer((request, response) => {
  const pathname = request.url?.split("?", 1)[0] ?? "/";
  if (pathname === "/favicon.ico" || pathname.startsWith("/fonts/") || pathname.startsWith("/icons/")) {
    response.setHeader("cache-control", "public, max-age=31536000, immutable");
  }
  compress(request, response, () => handler(request, response));
});

server.listen(port, host, () => {
  console.log(`Listening on http://${host}:${port}`);
});
