import { SITE_URL } from "$lib/home-content";

export const prerender = true;

export function GET() {
  const urls = [SITE_URL, new URL("about", SITE_URL).href, new URL("terms", SITE_URL).href];
  const body = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls.map((url) => `  <url><loc>${url}</loc></url>`).join("\n")}
</urlset>`;
  return new Response(body, {
    headers: { "Content-Type": "application/xml; charset=utf-8" },
  });
}
