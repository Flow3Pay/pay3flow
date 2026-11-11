<script lang="ts">
  import { onMount, tick } from "svelte";
  import { goto } from "$app/navigation";
  import { authenticate, requestAuthCode } from "$lib/exchange";
  import { readAuthToken, storeAuthToken } from "$lib/auth-session";

  type Step = "email" | "code" | "success";

  let step: Step = "email";
  let email = "";
  let code = "";
  let accepted = false;
  let pending = false;
  let error = "";
  let emailInput: HTMLInputElement | undefined;
  let codeInput: HTMLInputElement | undefined;
  let nextTarget: string | null = null;
  let referralCode: string | undefined;

  const devMode = import.meta.env.DEV;
  const devCode = devMode ? import.meta.env.PUBLIC_AUTH_DEV_CODE ?? "" : "";
  // The buttons stay hidden until the backend OAuth flow exists (§4.5): a
  // button that pretends to work is worse than no button.
  const oauthEnabled = import.meta.env.PUBLIC_AUTH_OAUTH === "1";
  const emailPattern = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  const codePattern = /^\d{6}$/;

  function safeNext(value: string | null): string | null {
    if (!value) return null;
    // Same-origin paths only: no absolute URL, protocol-relative URL or
    // backslash trick can turn the redirect into an open redirect.
    if (!value.startsWith("/") || value.startsWith("//") || value.startsWith("/\\")) {
      return null;
    }
    return value;
  }

  onMount(() => {
    const params = new URLSearchParams(window.location.search);
    nextTarget = safeNext(params.get("next"));
    const ref = params.get("ref")?.trim() ?? "";
    referralCode = /^[a-f0-9]{16}$/i.test(ref) ? ref.toUpperCase() : undefined;
    if (readAuthToken()) {
      void goto(nextTarget ?? "/business", { replaceState: true });
    }
  });

  async function requestCode() {
    error = "";
    if (!emailPattern.test(email.trim())) {
      error = "Введите корректный email.";
      emailInput?.focus();
      return;
    }
    if (!accepted) {
      error = "Подтвердите согласие с условиями.";
      return;
    }
    pending = true;
    try {
      await requestAuthCode(email.trim());
      code = "";
      step = "code";
      void tick().then(() => codeInput?.focus());
    } catch (cause) {
      error = cause instanceof Error && cause.message.includes("(429)")
        ? "Слишком много запросов. Попробуйте через минуту."
        : "Не удалось отправить код. Попробуйте ещё раз.";
    } finally {
      pending = false;
    }
  }

  async function submitCode() {
    error = "";
    if (!codePattern.test(code.trim())) {
      error = "Введите шестизначный код из письма.";
      codeInput?.focus();
      return;
    }
    pending = true;
    try {
      const token = await authenticate(email.trim(), code.trim(), referralCode);
      storeAuthToken(token);
      step = "success";
    } catch {
      error = "Не удалось подтвердить код. Проверьте его и попробуйте снова.";
    } finally {
      pending = false;
    }
  }

  function changeEmail() {
    step = "email";
    code = "";
    error = "";
    void tick().then(() => emailInput?.focus());
  }
</script>

<svelte:head>
  <title>Регистрация — Pay3Flow</title>
  <meta
    name="description"
    content="Создайте аккаунт Pay3Flow или войдите в существующий. Один аккаунт для всех маршрутов."
  />
</svelte:head>

