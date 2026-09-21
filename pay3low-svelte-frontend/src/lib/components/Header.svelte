<script lang="ts">
  const moonIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAACWklEQVR4AbTVTYhNYRzH8TPCRs0kLxsWIvIywkrykvISTZRkbGiSQsqCIkuWyobkpZkiL5PQ2Eyk2BBGErOaZCMz0giRrEyNz/90z3XvzL23e6Zm+n3v/zz/5/n/f8957plzJyTj/FfTYHh4eDM60Il5Y9lLRQPN5uOyho+wH7GuQ8ytKCwr0ni7xHMcxGOsxW68QW6VGWi+TofrmIGzDQ0Nm8QpCL2Oj7wUDTRvVnwVTRqHjrsOLYoPvEJuFQ1U7sFc7ESplhoMcvwo5lZqYPcTVR5Ap0ZdYqmmG/RjTEoNVLZhKi5hpOILXzAyWe84M8jO+VOFwndyje4yW2NYvzKD2P0fx1PJoLfQLp6owmX9odRgoFIZ00H5CzjnLhaKuVRq8LdaJZMj5n7hIXIpM4jCZjuMx7Rag30m5lgzgOWu61Jm8KywuqUQRwV3cV9yI2bhLZMTmO26plIDxS+t+ow1qCrrnpichms4g34mvWjHqRLWm0uVGqRXSXJHbLWo6l2YT5j8QBzXLuMw+SbGy/CwuA0Rb4upigaKjsnE+6aLSfxnG1aX9fdwEhvQiJlW78VX9CFV0SAdJckWcTKeIpdsarGCOIUl4kWkKjOwi5+yq7FKwW/EC9CwtqzbakXWvFWfu8apygwiY/KFOAm3cENxfHktYhyB1H/JrcRNmQdoQllz42SUQSSZDOGQ66BV7MagZu/Rgw/4LteDZYjfjhVqijuXS1XRIJ3xoeAK4myDeJ1Hwy+mwvC0GE9NNI5fv3iapMpV0yBbyqQP7WjDDhzFeXRjKFtXKf4DAAD///Lx6McAAAAGSURBVAMASQPNMX2ya7kAAAAASUVORK5CYII=";
  const sunIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAACD0lEQVR4AbSTPS8FQRSG92o0+Ae0EkKDhk58NJQUoiASlUoQKolCoySESCQaCdEp0BGREBQK4W+goJDreSdnstmdXXe3cHOenDNn3nfO7t7duuiff6UGVKvVWVHmmkoNKHOw15YaUKlU9oQ3F8nBAB7BBSxlmem3ipy9ZfbO03vBAARX8AkuMOm535DfabwK1aDeLGsf8lz7hc/BAB7BOmxLwCEP5F1ogn2YMFSrt2uaCM8WrLOfiGCA38V4TN0Fl9CLeR6OjHn1QHtdpmUZRuYADCNIx0BXNcyhun2WcagHw3S2YMw8lMnIHIBkBZ5gAWqFNNLKE2jzBnSivOcKv8l/hmnuEclDSoYbwO3pTVnUFnUbuQHuoGhI22DeiLwI7g1zA1Kn/Ni65tWbTslrvVc9hxvAbeoL3VCH+o38AT1QNKT9MK9e2Q3qPZndABUpnlkPcJt6VJT5YZoBFPKQkpE34BBZB5xCrZBGWnkCbeYAu71H1ENc4SbUUydCPdikOQSP5qFMRuYASTB0k09gDm45bAemjB31QHsnpmUZRjCAA1ZgWlKM4+RR+IJJODBUqzdqGr2aM/iW2U9EMIDdfmgGFxxwBn3QSKNdqAb1zlj7aKGQlxRHMADjIKzFkrii/yLiTlzRXwX9H3GTKhhALzd4BPri3ReaK0ptlBqQ8hZalhrAI9AX777QQqdHUfQLAAD//91ClIsAAAAGSURBVAMAR3zSMQ+aPXkAAAAASUVORK5CYII=";

  function toggleTheme() {
    const root = document.documentElement;
    const nextTheme = root.dataset.theme === "dark" ? "light" : "dark";
    root.dataset.theme = nextTheme;
    localStorage.setItem("pay3flow-theme", nextTheme);
  }
</script>

