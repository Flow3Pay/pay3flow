<script lang="ts">
  import { locale, t } from "$lib/i18n";
  import { COMMUNITY_URL, PROJECT_URL, homeContent } from "$lib/home-content";
  import { assetIcon, venueIcon } from "$lib/icons";
  $: copy = homeContent[$locale];
  const assets = ["AMD", "RUB", "USD", "BYN", "KZT", "USDT", "USDC", "BTC", "ETH"];
  const providers = ["Binance", "Bybit", "OKX", "Bitget", "MEXC"];
</script>

<div class="overview" data-testid="home-overview">
  <div class="introduction">
    <section id="about" class="aboutCard" aria-labelledby="about-heading">
      <div class="brandLine"><span class="brandMark" aria-hidden="true">↗</span><span>Pay3Flow</span><span class="sectionIndex" aria-hidden="true">01 / 06</span></div>
      <h2 id="about-heading">{copy.aboutTitle}</h2>
      <p>{copy.about}</p>
      <div class="marketTags"><span>P2P</span><span>{t("Exchangers", {}, $locale)}</span><span>SPOT</span></div>
      <div class="flowPreview" aria-hidden="true"><span>AMD</span><b>→</b><span class="intermediate">USDT</span><b>→</b><span>RUB</span></div>
    </section>
    <section id="how-it-works" class="howCard" aria-labelledby="how-heading">
      <div class="sectionHead"><h2 id="how-heading">{copy.howTitle}</h2><span class="sectionIndex" aria-hidden="true">02 / 06</span></div>
      <ol>{#each copy.steps as step, index}<li><span class="stepIndex" aria-hidden="true">{String(index + 1).padStart(2, "0")}</span><p>{step}</p></li>{/each}</ol>
    </section>
  </div>

  <section class="coverage" aria-labelledby="coverage-heading">
    <div class="sectionHead"><h2 id="coverage-heading">{copy.coverageTitle}</h2><span class="sectionIndex" aria-hidden="true">03 / 06</span></div>
    <div class="coverageGrid">
      <div class="coverageItem"><div class="assetTags">{#each assets as asset}<span>{#if ["USDT", "USDC", "BTC", "ETH"].includes(asset)}<img src={assetIcon(asset)} alt="" width="20" height="20" loading="lazy" />{/if}{asset}</span>{/each}</div><p>{copy.currencies}</p></div>
      <div class="coverageItem"><div class="providerTags">{#each providers as provider}<span><img src={venueIcon(provider.toLowerCase())} alt="" width="20" height="20" loading="lazy" />{provider}</span>{/each}</div><p>{copy.sources}</p></div>
    </div>
    <h3>{copy.examplesTitle}</h3>
    <div class="examples">
      {#each copy.examples as example, index}
        <article><span class="exampleIndex" aria-hidden="true">{String(index + 1).padStart(2, "0")}</span><h4>{#each example.route.split(" → ") as asset, assetIndex}{#if assetIndex}<span class="routeArrow" aria-hidden="true">→</span>{/if}<span class="routeAsset">{asset}</span>{/each}</h4><p>{example.explanation}</p></article>
      {/each}
    </div>
  </section>

  <section id="methodology" class="methodology" aria-labelledby="method-heading">
    <div class="sectionHead"><h2 id="method-heading">{copy.methodTitle}</h2><span class="sectionIndex" aria-hidden="true">04 / 06</span></div>
    <div class="methodGrid">{#each copy.method as paragraph, index}<div class="methodCard"><span class="methodMark" aria-hidden="true">{index === 0 ? "⇄" : "%"}</span><p>{paragraph}</p></div>{/each}</div>
  </section>

  <section class="faq" aria-labelledby="faq-heading">
    <div class="faqHeading"><span class="sectionIndex" aria-hidden="true">05 / 06</span><h2 id="faq-heading">{copy.faqTitle}</h2><span class="questionMark" aria-hidden="true">?</span></div>
    <div class="faqItems">{#each copy.faq as item}<details><summary><span>{item.question}</span><span class="expandIcon" aria-hidden="true">+</span></summary><p>{item.answer}</p></details>{/each}</div>
  </section>

  <section id="project" class="project" aria-labelledby="project-heading">
    <div class="projectCopy"><span class="sectionIndex" aria-hidden="true">06 / 06</span><h2 id="project-heading">{copy.projectTitle}</h2><p>{copy.project}</p></div>
    <div class="projectLinks">
      <a class="primaryLink" href={PROJECT_URL} target="_blank" rel="noreferrer noopener"><span>{copy.github}</span><span aria-hidden="true">↗</span></a>
      <a href={COMMUNITY_URL} target="_blank" rel="noreferrer noopener"><span>{copy.telegram}</span><span aria-hidden="true">↗</span></a>
      <a href="/terms"><span>{copy.terms}</span><span aria-hidden="true">→</span></a>
    </div>
  </section>
</div>

<style>
  .overview { display: grid; gap: 24px; width: min(var(--layout-width), calc(100% - 2 * var(--page-gutter))); margin: 64px auto 0; color: var(--color-text); }
  section { min-width: 0; padding: 32px; border: 1px solid var(--color-border-strong); border-radius: var(--radius-card); background: var(--color-paper); scroll-margin-top: 24px; }
  .introduction { display: grid; grid-template-columns: .9fr 1.1fr; gap: 24px; }
  .sectionHead, .brandLine { display: flex; align-items: start; justify-content: space-between; gap: 16px; margin-bottom: 24px; }
  .sectionHead h2 { margin: 0; max-width: 30ch; }
  .sectionIndex { flex: 0 0 auto; color: var(--color-text-faint); font: 12px/1.6 var(--font-mono); letter-spacing: .04em; }
  h2 { margin: 0 0 20px; font-size: clamp(24px, 2.3vw, 30px); font-weight: 750; line-height: 1.2; letter-spacing: -.045em; text-wrap: balance; }
  h3 { margin: 32px 0 16px; font-size: 16px; font-weight: 750; letter-spacing: -.02em; }
  p, h2, h3, h4, a, summary { min-width: 0; overflow-wrap: anywhere; }
  .projectCopy, .faqHeading, .faqItems { min-width: 0; }
  p { margin: 0; font-size: 16px; line-height: 1.65; color: var(--color-text-soft); }
  .aboutCard { display: flex; flex-direction: column; background: var(--color-panel); }
  .brandLine { align-items: center; justify-content: start; margin-bottom: 32px; font-size: 16px; font-weight: 800; letter-spacing: -.04em; }
  .brandLine .sectionIndex { margin-left: auto; font-weight: 400; }
  .brandMark { display: grid; width: 36px; height: 36px; place-items: center; border: 1px solid var(--color-accent-strong); border-radius: 9px; background: var(--color-accent); color: #132015; font-size: 25px; }
  .aboutCard h2 { font-size: clamp(30px, 3.3vw, 44px); max-width: 14ch; }
  .marketTags { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 24px; }
  .marketTags span { padding: 5px 10px; border: 1px solid var(--color-border-strong); border-radius: 6px; color: var(--color-text-soft); font: 12px/1.5 var(--font-mono); }
  .flowPreview { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-top: auto; padding-top: 32px; font: 500 clamp(18px, 2vw, 25px)/1.2 var(--font-mono); }
  .flowPreview span { padding: 12px 0; }
  .flowPreview b { color: var(--color-text-faint); font-size: 18px; font-weight: 400; }
  .flowPreview .intermediate { padding: 12px 16px; border: 1px solid var(--color-accent-strong); border-radius: 8px; background: var(--color-accent); color: #132015; }
  ol { display: grid; gap: 0; list-style: none; }
  li { position: relative; display: grid; grid-template-columns: 40px 1fr; gap: 20px; padding: 24px 0; border-top: 1px solid var(--color-border); }
  li:first-child { padding-top: 0; border-top: 0; }
  li:last-child { padding-bottom: 0; }
  .stepIndex { display: grid; width: 40px; height: 40px; place-items: center; border: 1px solid var(--color-border-strong); border-radius: 50%; background: var(--color-panel); color: var(--color-accent-text); font: 500 14px/1 var(--font-mono); }
  .coverageGrid, .methodGrid { display: grid; grid-template-columns: 1fr 1fr; gap: 24px; }
  .coverageItem { min-width: 0; }
  .assetTags, .providerTags { display: flex; flex-wrap: wrap; align-content: start; gap: 8px; min-height: 80px; margin-bottom: 16px; }
  .assetTags span, .providerTags span { display: flex; align-items: center; gap: 7px; min-height: 34px; padding: 6px 10px; border: 1px solid var(--color-border); border-radius: 7px; background: var(--color-panel); font-size: 12px; font-weight: 750; }
  .assetTags img, .providerTags img { object-fit: contain; border-radius: 50%; }
  .examples { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
  article { min-width: 0; padding: 20px; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-panel); }
  .exampleIndex { display: block; margin-bottom: 20px; color: var(--color-text-faint); font: 12px/1 var(--font-mono); }
  h4 { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 0 0 16px; font: 500 14px/1.4 var(--font-mono); }
  .routeAsset { color: var(--color-text); }
  .routeArrow { color: var(--color-accent-text); font-size: 18px; }
  article p { font-size: 14px; line-height: 1.6; }
  .methodCard { display: grid; grid-template-columns: 36px 1fr; align-items: start; gap: 16px; padding: 24px; border: 1px solid var(--color-border); border-radius: 10px; }
  .methodMark { display: grid; width: 36px; height: 36px; place-items: center; border: 1px solid var(--color-border); border-radius: 8px; background: var(--color-panel); color: var(--color-accent-text); font: 500 20px/1 var(--font-mono); }
  .methodCard p { font-size: 14px; }
  .faq { display: grid; grid-template-columns: 1fr 2fr; gap: 40px; }
  .faqHeading .sectionIndex, .projectCopy > .sectionIndex { display: block; margin-bottom: 16px; }
  .questionMark { display: grid; width: 56px; height: 56px; place-items: center; margin-top: 24px; border: 1px solid var(--color-border); border-radius: 14px; background: var(--color-panel); color: var(--color-accent-text); font: 500 32px/1 var(--font-mono); }
  details { border-bottom: 1px solid var(--color-border); }
  details:first-child { border-top: 1px solid var(--color-border); }
  summary { display: flex; align-items: center; justify-content: space-between; gap: 20px; min-height: 64px; padding: 16px 0; cursor: pointer; list-style: none; font-size: 16px; font-weight: 650; line-height: 1.5; }
  summary::-webkit-details-marker { display: none; }
  .expandIcon { display: grid; width: 28px; height: 28px; flex: 0 0 auto; place-items: center; border: 1px solid var(--color-border); border-radius: 50%; color: var(--color-text-soft); font-size: 20px; font-weight: 400; transition: transform .2s ease, background .2s ease; }
  details[open] .expandIcon { transform: rotate(45deg); border-color: var(--color-accent-strong); background: var(--color-accent); color: #132015; }
  details p { padding: 0 48px 24px 0; font-size: 14px; }
  .project { display: grid; grid-template-columns: 1.5fr 1fr; align-items: center; gap: 64px; background: var(--color-panel); }
  .projectLinks { display: grid; gap: 10px; }
  a { display: flex; align-items: center; justify-content: space-between; gap: 16px; min-height: 48px; padding: 12px 16px; border: 1px solid var(--color-border-strong); border-radius: 8px; background: var(--color-paper); font-size: 14px; font-weight: 650; line-height: 1.5; transition: border-color .16s ease, background .16s ease; }
  a > span:last-child { font-size: 20px; }
  a:hover { border-color: var(--color-accent-strong); }
  a.primaryLink { border-color: var(--color-accent-strong); background: var(--color-accent); color: #132015; }
  a.primaryLink:hover { background: var(--color-accent-strong); }
  @media (max-width: 980px) {
    .overview { margin-top: 44px; gap: 16px; }
    .introduction { grid-template-columns: 1fr 1fr; gap: 16px; }
    section { padding: 24px; }
    .coverageGrid, .methodGrid { gap: 16px; }
    .methodCard { grid-template-columns: 1fr; padding: 20px; }
    .examples { grid-template-columns: 1fr; }
    article { display: grid; grid-template-columns: 28px 1fr 1fr; align-items: center; gap: 16px; }
    article .exampleIndex, article h4 { margin: 0; }
    .faq { gap: 24px; }
    .project { gap: 32px; }
  }
  @media (max-width: 640px) {
    .introduction, .coverageGrid, .methodGrid, .faq, .project { grid-template-columns: 1fr; }
    section { padding: 22px 18px; border-radius: 12px; }
    .sectionHead { gap: 12px; }
    .sectionIndex { font-size: 12px; }
    .brandLine { margin-bottom: 24px; }
    .aboutCard h2 { max-width: none; }
    li { gap: 14px; padding: 20px 0; }
    .coverageGrid { gap: 24px; }
    .assetTags, .providerTags { min-height: 0; }
    article { grid-template-columns: 24px 1fr; padding: 18px; gap: 12px; }
    article p { grid-column: 2; }
    .methodCard { grid-template-columns: 32px 1fr; padding: 18px 14px; gap: 12px; }
    .methodMark { width: 32px; height: 32px; }
    .questionMark { display: none; }
    .faqHeading h2 { margin: 0; }
    summary { gap: 12px; }
    details p { padding-right: 0; }
    .project { gap: 24px; }
  }
</style>
