import http from "node:http";
import compression from "compression";
import { handler } from "./build/handler.js";

const host = process.env.HOST ?? "0.0.0.0";
const port = Number(process.env.PORT ?? 3000);
const compress = compression();
const startedAt = process.hrtime.bigint();
const metrics = {
  requestsTotal: 0,
  requestsInFlight: 0,
  durationNanosecondsTotal: 0n,
  responses: { "2xx": 0, "3xx": 0, "4xx": 0, "5xx": 0 },
};

function renderMetrics() {
  const uptimeSeconds = Number(process.hrtime.bigint() - startedAt) / 1e9;
  const durationSeconds = Number(metrics.durationNanosecondsTotal) / 1e9;
  const lines = [
    "# HELP pay3flow_up Whether the Pay3Flow frontend is accepting requests.",
    "# TYPE pay3flow_up gauge",
    "pay3flow_up{service=\"frontend\"} 1",
    "# HELP pay3flow_build_info Static information about the Pay3Flow frontend.",
    "# TYPE pay3flow_build_info gauge",
    "pay3flow_build_info{service=\"frontend\"} 1",
    "# HELP process_uptime_seconds Seconds since the frontend process started.",
    "# TYPE process_uptime_seconds gauge",
    `process_uptime_seconds ${uptimeSeconds}`,
    "# HELP pay3flow_http_requests_total Total HTTP requests received by the frontend.",
    "# TYPE pay3flow_http_requests_total counter",
    `pay3flow_http_requests_total ${metrics.requestsTotal}`,
    "# HELP pay3flow_http_requests_in_flight Current HTTP requests being handled.",
    "# TYPE pay3flow_http_requests_in_flight gauge",
    `pay3flow_http_requests_in_flight ${metrics.requestsInFlight}`,
    "# HELP pay3flow_http_request_duration_seconds_total Sum of HTTP request durations.",
    "# TYPE pay3flow_http_request_duration_seconds_total counter",
    `pay3flow_http_request_duration_seconds_total ${durationSeconds}`,
    "# HELP pay3flow_http_responses_total HTTP responses grouped by status class.",
    "# TYPE pay3flow_http_responses_total counter",
    ...Object.entries(metrics.responses).map(([statusClass, value]) =>
      `pay3flow_http_responses_total{status_class="${statusClass}"} ${value}`),
  ];
  return `${lines.join("\n")}\n`;
}

const server = http.createServer((request, response) => {
  const pathname = request.url?.split("?", 1)[0] ?? "/";
  if (pathname === "/frontend-metrics") {
    response.writeHead(200, { "content-type": "text/plain; version=0.0.4; charset=utf-8" });
    response.end(renderMetrics());
    return;
  }

  const startedRequestAt = process.hrtime.bigint();
  metrics.requestsTotal += 1;
  metrics.requestsInFlight += 1;
  response.once("finish", () => {
    metrics.requestsInFlight = Math.max(0, metrics.requestsInFlight - 1);
    metrics.durationNanosecondsTotal += process.hrtime.bigint() - startedRequestAt;
    const statusClass = `${Math.floor(response.statusCode / 100)}xx`;
    if (statusClass in metrics.responses) metrics.responses[statusClass] += 1;
  });
  if (pathname === "/favicon.ico" || pathname.startsWith("/fonts/") || pathname.startsWith("/icons/")) {
    response.setHeader("cache-control", "public, max-age=31536000, immutable");
  }
  compress(request, response, () => handler(request, response));
});

server.listen(port, host, () => {
  console.log(`Listening on http://${host}:${port}`);
});