<header class="header">
  <div class="inner">
    <a href="/" class="brand" aria-label="Pay3Flow home">
      <span class="logo" aria-hidden="true">
        <svg width="26" height="26" viewBox="0 0 26 26" fill="none">
          <path d="M4 7.25 13 2l9 5.25v11.5L13 24l-9-5.25V7.25Z" fill="currentColor" />
          <path d="m8.2 10.2 4.8-2.8 4.8 2.8-4.8 2.8-4.8-2.8Zm0 5.3 4.8 2.8 4.8-2.8" stroke="#171a17" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </span>
      <span class="wordmark">Pay3Flow</span>
      <span class="beta">Beta</span>
    </a>
    <div class="actions">
      <span class="marketStatus"><span class="pulse"></span>Markets live</span>
      <button class="themeToggle" type="button" on:click={toggleTheme} aria-label="Переключить цветовую тему" title="Переключить тему">
        <img class="moonIcon" src={moonIcon} alt="" />
        <img class="sunIcon" src={sunIcon} alt="" />
      </button>
      <a class="githubLink" href="https://github.com/Flow3Pay/pay3flow" target="_blank" rel="noreferrer noopener" aria-label="Open Pay3Flow on GitHub">
        <img src="https://cdn-icons-png.flaticon.com/512/2111/2111432.png" alt="" />
      </a>
    </div>
  </div>
</header>

<style>
.header {
  position: relative;
  z-index: 50;
  width: 100%;
  padding: 18px 24px;
}

.inner {
  display: grid;
  width: 100%;
  min-height: 66px;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 24px;
  margin: 0 auto;
  padding: 8px 10px 8px 18px;
  border: 1px solid rgba(255, 255, 255, 0.7);
  border-radius: 22px;
  background: rgba(255, 255, 255, 0.72);
  box-shadow: 0 9px 30px rgba(31, 34, 29, 0.07);
  backdrop-filter: blur(24px) saturate(150%);
  -webkit-backdrop-filter: blur(24px) saturate(150%);
}

.brand,
.actions,
.profile,
.marketStatus {
  display: flex;
  align-items: center;
}

.brand {
  width: fit-content;
  gap: 9px;
}

.logo {
  display: grid;
  width: 34px;
  height: 34px;
  place-items: center;
  border-radius: 11px;
  background: var(--color-accent);
  color: var(--color-accent);
  box-shadow: 0 8px 18px rgba(145, 190, 20, 0.2);
}

.wordmark {
  font-size: 17px;
  font-weight: 800;
  letter-spacing: -0.045em;
}

.beta {
  padding: 4px 7px;
  border-radius: var(--radius-pill);
  background: var(--color-violet-soft);
  color: var(--color-violet);
  font-size: 9px;
  font-weight: 800;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.actions {
  justify-content: flex-end;
  gap: 9px;
}

.githubLink,
.themeToggle {
  display: grid;
  width: 36px;
  height: 36px;
  place-items: center;
  border: 1px solid var(--color-border);
  border-radius: 11px;
  background: rgba(255, 255, 255, 0.72);
  transition: border-color 0.16s ease, background 0.16s ease, transform 0.16s ease;
}

.githubLink img,
.themeToggle img {
  display: block;
  width: 20px;
  height: 20px;
  object-fit: contain;
}

.githubLink:hover,
.themeToggle:hover {
  border-color: var(--color-accent-strong);
  background: var(--color-accent-soft);
  transform: translateY(-1px);
}

.themeToggle {
  position: relative;
  overflow: hidden;
}

.themeToggle img {
  position: absolute;
  transition: opacity 0.2s ease, transform 0.25s ease;
}

.moonIcon {
  filter: brightness(0);
}

.sunIcon {
  opacity: 0;
  transform: rotate(-45deg) scale(0.65);
}

:global(html[data-theme="dark"]) .moonIcon {
  opacity: 0;
  transform: rotate(45deg) scale(0.65);
}

:global(html[data-theme="dark"]) .sunIcon {
  opacity: 1;
  filter: brightness(0) invert(1);
  transform: rotate(0) scale(1);
}

:global(html[data-theme="dark"]) .inner {
  border-color: var(--color-border-strong);
  background: rgba(25, 25, 25, 0.92);
  box-shadow: 0 14px 36px rgba(0, 0, 0, 0.25);
}

:global(html[data-theme="dark"]) .logo {
  background: #080808;
}

:global(html[data-theme="dark"]) .beta {
  color: var(--color-accent);
}

:global(html[data-theme="dark"]) .githubLink,
:global(html[data-theme="dark"]) .themeToggle,
:global(html[data-theme="dark"]) .profile {
  border-color: var(--color-border-strong);
  background: #262626;
}

:global(html[data-theme="dark"]) .githubLink img {
  filter: invert(1) brightness(1.35);
}

:global(html[data-theme="dark"]) .menu {
  background: rgba(25, 25, 25, 0.98);
}

.marketStatus {
  gap: 8px;
  padding: 0 10px;
  color: var(--color-text-soft);
  font-size: 10px;
  font-weight: 750;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}

.pulse {
  position: relative;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--color-good);
  box-shadow: 0 0 0 4px rgba(45, 142, 69, 0.12);
}

