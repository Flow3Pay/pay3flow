//! Hosted payment page. One screen, no wallet connection, no signature.
//!
//! The page never marks an order paid: it only displays backend state and
//! refreshes it. `paid` means the provider delivered to the recipient.

use super::model::CheckoutStatusView;

pub fn render(token: &str, view: &CheckoutStatusView, qr_svg: &str) -> String {
    let quote = &view.quote;
    let rate = display_rate(&quote.input_amount, &quote.output_amount);
    let success = view
        .success_url
        .as_deref()
        .filter(|_| matches!(view.status.as_str(), "paid" | "overpaid"))
        .map(|url| {
            format!(
                r#"<a class="success" href="{}">Return to the merchant</a>"#,
                escape(url)
            )
        })
        .unwrap_or_default();
    let token_json = serde_json::to_string(token).unwrap_or_else(|_| "\"\"".into());
    let expires = quote.expires_at.to_rfc3339();
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>Pay {input_amount} {input_asset}</title>
<style>
:root {{ color-scheme: light dark; --fg:#111; --bg:#fff; --muted:#5b6470; --line:#dfe3e8; --warn:#7a4b00; --warnbg:#fff4e0; }}
@media (prefers-color-scheme: dark) {{ :root {{ --fg:#f3f4f6; --bg:#101216; --muted:#9aa3af; --line:#2a2f37; --warn:#ffd390; --warnbg:#3a2a10; }} }}
* {{ box-sizing: border-box; }}
body {{ margin:0; font:16px/1.5 system-ui,-apple-system,Segoe UI,Roboto,sans-serif; color:var(--fg); background:var(--bg); }}
main {{ max-width:460px; margin:0 auto; padding:24px 18px 48px; }}
h1 {{ font-size:1.35rem; margin:0 0 4px; }}
.muted {{ color:var(--muted); }}
.net {{ display:inline-block; margin:6px 0 18px; padding:2px 8px; border:1px solid var(--line); border-radius:999px; font-size:.85rem; }}
.amount {{ font-size:1.05rem; margin-bottom:18px; }}
.rate {{ font-size:.9rem; color:var(--muted); margin:-8px 0 18px; }}
.qr {{ display:flex; justify-content:center; padding:12px; background:#fff; border-radius:12px; border:1px solid var(--line); }}
.qr svg {{ width:min(70vw,220px); height:auto; }}
.addr {{ display:flex; gap:8px; align-items:center; margin:14px 0; padding:10px; border:1px solid var(--line); border-radius:10px; }}
.addr code {{ flex:1; overflow-wrap:anywhere; font-size:.9rem; }}
button {{ font:inherit; padding:9px 14px; border-radius:9px; border:1px solid var(--line); background:transparent; color:var(--fg); cursor:pointer; }}
button.primary {{ width:100%; background:var(--fg); color:var(--bg); border-color:var(--fg); }}
.warning {{ background:var(--warnbg); color:var(--warn); padding:10px 12px; border-radius:10px; font-size:.9rem; }}
.status {{ margin:18px 0; font-weight:600; }}
.timer {{ font-variant-numeric:tabular-nums; }}
.success {{ display:block; text-align:center; margin-top:18px; padding:12px; border-radius:10px; background:var(--fg); color:var(--bg); text-decoration:none; }}
</style>
</head>
<body>
<main>
  <h1>Pay {input_amount} {input_asset}</h1>
  <span class="net">{input_network} network</span>
  <div class="amount">Recipient gets <strong>{output_amount} {output_asset}</strong> on {output_network}</div>
  <div class="rate">1 {input_asset} ≈ {rate} {output_asset} &middot; 0% Pay3Flow fee; the rate includes liquidity margin.</div>
  <div class="qr">{qr_svg}</div>
  <div class="addr">
    <code id="deposit">{deposit_address}</code>
    <button id="copy" type="button">Copy</button>
  </div>
  <p class="warning">Send only {input_asset} on {input_network}. Assets or networks sent elsewhere may be lost.</p>
  <p class="status" id="status">{status_label}</p>
  <p class="muted">Quote expires in <span class="timer" id="countdown">&mdash;</span> (<span id="expires">{expires}</span>).</p>
  <button class="primary" id="paid" type="button">I have paid</button>
  {success}
</main>
<script>
const token = {token_json};
const statusNode = document.getElementById("status");
const labels = {{awaiting_deposit:"Waiting for your deposit",processing:"Deposit received, delivering",paid:"Delivered to the recipient",overpaid:"Delivered to the recipient",underpaid:"Deposit was below the quoted amount",expired:"Quote expired",failed:"Transfer failed",refunded:"Refunded",cancelled:"Cancelled"}};
function show(status) {{ statusNode.textContent = labels[status] || status; }}
const expiry = new Date(document.getElementById("expires").textContent).getTime();
function tick() {{
  const left = Math.max(0, expiry - Date.now());
  const minutes = Math.floor(left / 60000);
  const seconds = Math.floor((left % 60000) / 1000);
  document.getElementById("countdown").textContent = left === 0 ? "expired" : minutes + ":" + String(seconds).padStart(2, "0");
}}
tick(); setInterval(tick, 1000);
async function refresh() {{
  try {{
    const response = await fetch("/pay/" + encodeURIComponent(token) + "/status", {{headers: {{accept: "application/json"}}}});
    if (!response.ok) return;
    const body = await response.json();
    show(body.status);
    if (body.status === "paid" || body.status === "overpaid" || body.status === "refunded") {{
      clearInterval(poll);
    }}
  }} catch (_) {{}}
}}
document.getElementById("paid").addEventListener("click", refresh);
document.getElementById("copy").addEventListener("click", async () => {{
  await navigator.clipboard.writeText(document.getElementById("deposit").textContent);
  document.getElementById("copy").textContent = "Copied";
}});
const poll = setInterval(refresh, 10000);
refresh();
</script>
</body>
</html>"#,
        input_amount = escape(&quote.input_amount),
        input_asset = escape(&quote.input_asset),
        input_network = escape(&quote.input_network),
        output_amount = escape(&quote.output_amount),
        output_asset = escape(&quote.output_asset),
        output_network = escape(&quote.output_network),
        rate = escape(&rate),
        qr_svg = qr_svg,
        deposit_address = escape(&quote.deposit_address),
        status_label = escape(status_label(&view.status)),
        expires = escape(&expires),
        token_json = token_json,
        success = success,
    )
}

fn status_label(status: &str) -> &str {
    match status {
        "awaiting_deposit" => "Waiting for your deposit",
        "processing" => "Deposit received, delivering",
        "paid" | "overpaid" => "Delivered to the recipient",
        "underpaid" => "Deposit was below the quoted amount",
        "expired" => "Quote expired",
        "failed" => "Transfer failed",
        "refunded" => "Refunded",
        "cancelled" => "Cancelled",
        other => other,
    }
}

fn display_rate(input: &str, output: &str) -> String {
    let (Some(input), Some(output)) = (super::parse_scaled(input), super::parse_scaled(output))
    else {
        return "—".into();
    };
    if input == 0 {
        return "—".into();
    }
    let rate = output.saturating_mul(1_000_000) / input;
    let whole = rate / 1_000_000;
    let fraction = rate % 1_000_000;
    if fraction == 0 {
        return whole.to_string();
    }
    let fraction = format!("{fraction:06}");
    format!("{whole}.{}", fraction.trim_end_matches('0'))
}

fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_html() {
        assert_eq!(escape("<script>\"&"), "&lt;script&gt;&quot;&amp;");
    }

    #[test]
    fn rate_is_displayed_without_f64() {
        assert_eq!(display_rate("1000", "994.5"), "0.9945");
        assert_eq!(display_rate("2", "4"), "2");
    }
}