<div class="authPage" lang="ru" data-no-scramble>
  <header class="authTop">
    <a class="brand" href="/" aria-label="Pay3Flow — на главную">
      <img class="brandMark" src="/icons/assets/pay3flow-mark.svg" alt="" width="38" height="38" />
      <span class="brandName">Pay3Flow</span>
    </a>
  </header>

  <main class="authMain">
    <section class="authHero" aria-labelledby="auth-title">
      <h1 class="authTitle" id="auth-title">
        <span class="titleLine">Один аккаунт.</span>
        <span class="titleLine titleAccent">Все маршруты.</span>
      </h1>

      <div class="routes" aria-label="Примеры быстрых маршрутов">
        <article class="routeCard">
          <p class="routeLabel">БЫСТРЫЙ МАРШРУТ</p>
          <p class="routeAmount">100&nbsp;000&nbsp;₽</p>
          <div class="routeDivider"></div>
          <div class="routePair">
            <span class="assetChip"><img src="/icons/assets/rub.svg" alt="" width="26" height="26" />RUB</span>
            <span class="pairArrow" aria-hidden="true">→</span>
            <span class="assetChip"><img src="/icons/assets/usdt.webp" alt="" width="26" height="26" />USDT</span>
          </div>
        </article>

        <article class="routeCard routeCardSecond">
          <p class="routeLabel">БЫСТРЫЙ МАРШРУТ</p>
          <p class="routeAmount">1148.20 USDT</p>
          <div class="routeDivider"></div>
          <div class="routePair">
            <span class="assetChip"><img src="/icons/assets/usdt.webp" alt="" width="26" height="26" />USDT</span>
            <span class="pairArrow" aria-hidden="true">→</span>
            <span class="assetChip"><img src="/icons/assets/amd.svg" alt="" width="26" height="26" />AMD</span>
          </div>
        </article>
      </div>
    </section>

    <section class="authCard" aria-labelledby="auth-card-title">
      <p class="eyebrow">
        {step === "success" ? "ГОТОВО" : step === "code" ? "ПОДТВЕРДИТЕ EMAIL" : "БЕСПЛАТНАЯ РЕГИСТРАЦИЯ"}
      </p>

      {#if step === "email"}
        <h2 id="auth-card-title">Создайте аккаунт</h2>
        <p class="lede">Начните искать лучшие маршруты уже сегодня</p>

        <form on:submit|preventDefault={requestCode} novalidate>
          <label class="field">
            <span>Email</span>
            <span class="inputWrap">
              <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <rect x="3" y="5" width="18" height="14" rx="2" stroke="currentColor" stroke-width="1.7"></rect>
                <path d="m4 7 8 6 8-6" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"></path>
              </svg>
              <input
                bind:this={emailInput}
                bind:value={email}
                type="email"
                name="email"
                autocomplete="email"
                inputmode="email"
                placeholder="name@example.com"
                aria-invalid={error ? "true" : undefined}
              />
            </span>
          </label>

          <label class="consent">
            <input bind:checked={accepted} type="checkbox" name="terms" />
            <span class="checkBox" aria-hidden="true">
              <svg viewBox="0 0 16 16" fill="none">
                <path d="m3.2 8.2 3.1 3.1 6.5-6.6" stroke="#0a0a0a" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"></path>
              </svg>
            </span>
            <span class="consentText">
              Я принимаю <a href="/terms" on:click|stopPropagation>Условия использования</a> и
              <a href="/terms#privacy" on:click|stopPropagation>Политику конфиденциальности</a>
            </span>
          </label>

          {#if error}<p class="error" role="alert">{error}</p>{/if}

          <button class="primaryButton" type="submit">
            Создать аккаунт <span aria-hidden="true">→</span>
          </button>
        </form>

        {#if oauthEnabled}
          <div class="divider"><span>или продолжить через</span></div>

          <div class="socialRow">
            <a class="socialButton" href="/api/auth/oauth/github/start">
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path fill="currentColor" d="M12 .7a11.3 11.3 0 0 0-3.58 22.02c.57.1.78-.25.78-.55v-2.16c-3.18.7-3.85-1.34-3.85-1.34-.52-1.32-1.27-1.67-1.27-1.67-1.04-.71.08-.7.08-.7 1.15.08 1.76 1.18 1.76 1.18 1.02 1.75 2.68 1.24 3.34.95.1-.74.4-1.24.73-1.53-2.54-.29-5.2-1.27-5.2-5.65 0-1.25.45-2.26 1.18-3.06-.12-.29-.51-1.45.11-3.02 0 0 .96-.31 3.12 1.17a10.8 10.8 0 0 1 5.68 0c2.16-1.48 3.12-1.17 3.12-1.17.62 1.57.23 2.73.11 3.02.73.8 1.18 1.81 1.18 3.06 0 4.39-2.67 5.35-5.21 5.64.41.36.78 1.08.78 2.18v3.23c0 .3.2.65.79.54A11.3 11.3 0 0 0 12 .7Z"></path>
              </svg>
              GitHub
            </a>
            <a class="socialButton" href="/api/auth/oauth/telegram/start">
              <img src="/icons/assets/telegram-messenger.png" alt="" width="22" height="22" />
              Telegram
            </a>
          </div>
        {/if}
      {:else if step === "code"}
        <h2 id="auth-card-title">Введите код</h2>
        <p class="lede">Код подтверждения для {email.trim()}</p>

        <form on:submit|preventDefault={submitCode} novalidate>
          <label class="field">
            <span>Код из письма</span>
            <span class="inputWrap inputWrapCode">
              <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <rect x="5" y="10" width="14" height="10" rx="2" stroke="currentColor" stroke-width="1.7"></rect>
                <path d="M8 10V7a4 4 0 0 1 8 0v3" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"></path>
              </svg>
              <input
                bind:this={codeInput}
                bind:value={code}
                type="text"
                name="code"
                autocomplete="one-time-code"
                inputmode="numeric"
                pattern="[0-9]*"
                maxlength="6"
                placeholder="000000"
                aria-invalid={error ? "true" : undefined}
              />
            </span>
          </label>

          {#if devCode}<p class="devHint">Демо-режим: код <strong>{devCode}</strong></p>{/if}
          {#if error}<p class="error" role="alert">{error}</p>{/if}

          <button class="primaryButton" type="submit" disabled={pending}>
            {pending ? "Проверяем…" : "Продолжить"} <span aria-hidden="true">→</span>
          </button>
        </form>

        <button class="changeEmail" type="button" on:click={changeEmail}>Изменить email</button>
      {:else}
        <h2 id="auth-card-title">Вы в аккаунте</h2>
        <p class="lede">Аккаунт {email.trim()} готов. Теперь маршруты и настройки будут сохраняться.</p>
        <a class="primaryButton" href={nextTarget ?? "/business"}>
          Продолжить <span aria-hidden="true">→</span>
        </a>
      {/if}
    </section>
  </main>
</div>

<style>
  .authPage {
    --lime: #b5f500;
    --lime-soft: #d9ff70;
    --ink: #f6f6f2;
    --muted: #8f918b;
    --card: #111211;
    --field: #191a19;
    --line: #2a2c29;
    display: flex;
    min-height: 100dvh;
    flex-direction: column;
    padding: clamp(20px, 3vw, 42px) clamp(20px, 5vw, 88px) clamp(36px, 5vw, 64px);
    background: #090a09;
    color: var(--ink);
  }

  .authTop {
    width: min(1280px, 100%);
    margin: 0 auto;
  }

  .brand {
    display: inline-flex;
    align-items: center;
    gap: 12px;
  }

  .brandMark {
    display: block;
    flex: 0 0 auto;
  }

  .brandName {
    font-size: 22px;
    font-weight: 800;
    letter-spacing: -0.045em;
  }

  .authMain {
    display: grid;
    width: min(1280px, 100%);
    flex: 1;
    grid-template-columns: minmax(0, 1fr) minmax(390px, 500px);
    align-items: center;
    gap: clamp(36px, 6vw, 96px);
    margin: 0 auto;
    padding: clamp(44px, 7vh, 90px) 0 clamp(48px, 8vh, 100px);
  }

  .authHero {
    min-width: 0;
  }

  .authTitle {
    max-width: 760px;
    font-size: clamp(48px, 6.2vw, 96px);
    font-weight: 800;
    letter-spacing: -0.06em;
    line-height: 0.98;
  }

  .titleLine {
    display: block;
  }

  .titleAccent {
    width: fit-content;
    padding: 0 0.04em 0.02em;
    margin-left: -0.04em;
    background-image: linear-gradient(var(--lime), var(--lime));
    background-position: -1% 91%;
    background-repeat: no-repeat;
    background-size: 102% 0.12em;
  }

  .routes {
    display: grid;
    width: min(100%, 680px);
    grid-template-columns: repeat(12, minmax(0, 1fr));
    margin-top: clamp(34px, 5.5vh, 64px);
  }

  .routeCard {
    grid-row: 1;
    padding: clamp(18px, 2vw, 26px) clamp(18px, 2.2vw, 28px);
    border: 1px solid #272927;
    border-radius: 15px;
    background: var(--card);
  }

  .routeCard:first-child {
    grid-column: 1 / 8;
  }

  .routeCardSecond {
    z-index: 1;
    grid-column: 6 / 13;
    margin-top: clamp(76px, 11vh, 128px);
  }

  .routeLabel {
    color: #8b8d87;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.12em;
  }

  .routeAmount {
    margin-top: 12px;
    font-size: clamp(24px, 2vw, 30px);
    font-weight: 800;
    letter-spacing: -0.045em;
    white-space: nowrap;
  }

  .routeDivider {
    height: 1px;
    margin: 18px 0 14px;
    background: var(--line);
  }

  .routePair {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .assetChip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: #d9dad5;
    font-size: 13px;
    font-weight: 700;
  }

  .assetChip img {
    display: block;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    object-fit: cover;
  }

  .pairArrow {
    color: #777973;
    font-size: 15px;
  }

  .authCard {
    width: 100%;
    padding: clamp(26px, 3.2vw, 40px);
    border: 1px solid #252725;
    border-radius: 24px;
    background: var(--card);
  }

  .eyebrow {
    color: var(--lime);
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.14em;
  }

  .authCard h2 {
    margin-top: 12px;
    font-size: clamp(28px, 2.5vw, 34px);
    font-weight: 700;
    letter-spacing: -0.045em;
    line-height: 1.1;
  }

  .lede {
    margin-top: 10px;
    color: var(--muted);
    font-size: 15px;
    line-height: 1.5;
  }

  .field {
    display: grid;
    gap: 9px;
    margin-top: 26px;
  }

  .field > span:first-child {
    color: #d8dad4;
    font-size: 12px;
    font-weight: 700;
  }

  .inputWrap {
    position: relative;
    display: block;
  }

  .inputWrap svg {
    position: absolute;
    top: 50%;
    left: 16px;
    width: 20px;
    height: 20px;
    color: #777973;
    pointer-events: none;
    transform: translateY(-50%);
  }

  .inputWrap input {
    width: 100%;
    height: 56px;
    padding: 0 16px 0 48px;
    border: 1px solid #30322f;
    border-radius: 12px;
    background: var(--field);
    color: var(--ink);
    font-size: 15px;
  }

  .inputWrap input::placeholder {
    color: #70726c;
  }

  .inputWrap input:focus {
    border-color: var(--lime);
    outline: none;
  }

  .inputWrapCode input {
    padding-left: 48px;
    font-family: var(--font-mono);
    letter-spacing: 0.28em;
  }

  .consent {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: 10px;
    margin: 18px 0 22px;
    color: #a2a49e;
    cursor: pointer;
    font-size: 12px;
    line-height: 1.55;
  }

  .consent input {
    position: absolute;
    width: 1px;
    height: 1px;
    opacity: 0;
  }

  .checkBox {
    display: grid;
    width: 18px;
    height: 18px;
    flex: 0 0 auto;
    margin-top: 1px;
    border: 1px solid #3a3d39;
    border-radius: 4px;
    background: #171816;
    place-items: center;
  }

  .checkBox svg {
    width: 13px;
    height: 13px;
    opacity: 0;
  }

  .consent input:checked + .checkBox {
    border-color: var(--lime);
    background: var(--lime);
  }

  .consent input:checked + .checkBox svg {
    opacity: 1;
  }

  .consent input:focus-visible + .checkBox {
    outline: 3px solid var(--lime-soft);
    outline-offset: 2px;
  }

  .consentText a {
    color: #d7d9d2;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .primaryButton {
    display: flex;
    width: 100%;
    height: 56px;
    align-items: center;
    justify-content: center;
    gap: 10px;
    border: 0;
    border-radius: 11px;
    background: var(--lime);
    color: #0a0b09;
    font-size: 15px;
    font-weight: 800;
    text-align: center;
  }

  .primaryButton:disabled {
    cursor: default;
    opacity: 0.65;
  }

  .primaryButton:focus-visible,
  .socialButton:focus-visible,
  .changeEmail:focus-visible,
  .brand:focus-visible {
    outline: 3px solid var(--lime-soft);
    outline-offset: 3px;
  }

  .divider {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 24px 0 18px;
    color: #70726c;
    font-size: 11px;
  }

  .divider::before,
  .divider::after {
    height: 1px;
    flex: 1;
    background: #292b28;
    content: "";
  }

  .socialRow {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .socialButton {
    display: flex;
    height: 54px;
    align-items: center;
    justify-content: center;
    gap: 9px;
    border: 1px solid #30322f;
    border-radius: 11px;
    background: #151615;
    color: #f0f1eb;
    font-size: 14px;
    font-weight: 700;
  }

  .socialButton svg,
  .socialButton img {
    display: block;
    width: 22px;
    height: 22px;
  }

  .socialNotice,
  .devHint {
    margin-top: 14px;
    color: #8f918b;
    font-size: 12px;
    line-height: 1.45;
  }

  .devHint strong {
    color: #d8dad4;
    font-family: var(--font-mono);
    letter-spacing: 0.14em;
  }

  .error {
    margin: 0 0 14px;
    color: #ff8f88;
    font-size: 12px;
    line-height: 1.45;
  }

  .changeEmail {
    display: block;
    margin: 18px auto 0;
    color: #a2a49e;
    font-size: 12px;
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  @media (max-width: 1100px) {
    .authMain {
      width: min(760px, 100%);
      grid-template-columns: 1fr;
      gap: 48px;
      padding-top: 48px;
    }

    .authCard {
      max-width: 560px;
      margin: 0 auto;
    }
  }

  @media (max-width: 680px) {
    .authPage {
      padding-right: 18px;
      padding-left: 18px;
    }

    .authTitle {
      font-size: clamp(42px, 12vw, 58px);
    }

    .routes {
      display: flex;
      flex-direction: column;
      gap: 12px;
      margin-top: 30px;
    }

    .routeCard {
      width: 100%;
    }

    .routeCardSecond {
      width: calc(100% - 20px);
      margin-top: 0;
      margin-left: 20px;
    }

    .authCard {
      padding: 24px 20px;
      border-radius: 18px;
    }
  }

  @media (max-width: 380px) {
    .socialRow {
      grid-template-columns: 1fr;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .authPage {
      scroll-behavior: auto;
    }
  }
</style>
