"use client";

import { useRef, useState } from "react";

import { useI18nSafe } from "@/i18n/context";

import styles from "./header.module.css";

export interface ChainOption {
  id: string;
  label: string;
  color: string;
}

const LANDING_LINKS: { href: string; labelKey: "exchange" | "payouts" | "pricing" }[] = [
  { href: "/swap", labelKey: "exchange" },
  { href: "/#how", labelKey: "payouts" },
  { href: "/#business", labelKey: "pricing" },
];

const SWAP_FALLBACK_LINKS: { href: string; label: string }[] = [
  { href: "/", label: "Pay3Flow" },
  { href: "/swap", label: "Exchange" },
];

const CHAINS: ChainOption[] = [
  { id: "ethereum", label: "Ethereum", color: "#627eea" },
  { id: "polygon", label: "Polygon", color: "#8247e5" },
  { id: "solana", label: "Solana", color: "#9945ff" },
  { id: "bitcoin", label: "Bitcoin", color: "#f7931a" },
  { id: "mx", label: "Mir", color: "#00a77f" },
];

interface HeaderProps {
  connected: boolean;
  address: string;
  onConnect: () => void;
  onDisconnect: () => void;
  links?: { href: string; label: string }[];
  brandHref?: string;
  landing?: boolean;
}

