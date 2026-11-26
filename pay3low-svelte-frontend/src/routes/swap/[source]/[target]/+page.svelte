<script lang="ts">
  import Header from '$lib/components/Header.svelte';
  import { assetDisplayName, relatedSwapPairs } from '$lib/swap-landing';
  import type { PageData } from './$types';
  export let data: PageData;

  $: copy = data.russian
    ? {
        heading: `Обмен ${data.source} на ${data.targetName}`,
        eyebrow: 'PAY3FLOW / БРИДЖ',
        amount: `Запрошенная сумма: ${data.amount} ${data.source}`,
        cta: 'Открыть обмен',
        stepsTitle: 'Как проходит обмен',
        steps: [
          `Укажите сумму ${data.source}, которую хотите отправить, и способ получения ${data.target}.`,
          'Выберите банк, сеть и площадки, которые нужно сравнить.',
          'Проверьте живые предложения и завершите обмен у выбранного провайдера.',
        ],
        relatedTitle: 'Другие направления',
        note: 'Pay3Flow сравнивает подключённые площадки. Итоговый курс, лимиты и комиссии устанавливает провайдер, которого вы выберете.',
        platforms: 'Площадки',
        terms: 'Условия использования',
      }
    : {
        heading: `Exchange ${data.source} for ${data.targetName}`,
        eyebrow: 'PAY3FLOW / BRIDGE',
        amount: `Requested amount: ${data.amount} ${data.source}`,
        cta: 'Open the exchange',
        stepsTitle: 'How the exchange works',
        steps: [
          `Enter the amount of ${data.source} you want to send and how you want to receive ${data.target}.`,
          'Pick the bank, network and providers you want to compare.',
          'Review the live offers and complete the exchange with the provider you choose.',
        ],
        relatedTitle: 'Other directions',
        note: 'Pay3Flow compares connected providers. The final rate, limits and fees are set by the provider you pick.',
        platforms: 'Platforms',
        terms: 'Usage policy',
      };

  $: related = relatedSwapPairs(data.source, data.target).map(([source, target]) => ({
    href: `/swap/${source}/${target}`,
    label: `${source} → ${assetDisplayName(target)}`,
  }));
</script>

<svelte:head>
  <title>{data.title}</title>
  <meta name="description" content={data.description} />
  <link rel="canonical" href={data.canonical} />
  {#if !data.indexable}<meta name="robots" content="noindex, follow" />{/if}
  <meta property="og:type" content="website" />
  <meta property="og:site_name" content="Pay3Flow" />
  <meta property="og:title" content={data.title} />
  <meta property="og:description" content={data.description} />
  <meta property="og:url" content={data.canonical} />
  <meta property="og:image" content={data.imageUrl} />
  <meta property="og:image:type" content="image/png" />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta property="og:image:alt" content={`${data.source} → ${data.target} · Pay3Flow exchange bridge`} />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={data.title} />
  <meta name="twitter:description" content={data.description} />
  <meta name="twitter:image" content={data.imageUrl} />
</svelte:head>

<div class="appShell">
  <Header activePage="swap" swapHref={data.exchangeUrl} />
  <main class="pair">
    <span class="eyebrow">{copy.eyebrow}</span>
    <h1>{copy.heading}</h1>
    <p class="lead">{data.description}</p>
    {#if data.amount}<p class="amount">{copy.amount}</p>{/if}
    <a class="cta" href={data.exchangeUrl}>{copy.cta} →</a>

    <section class="block">
      <h2>{copy.stepsTitle}</h2>
      <ol>
        {#each copy.steps as step}
          <li>{step}</li>
        {/each}
      </ol>
    </section>

    <section class="block">
      <h2>{copy.relatedTitle}</h2>
      <ul class="related">
        {#each related as item}
          <li><a href={item.href}>{item.label}</a></li>
        {/each}
      </ul>
    </section>

    <p class="note">{copy.note}</p>
  </main>
  <footer class="siteFooter">
    <span>Pay3Flow</span>
    <a href="/providers">{copy.platforms}</a>
    <a href="/terms">{copy.terms}</a>
  </footer>
</div>

<style>
  .pair {
    width: min(760px, calc(100% - 2 * var(--page-gutter)));
    margin: 56px auto;
  }

  .eyebrow {
    color: var(--color-text-soft);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.12em;
  }

  h1 {
    margin: 14px 0 0;
    font-size: clamp(30px, 4.4vw, 46px);
    letter-spacing: -0.045em;
    line-height: 1.05;
  }

  h2 {
    margin: 0 0 12px;
    font-size: 18px;
    letter-spacing: -0.02em;
  }

  .lead {
    margin: 18px 0 0;
    color: var(--color-text-soft);
    font-size: 14px;
    line-height: 1.75;
  }

  .amount {
    margin: 14px 0 0;
    color: var(--color-text);
    font-family: var(--font-mono);
    font-size: 13px;
  }

  .cta {
    display: inline-flex;
    margin-top: 26px;
    padding: 14px 22px;
    border-radius: 12px;
    background: var(--color-accent);
    color: #152016;
    font-weight: 700;
    text-decoration: none;
  }

  .cta:focus-visible {
    outline: var(--focus-ring-width) solid var(--color-focus);
    outline-offset: 3px;
  }

  .block {
    margin-top: 40px;
    padding-top: 24px;
    border-top: 1px solid var(--color-border);
  }

  ol,
  .related {
    display: grid;
    margin: 0;
    padding-left: 20px;
    gap: 10px;
    color: var(--color-text-soft);
    font-size: 14px;
    line-height: 1.65;
  }

  .related {
    padding-left: 0;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    list-style: none;
  }

  .related a {
    display: block;
    padding: 12px 14px;
    border: 1px solid var(--color-border);
    border-radius: 10px;
    background: var(--color-paper);
    color: var(--color-text);
    text-decoration: none;
  }

  .related a:hover {
    border-color: var(--color-accent-text);
  }

  .note {
    margin: 32px 0 0;
    color: var(--color-text-faint);
    font-size: 12px;
    line-height: 1.7;
  }
</style>
