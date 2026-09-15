export default function Home() {
  return (
    <main style={{ maxWidth: 640, margin: "4rem auto", fontFamily: "system-ui" }}>
      <h1>Pay3Flow</h1>
      <p>Трансграничные платежи через эквайринг — комиссия ниже, чем SWIFT.</p>
      <hr />
      <h2>Сервисы</h2>
      <ul>
        <li><strong>Backend</strong> — API, WebSocket (Rust / Axum)</li>
        <li><strong>Fmatch</strong> — матчинг запросов к эквайерам</li>
        <li><strong>Searcher</strong> — поиск новых эквайеров (SearXNG)</li>
      </ul>
    </main>
  );
}