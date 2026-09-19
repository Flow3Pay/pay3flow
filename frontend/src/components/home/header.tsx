"use client";

import { useEffect, useRef, useState } from "react";

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
  const [profileOpen, setProfileOpen] = useState(false);
  const profileRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!profileOpen) return;
    const onClickOutside = (event: MouseEvent) => {
      if (profileRef.current && !profileRef.current.contains(event.target as Node)) {
        setProfileOpen(false);
      }
    };
    document.addEventListener("mousedown", onClickOutside);
    return () => document.removeEventListener("mousedown", onClickOutside);
  }, [profileOpen]);

  return (
    <header className={styles.header}>
      <div className={styles.inner}>
        <a href={brandHref} className={styles.brand} aria-label="Pay3Flow home">
          <span className={styles.logo} aria-hidden="true">
            <svg width="26" height="26" viewBox="0 0 26 26" fill="none">
              <path d="M4 7.25 13 2l9 5.25v11.5L13 24l-9-5.25V7.25Z" fill="currentColor" />
              <path d="m8.2 10.2 4.8-2.8 4.8 2.8-4.8 2.8-4.8-2.8Zm0 5.3 4.8 2.8 4.8-2.8" stroke="#171a17" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </span>
          <span className={styles.wordmark}>Pay3Flow</span>
          <span className={styles.beta}>Beta</span>
        </a>

        <nav className={styles.nav} aria-label="Primary navigation">
          <a className={styles.navActive} href="#transfer">Transfer</a>
          <a href="#routes">Live routes</a>
          <a href="#how-it-works">How it works</a>
        </nav>

        <div className={styles.actions}>
          <span className={styles.marketStatus}>
            <span className={styles.pulse} />
            Markets live
          </span>
          {connected ? (
            <div className={styles.profileWrap} ref={profileRef}>
              <button
                type="button"
                className={styles.profile}
                aria-expanded={profileOpen}
                onClick={() => setProfileOpen((value) => !value)}
              >
                <span className={styles.profileAvatar}>{address.slice(0, 1).toUpperCase()}</span>
                <span className={styles.profileAddress}>{address}</span>
                <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                  <path d="m4 6 4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              </button>
              {profileOpen && (
                <div className={styles.menu} role="menu">
                  <div className={styles.menuIdentity}>
                    <span>Signed in as</span>
                    <strong>{address}</strong>
                  </div>
                  <button type="button" className={styles.menuItem} role="menuitem" onClick={onDisconnect}>
                    Sign out
                  </button>
                </div>
              )}
            </div>
          ) : (
            <button type="button" className={styles.connect} onClick={onConnect}>
              Sign in
              <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <path d="M3 8h10M9 4l4 4-4 4" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
          )}
        </div>
      </div>
    </header>
  );
}