export function Header({
  connected,
  address,
  onConnect,
  onDisconnect,
  links = SWAP_FALLBACK_LINKS,
  brandHref = "#top",
  landing = false,
}: HeaderProps) {
  const { lang, setLang, d } = useI18nSafe();
  const [chainOpen, setChainOpen] = useState(false);
  const [chain, setChain] = useState<ChainOption>(CHAINS[0]);
  const [walletOpen, setWalletOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const landingLinks = landing
    ? LANDING_LINKS.map((l) => ({ href: l.href, label: d.header.nav[l.labelKey] }))
    : links.map((l) => ({ href: l.href, label: l.label }));

  const chooseChain = (option: ChainOption) => {
    setChain(option);
    setChainOpen(false);
  };

  const scheduleWalletClose = () => {
    if (closeTimer.current) clearTimeout(closeTimer.current);
    setWalletOpen(false);
  };

  const cancelWalletClose = () => {
    if (closeTimer.current) clearTimeout(closeTimer.current);
  };

  const switchLanguage = () => {
    setLang(lang === "en" ? "ru" : "en");
  };

  return (
    <header className={`${styles.header} ${styles.bar}`}>
      <div className={styles.inner}>
        <div className={styles.left}>
          {landing && (
            <button
              type="button"
              className={styles.navTrigger}
              aria-label={menuOpen ? d.header.mobileMenu.close : d.header.mobileMenu.open}
              aria-expanded={menuOpen}
              onClick={() => setMenuOpen((v) => !v)}
            >
              <svg width="17" height="17" viewBox="0 0 17 17" fill="currentColor" aria-hidden="true">
                {[2, 8.5, 15].flatMap((x) =>
                  [2, 8.5, 15].map((y) => <circle key={`${x}-${y}`} cx={x} cy={y} r="1.5" />)
                )}
              </svg>
            </button>
          )}

          <a href={brandHref} className={styles.brand}>
            <span className={styles.logo}>
              <svg width="30" height="30" viewBox="0 0 32 32" fill="none" aria-hidden="true">
                <rect x="1" y="1" width="30" height="30" rx="10" fill="var(--color-text)" />
                <path
                  d="M10 21V11m0 0 5 5m-5-5-5 5"
                  stroke="var(--color-accent)"
                  strokeWidth="2.2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <path
                  d="M22 11v10m0 0-5-5m5 5 5-5"
                  stroke="#ffffff"
                  strokeWidth="2.2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </span>
            <span className={styles.wordmark}>
              Pay3<span className={styles.wordmarkAccent}>Flow</span>
            </span>
          </a>
        </div>

        <nav className={styles.nav} aria-label={d.header.ariaNav}>
          {landingLinks.map((link) => (
            <a key={link.href} href={link.href}>
              {link.label}
            </a>
          ))}
        </nav>

        <div className={styles.actions}>
          {landing ? (
            <>
              <button
                type="button"
                className={styles.lang}
                aria-label={lang === "en" ? "Switch to Russian" : "Switch to English"}
                onClick={switchLanguage}
              >
                {lang === "en" ? "RU" : "EN"}
              </button>
              <a href="/swap" className={styles.launch}>
                {d.header.launch}
                <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                  <path d="M3 8h10m0 0L9 4m4 4-4 4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              </a>
            </>
          ) : (
            <>
              <div className={styles.networkWrap}>
                <button
                  type="button"
                  className={styles.network}
                  aria-haspopup="menu"
                  aria-expanded={chainOpen}
                  onClick={() => setChainOpen((v) => !v)}
                >
                  <span className={styles.chainDot} style={{ background: chain.color }} />
                  {chain.label}
                  <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                    <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                  </svg>
                </button>
                {chainOpen && (
                  <>
                    <div className={styles.overlay} onClick={() => setChainOpen(false)} />
                    <div className={styles.menu} role="menu">
                      {CHAINS.map((option) => (
                        <button
                          key={option.id}
                          type="button"
                          role="menuitem"
                          className={styles.menuItem}
                          onClick={() => chooseChain(option)}
                        >
                          <span className={styles.chainDot} style={{ background: option.color }} />
                          {option.label}
                          {option.id === chain.id && <span className={styles.menuCheck}>✓</span>}
                        </button>
                      ))}
                    </div>
                  </>
                )}
              </div>

              {connected ? (
                <div
                  className={styles.walletWrap}
                  onMouseLeave={scheduleWalletClose}
                  onMouseEnter={cancelWalletClose}
                >
                  <button
                    type="button"
                    className={styles.walletConnected}
                    aria-expanded={walletOpen}
                    onClick={() => setWalletOpen((v) => !v)}
                  >
                    <span className={styles.walletDot} />
                    {address}
                    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                      <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                    </svg>
                  </button>
                  {walletOpen && (
                    <div className={styles.menu} role="menu">
                      <button type="button" className={styles.menuItem} role="menuitem" onClick={onDisconnect}>
                        {d.header.wallet.disconnect}
                      </button>
                    </div>
                  )}
                </div>
              ) : (
                <button type="button" className={styles.connect} onClick={onConnect}>
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                    <path
                      d="M3 3h7a2 2 0 0 1 2 2v6a2 2 0 0 0 2 2H3V5"
                      stroke="currentColor"
                      strokeWidth="1.6"
                      strokeLinecap="round"
                    />
                    <circle cx="10" cy="9" r="1" fill="currentColor" />
                  </svg>
                  {d.header.wallet.connect}
                </button>
              )}

              <button
                type="button"
                className={styles.burger}
                aria-label={menuOpen ? d.header.mobileMenu.close : d.header.mobileMenu.open}
                aria-expanded={menuOpen}
                onClick={() => setMenuOpen((v) => !v)}
              >
                {menuOpen ? (
                  <svg width="22" height="22" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                    <path d="M6 6l12 12M18 6 6 18" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
                  </svg>
                ) : (
                  <svg width="22" height="22" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                    <path d="M4 7h16M4 12h16M4 17h16" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
                  </svg>
                )}
              </button>
            </>
          )}
        </div>
      </div>

      {menuOpen && (
        <>
          <div className={styles.menuOverlay} onClick={() => setMenuOpen(false)} />
          <div className={styles.mobileMenu} role="menu" aria-label={d.header.mobileMenu.ariaLabel}>
            {landingLinks.map((link) => (
              <a key={link.href} href={link.href} role="menuitem" onClick={() => setMenuOpen(false)}>
                {link.label}
              </a>
            ))}
          </div>
        </>
      )}
    </header>
  );
}