.pulse::after {
  position: absolute;
  inset: -4px;
  border: 1px solid rgba(45, 142, 69, 0.45);
  border-radius: 50%;
  content: "";
  animation: pulse 2s ease-out infinite;
}

.connect,
.profile {
  min-height: 48px;
  border-radius: 15px;
  font-size: 12px;
  font-weight: 750;
}

.connect {
  display: inline-flex;
  align-items: center;
  gap: 9px;
  padding: 0 18px;
  background: var(--color-primary);
  color: #fff;
  box-shadow: 0 8px 18px rgba(12, 14, 12, 0.15);
  transition: transform 0.16s ease, background 0.16s ease;
}

.connect:hover {
  background: #000;
  transform: translateY(-1px);
}

.profileWrap {
  position: relative;
}

.profile {
  max-width: 230px;
  gap: 9px;
  padding: 5px 12px 5px 5px;
  border: 1px solid var(--color-border);
  background: rgba(255, 255, 255, 0.72);
}

.profileAvatar {
  display: grid;
  width: 36px;
  height: 36px;
  flex: 0 0 auto;
  place-items: center;
  border-radius: 11px;
  background: linear-gradient(145deg, var(--color-violet), #9b83ff);
  color: #fff;
  font-size: 12px;
}

.profileAddress {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.menu {
  position: absolute;
  top: calc(100% + 10px);
  right: 0;
  width: 240px;
  padding: 8px;
  border: 1px solid var(--color-border);
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.96);
  box-shadow: var(--shadow-pop);
  backdrop-filter: blur(20px);
  animation: menuIn 0.18s ease;
}

.menuIdentity {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 11px 12px 13px;
  border-bottom: 1px solid var(--color-border);
}

.menuIdentity span {
  color: var(--color-text-faint);
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
}

.menuIdentity strong {
  overflow: hidden;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.menuItem {
  width: 100%;
  margin-top: 6px;
  padding: 10px 12px;
  border-radius: 11px;
  color: var(--color-danger);
  font-size: 12px;
  font-weight: 700;
  text-align: left;
}

.menuItem:hover {
  background: rgba(212, 61, 53, 0.07);
}

@keyframes pulse {
  from { opacity: 0.8; transform: scale(0.6); }
  to { opacity: 0; transform: scale(1.8); }
}

@keyframes menuIn {
  from { opacity: 0; transform: translateY(-5px) scale(0.98); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

@media (max-width: 860px) {
  .marketStatus {
    display: none;
  }
}

@media (max-width: 560px) {
  .header {
    padding: 12px;
  }

  .inner {
    min-height: 58px;
    padding: 6px 7px 6px 13px;
    border-radius: 18px;
  }

  .beta,
  .profileAddress {
    display: none;
  }

  .profile {
    padding-right: 8px;
  }

  .wordmark {
    font-size: 15px;
  }
}

/* Shared visual language for the light workspace. Existing transitions and pulse motion remain intact. */
.header {
  padding: 16px 24px 8px;
}

.inner {
  min-height: 58px;
  padding: 6px 8px 6px 14px;
  border-color: var(--color-border-strong);
  border-radius: 13px;
  background: rgba(255, 255, 255, 0.86);
  box-shadow: 0 10px 26px rgba(55, 77, 52, 0.06);
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}

.logo {
  width: 34px;
  height: 34px;
  border-radius: 9px;
  background: var(--color-primary);
  color: var(--color-accent);
  box-shadow: none;
}

.beta {
  background: var(--color-accent-soft);
  color: #668600;
}

.connect {
  min-height: 40px;
  border-radius: 9px;
  background: var(--color-primary);
  box-shadow: 0 7px 16px rgba(19, 32, 21, 0.12);
}

.connect:hover {
  background: var(--color-primary-strong);
}

.profile {
  min-height: 40px;
  border-radius: 9px;
  background: #f4f8f1;
}

.profileAvatar {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  background: var(--color-primary);
  color: var(--color-accent);
}

.menu {
  top: calc(100% + 8px);
  border-color: var(--color-border-strong);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.98);
  backdrop-filter: none;
}

</style>
