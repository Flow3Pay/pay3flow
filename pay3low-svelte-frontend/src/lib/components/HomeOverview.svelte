<script lang="ts">
  import { locale } from "$lib/i18n";
  import { COMMUNITY_URL, PROJECT_URL, homeContent } from "$lib/home-content";
  $: copy = homeContent[$locale];
</script>

<div class="overview" data-testid="home-overview">
  <div class="introduction">
    <section id="about" aria-labelledby="about-heading">
      <h2 id="about-heading">{copy.aboutTitle}</h2>
      <p>{copy.about}</p>
    </section>
    <section id="how-it-works" aria-labelledby="how-heading">
      <h2 id="how-heading">{copy.howTitle}</h2>
      <ol>{#each copy.steps as step}<li>{step}</li>{/each}</ol>
    </section>
  </div>

  <section aria-labelledby="coverage-heading">
    <h2 id="coverage-heading">{copy.coverageTitle}</h2>
    <p>{copy.currencies}</p>
    <p>{copy.sources}</p>
    <h3>{copy.examplesTitle}</h3>
    <div class="examples">
      {#each copy.examples as example}
        <article><h4>{example.route}</h4><p>{example.explanation}</p></article>
      {/each}
    </div>
  </section>

  <section id="methodology" aria-labelledby="method-heading">
    <h2 id="method-heading">{copy.methodTitle}</h2>
    {#each copy.method as paragraph}<p>{paragraph}</p>{/each}
  </section>

  <section aria-labelledby="faq-heading">
    <h2 id="faq-heading">{copy.faqTitle}</h2>
    {#each copy.faq as item}
      <details><summary>{item.question}</summary><p>{item.answer}</p></details>
    {/each}
  </section>

  <section id="project" aria-labelledby="project-heading">
    <h2 id="project-heading">{copy.projectTitle}</h2>
    <p>{copy.project}</p>
    <div class="projectLinks">
      <a href={PROJECT_URL} target="_blank" rel="noreferrer noopener">{copy.github} ↗</a>
      <a href={COMMUNITY_URL} target="_blank" rel="noreferrer noopener">{copy.telegram} ↗</a>
      <a href="/terms">{copy.terms} →</a>
    </div>
  </section>
</div>

<style>
  .overview { width: min(var(--layout-width), calc(100% - 2 * var(--page-gutter))); margin: 64px auto 0; color: var(--color-text); }
  section { padding: 32px 0; border-top: 1px solid var(--color-border); scroll-margin-top: 24px; }
  .introduction { display: grid; grid-template-columns: 1fr 1fr; gap: 48px; }
  h2 { margin: 0 0 16px; font-size: clamp(22px, 2.5vw, 28px); font-weight: 650; line-height: 1.3; letter-spacing: -.025em; text-wrap: balance; }
  h3 { margin: 28px 0 16px; font-size: 18px; font-weight: 650; }
  h4 { margin: 0 0 12px; font-size: 16px; font-weight: 650; }
  p, li { max-width: 85ch; font-size: 16px; line-height: 1.7; color: var(--color-text-soft); }
  p { margin: 0 0 16px; }
  p:last-child { margin-bottom: 0; }
  ol { margin: 0; padding-left: 24px; }
  li { padding-left: 4px; margin-bottom: 12px; }
  li::marker { color: var(--color-accent-text); font-weight: 650; }
  .examples { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; }
  article { padding: 20px; border: 1px solid var(--color-border); border-radius: 13px; background: var(--color-panel); }
  article p { font-size: 14px; }
  details { border-bottom: 1px solid var(--color-border); }
  summary { padding: 16px 0; cursor: pointer; font-size: 16px; font-weight: 600; line-height: 1.5; }
  summary::marker { color: var(--color-accent-text); }
  details p { padding-bottom: 20px; }
  a { display: inline-flex; align-items: center; min-height: 44px; color: var(--color-accent-text); font-size: 14px; line-height: 1.5; text-decoration: underline; text-underline-offset: 4px; }
  .projectLinks { display: flex; flex-wrap: wrap; column-gap: 24px; row-gap: 8px; }
  @media (max-width: 980px) { .introduction { grid-template-columns: 1fr; gap: 0; } .overview { margin-top: 44px; } }
  @media (max-width: 640px) { .examples { grid-template-columns: 1fr; } section { padding: 24px 0; } }
</style>
