<script lang="ts">
  import { locale, t } from "$lib/i18n";
  import { COMMUNITY_URL, PROJECT_URL, homeContent } from "$lib/home-content";
  import { assetIcon, networkIcon, venueIcon } from "$lib/icons";
  import { fiatFlagUrl } from "$lib/currency-flags";
  import type { PaymentMethod } from "$lib/payment-methods";
  import type { ProviderDefinition } from "$lib/exchange";
  export let paymentMethods: PaymentMethod[] = [];
  export let providerCatalog: ProviderDefinition[] = [];
  $: copy = homeContent[$locale];

  // Complete selectable catalog for SSR and API outages. Live catalogs are
  // shared with the converter after hydration, without additional requests.
  const defaultAssets = ["AMD", "RUB", "USD", "BYN", "KZT", "UAH", "USDT", "USDC", "BTC", "ETH", "ADA", "APT", "ATOM", "AVAX", "BCH", "BNB", "DAI", "DOGE", "DOT", "FDUSD", "LINK", "LTC", "MATIC", "NEAR", "SOL", "SUI", "TON", "TRX", "UNI", "XRP"];
  const defaultProviders = [
    { id: "bestchange", name: "BestChange" },
    { id: "binance", name: "Binance" },
    { id: "bitcoin-center", name: "Bitcoin Center" },
    { id: "bitget", name: "Bitget" },
    { id: "bncex", name: "bncex" },
    { id: "bybit", name: "Bybit" },
    { id: "cifra-broker", name: "Cifra Markets" },
    { id: "cow-swap", name: "CoW Swap" },
    { id: "dzengi", name: "Dzengi" },
    { id: "id-pay", name: "ID Pay" },
    { id: "mexc", name: "MEXC" },
    { id: "near-intents", name: "NEAR Intents" },
    { id: "okx", name: "OKX" },
    { id: "papa-change", name: "Papa Change" },
    { id: "rapira", name: "Rapira" },
    { id: "skylabs", name: "SkyLabs" },
    { id: "symbiosis", name: "Symbiosis" },
    { id: "whitebird", name: "Whitebird" },
  ];
  $: assets = paymentMethods.length
    ? [...new Set(paymentMethods.map((method) => method.currency.toUpperCase()))]
        .sort((a, b) => assetOrder(a) - assetOrder(b) || a.localeCompare(b))
    : defaultAssets;
  $: providers = providerCatalog.length
    ? [...new Map(providerCatalog.map((provider) => [provider.slug, {
        id: provider.slug, name: provider.name.replace(/\s+(buy|sell)$/i, "").trim() || provider.slug
      }])).values()].sort((a, b) => a.name.localeCompare(b.name))
    : defaultProviders;
  function assetOrder(asset: string) {
    const index = defaultAssets.indexOf(asset);
    return index < 0 ? defaultAssets.length : index;
  }
</script>

