"use client";

import { useEffect, useRef, useState } from "react";

import { useI18nSafe } from "@/i18n/context";

import styles from "./header.module.css";

interface HeaderProps {
  connected: boolean;
  address: string;
  onConnect: () => void;
  onDisconnect: () => void;
  brandHref?: string;
}

export function Header({
  connected,
  address,
  onConnect,
  onDisconnect,
  brandHref = "/",
}: HeaderProps) {
  const { d } = useI18nSafe();
  const [walletOpen, setWalletOpen] = useState(false);
  const walletRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!walletOpen) return;
    const onClickOutside = (event: MouseEvent) => {
      if (walletRef.current && !walletRef.current.contains(event.target as Node)) {
        setWalletOpen(false);
      }
    };
    document.addEventListener("mousedown", onClickOutside);
    return () => document.removeEventListener("mousedown", onClickOutside);
  }, [walletOpen]);

  return (
    <header className={`${styles.header} ${styles.bar}`}>
      <div className={styles.inner}>
        <div className={styles.left}>
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

        <div className={styles.actions}>
          {connected ? (
            <div
              className={styles.walletWrap}
              ref={walletRef}
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
        </div>
      </div>
    </header>
  );
}
