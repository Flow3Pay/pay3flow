import { SITE_URL } from "$lib/home-content";
import { profileVenues } from "$lib/provider-profile";

export const prerender = true;

const origin = SITE_URL.replace(/\/$/, "");

/** Public pages worth indexing, in descending priority for the crawler. */
const pages = ["", "providers", "trends", "about", "terms"];

function escapeXml(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

export function GET() {
  const urls = [
    ...pages.map((page) => `${origin}/${page}`),
    ...profileVenues.map((venue) => `${origin}/providers/${venue.slug}`),
  ];
  const body = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls.map((url) => `  <url><loc>${escapeXml(url)}</loc></url>`).join("\n")}
</urlset>`;
  return new Response(body, {
    headers: { "Content-Type": "application/xml; charset=utf-8" },
  });
}