<div class="overview" data-testid="home-overview">
  <div class="introduction">
    <section id="about" class="aboutCard" aria-labelledby="about-heading">
      <div class="brandLine"><span class="brandMark" aria-hidden="true">↗</span><span>Pay3Flow</span></div>
      <h2 id="about-heading">{copy.aboutTitle}</h2>
      <p>{copy.about}</p>
      <div class="marketTags"><span>P2P</span><span>{t("Exchangers", {}, $locale)}</span><span>SPOT</span></div>
      <div class="flowPreview" aria-hidden="true">{#each ["AMD", "USDT", "RUB"] as asset, index}{@const flag = fiatFlagUrl(asset)}{#if index}<b>→</b>{/if}<span><img src={flag ?? assetIcon(asset)} class:fiatFlag={Boolean(flag)} alt="" width="24" height="24" loading="lazy" />{asset}</span>{/each}</div>
    </section>
    <section id="how-it-works" class="howCard" aria-labelledby="how-heading">
      <div class="sectionHead"><h2 id="how-heading">{copy.howTitle}</h2></div>
      <ol>{#each copy.steps as step, index}<li><span class="stepIndex" aria-hidden="true">{String(index + 1).padStart(2, "0")}</span><p>{step}</p></li>{/each}</ol>
    </section>
  </div>

  <section class="coverage" aria-labelledby="coverage-heading">
    <div class="sectionHead"><h2 id="coverage-heading">{copy.coverageTitle}</h2></div>
    <div class="coverageGrid">
      <div class="coverageItem"><div class="assetTags">{#each assets as asset}{@const flag = fiatFlagUrl(asset)}<span><img src={flag ?? assetIcon(asset)} class:fiatFlag={Boolean(flag)} alt="" width="20" height="20" loading="lazy" />{asset}</span>{/each}</div><p>{copy.currencies}</p></div>
      <div class="coverageItem"><div class="providerTags">{#each providers as provider}<span><img src={venueIcon(provider.id)} alt="" width="20" height="20" loading="lazy" />{provider.name}</span>{/each}</div><p>{copy.sources}</p></div>
    </div>
    <h3>{copy.examplesTitle}</h3>
    <div class="examples">
      {#each copy.examples as example, index}
        <article><span class="exampleIndex" aria-hidden="true">{String(index + 1).padStart(2, "0")}</span><h4>{#each example.route.split(" → ") as asset, assetIndex}<span class="examplePart">{#if assetIndex}<span class="routeArrow" aria-hidden="true"><svg width="20" height="20" viewBox="0 0 24 24" fill="none"><path d="M3 12h16m-6-6 6 6-6 6" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round" /></svg></span>{/if}<span class="routeAsset"><span class="routeAssetMain"><img src={fiatFlagUrl(asset.split(" ")[0]) ?? assetIcon(asset.split(" ")[0])} class:fiatFlag={Boolean(fiatFlagUrl(asset.split(" ")[0]))} alt="" width="18" height="18" loading="lazy" /><strong>{asset.split(" (")[0]}</strong></span>{#if asset.includes(" (")}{@const network = asset.split(" (")[1].replace(")", "")}<span class="routeNetwork" title={network}><img src={networkIcon(network)} alt={network} width="18" height="18" loading="lazy" /></span>{/if}</span></span>{/each}</h4><p>{example.explanation}</p></article>
      {/each}
    </div>
  </section>


  <section class="faq" aria-labelledby="faq-heading">
    <div class="faqHeading"><h2 id="faq-heading">{copy.faqTitle}</h2></div>
    <div class="faqItems">{#each copy.faq as item}<details><summary><span>{item.question}</span><span class="expandIcon" aria-hidden="true">+</span></summary><p>{item.answer}</p></details>{/each}</div>
  </section>

  <section id="project" class="project" aria-labelledby="project-heading">
    <div class="projectCopy"><h2 id="project-heading">{copy.projectTitle}</h2><p>{copy.project}</p></div>
    <div class="projectLinks">
      <a class="primaryLink" href={PROJECT_URL} target="_blank" rel="noreferrer noopener"><span class="projectLinkLabel"><svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor" aria-hidden="true"><path d="M12 .7a11.3 11.3 0 0 0-3.58 22.02c.57.1.78-.25.78-.55v-2.16c-3.18.7-3.85-1.34-3.85-1.34-.52-1.32-1.27-1.67-1.27-1.67-1.04-.71.08-.7.08-.7 1.15.08 1.76 1.18 1.76 1.18 1.02 1.75 2.68 1.24 3.34.95.1-.74.4-1.24.73-1.53-2.54-.29-5.2-1.27-5.2-5.65 0-1.25.45-2.26 1.18-3.06-.12-.29-.51-1.45.11-3.02 0 0 .96-.31 3.12 1.17a10.8 10.8 0 0 1 5.68 0c2.16-1.48 3.12-1.17 3.12-1.17.62 1.57.23 2.73.11 3.02.73.8 1.18 1.81 1.18 3.06 0 4.39-2.67 5.35-5.21 5.64.41.36.78 1.08.78 2.18v3.23c0 .3.2.65.79.54A11.3 11.3 0 0 0 12 .7Z" /></svg><span>{copy.github}</span></span><span aria-hidden="true">↗</span></a>
      <a href={COMMUNITY_URL} target="_blank" rel="noreferrer noopener"><span class="projectLinkLabel"><img src="/icons/assets/telegram-messenger.png" alt="" width="20" height="20" loading="lazy" /><span>{copy.telegram}</span></span><span aria-hidden="true">↗</span></a>
      <a href="/terms"><span class="projectLinkLabel"><svg width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9m-6-6 6 6m-6-6v6h6M8 13h8M8 17h6" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" /></svg><span>{copy.terms}</span></span><span aria-hidden="true">↗</span></a>
    </div>
  </section>
</div>

<style>
  .overview { display: grid; gap: 24px; width: min(var(--layout-width), calc(100% - 2 * var(--page-gutter))); margin: 64px auto 0; color: var(--color-text); }
  section { min-width: 0; padding: 32px; border: 1px solid var(--color-border-strong); border-radius: var(--radius-card); background: var(--color-paper); scroll-margin-top: 24px; }
  .introduction { display: grid; grid-template-columns: .9fr 1.1fr; gap: 24px; }
  .sectionHead, .brandLine { display: flex; align-items: start; justify-content: space-between; gap: 16px; margin-bottom: 24px; }
  .sectionHead h2 { margin: 0; max-width: 30ch; }
  h2 { margin: 0 0 20px; font-size: clamp(24px, 2.3vw, 30px); font-weight: 750; line-height: 1.2; letter-spacing: -.045em; text-wrap: balance; }
  h3 { margin: 32px 0 16px; font-size: 16px; font-weight: 750; letter-spacing: -.02em; }
  p, h2, h3, h4, a, summary { min-width: 0; overflow-wrap: anywhere; }
  .projectCopy, .faqHeading, .faqItems { min-width: 0; }
  p { margin: 0; font-size: 16px; line-height: 1.65; color: var(--color-text-soft); }
  .aboutCard { display: flex; flex-direction: column; background: var(--color-panel); }
  .brandLine { align-items: center; justify-content: start; margin-bottom: 32px; font-size: 16px; font-weight: 800; letter-spacing: -.04em; }
  .brandMark { display: grid; width: 36px; height: 36px; place-items: center; border: 1px solid var(--color-accent-strong); border-radius: 9px; background: var(--color-accent); color: #132015; font-size: 25px; }
  .aboutCard h2 { font-size: clamp(30px, 3.3vw, 44px); max-width: 14ch; }
  .marketTags { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 24px; }
  .marketTags span { padding: 5px 10px; border: 1px solid var(--color-border-strong); border-radius: 6px; color: var(--color-text-soft); font: 12px/1.5 var(--font-mono); }
  .flowPreview { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-top: auto; padding-top: 32px; font: 500 clamp(18px, 2vw, 25px)/1.2 var(--font-mono); }
  .flowPreview span { display: inline-flex; align-items: center; gap: 8px; padding: 12px 0; }
  .flowPreview img { width: 24px; height: 24px; flex: 0 0 auto; border-radius: 50%; object-fit: contain; }
  .flowPreview img.fiatFlag { object-fit: cover; }
  .flowPreview b { color: var(--color-text-faint); font-size: 18px; font-weight: 400; }
  ol { display: grid; gap: 0; list-style: none; }
  .howCard { display: flex; flex-direction: column; }
  .howCard ol { flex: 1; grid-template-rows: repeat(3, minmax(0, 1fr)); }
  .howCard li { align-items: center; padding: 20px 0; }
  li { position: relative; display: grid; grid-template-columns: 40px 1fr; gap: 20px; padding: 24px 0; border-top: 1px solid var(--color-border); }
  li:first-child { padding-top: 0; border-top: 0; }
  li:last-child { padding-bottom: 0; }
  .stepIndex { display: grid; width: 40px; height: 40px; place-items: center; border: 1px solid var(--color-border-strong); border-radius: 50%; background: var(--color-panel); color: var(--color-accent-text); font: 500 14px/1 var(--font-mono); }
  .coverageGrid { display: grid; grid-template-columns: 1fr 1fr; gap: 24px; }
  .coverageItem { min-width: 0; }
  .assetTags, .providerTags { display: flex; flex-wrap: wrap; align-content: start; gap: 8px; min-height: 80px; margin-bottom: 16px; }
  .assetTags span, .providerTags span { display: flex; align-items: center; gap: 7px; min-height: 34px; padding: 6px 10px; border: 1px solid var(--color-border); border-radius: 7px; background: var(--color-panel); font-size: 12px; font-weight: 750; }
  .assetTags img, .providerTags img { object-fit: contain; border-radius: 50%; }
  .assetTags img.fiatFlag { object-fit: cover; }
  .examples { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
  article { min-width: 0; padding: 20px; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-panel); }
  .exampleIndex { display: block; margin-bottom: 20px; color: var(--color-text-faint); font: 12px/1 var(--font-mono); }
  h4 { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 0 0 16px; font: 500 14px/1.4 var(--font-mono); }
  .examplePart { display: inline-flex; align-items: center; gap: 8px; }
  .routeAsset { display: inline-flex; align-items: center; border: 1px solid var(--color-border-strong); border-radius: 5px; background: var(--color-panel-soft); color: var(--color-text); }
  .routeAssetMain { display: flex; align-items: center; gap: 6px; padding: 5px 7px; }
  .routeAsset strong { font-family: var(--font-sans); font-size: 12px; font-weight: 800; white-space: nowrap; }
  .routeAsset img { display: block; width: 18px; height: 18px; flex: 0 0 auto; border-radius: 50%; object-fit: contain; }
  .routeAsset img.fiatFlag { object-fit: cover; }
  .routeNetwork { display: flex; align-items: center; padding: 5px 7px; border-left: 1px solid var(--color-border-strong); }
  .routeArrow { display: grid; width: 20px; height: 20px; flex: 0 0 auto; place-items: center; color: var(--color-text); }
  .routeArrow svg { display: block; }
  article p { font-size: 14px; line-height: 1.6; }

  .faq { display: grid; grid-template-columns: 1fr; gap: 8px; }

  details { border-bottom: 1px solid var(--color-border); }
  details:first-child { border-top: 1px solid var(--color-border); }
  summary { display: flex; align-items: center; justify-content: space-between; gap: 20px; min-height: 64px; padding: 16px 0; cursor: pointer; list-style: none; font-size: 16px; font-weight: 650; line-height: 1.5; }
  summary::-webkit-details-marker { display: none; }
  .expandIcon { display: grid; width: 28px; height: 28px; flex: 0 0 auto; place-items: center; border: 1px solid var(--color-border); border-radius: 50%; color: var(--color-text-soft); font-size: 20px; font-weight: 400; transition: transform .2s ease, background .2s ease; }
  details[open] .expandIcon { transform: rotate(45deg); border-color: var(--color-accent-strong); background: var(--color-accent); color: #132015; }
  details p { max-width: 85ch; padding: 0 48px 24px 0; font-size: 14px; }
  .project { display: grid; grid-template-columns: 1.5fr 1fr; align-items: center; gap: 64px; background: var(--color-panel); }
  .projectLinks { display: grid; gap: 10px; }
  a { display: flex; align-items: center; justify-content: space-between; gap: 16px; min-height: 48px; padding: 12px 16px; border: 1px solid var(--color-border-strong); border-radius: 8px; background: var(--color-paper); font-size: 14px; font-weight: 650; line-height: 1.5; transition: border-color .16s ease, background .16s ease; }
  a > span:last-child { font-size: 20px; }
  .projectLinkLabel { display: inline-flex; align-items: center; gap: 10px; min-width: 0; }
  .projectLinkLabel img, .projectLinkLabel svg { display: block; width: 20px; height: 20px; flex: 0 0 auto; }
  a:hover { border-color: var(--color-accent-strong); }
  a.primaryLink { border-color: var(--color-accent-strong); background: var(--color-accent); color: #132015; }
  a.primaryLink:hover { background: var(--color-accent-strong); }
  @media (max-width: 980px) {
    .overview { margin-top: 44px; gap: 16px; }
    .introduction { grid-template-columns: 1fr 1fr; gap: 16px; }
    section { padding: 24px; }
    .coverageGrid { gap: 16px; }
    .examples { grid-template-columns: 1fr; }
    article { display: grid; grid-template-columns: 28px 1fr 1fr; align-items: center; gap: 16px; }
    article .exampleIndex, article h4 { margin: 0; }
    .faq { gap: 24px; }
    .project { gap: 32px; }
  }
  @media (max-width: 640px) {
    .introduction, .coverageGrid, .faq, .project { grid-template-columns: 1fr; }
    section { padding: 22px 18px; border-radius: 12px; }
    .sectionHead { gap: 12px; }
    .brandLine { margin-bottom: 24px; }
    .aboutCard h2 { max-width: none; }
    .flowPreview { gap: 6px; }
    .flowPreview span { gap: 6px; }
    .flowPreview img { width: 20px; height: 20px; }
    li { gap: 14px; padding: 20px 0; }
    .howCard ol { flex: initial; grid-template-rows: none; }
    .howCard li { align-items: start; }
    .howCard li:first-child { padding-top: 0; }
    .howCard li:last-child { padding-bottom: 0; }
    .coverageGrid { gap: 24px; }
    .assetTags, .providerTags { min-height: 0; }
    article { grid-template-columns: 24px 1fr; padding: 18px; gap: 12px; }
    article p { grid-column: 2; }

    .faqHeading h2 { margin: 0; }
    summary { gap: 12px; }
    details p { padding-right: 0; }
    .project { gap: 24px; }
  }
</style>
