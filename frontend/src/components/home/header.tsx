"use client";

import styles from "./header.module.css";

interface HeaderProps {
  brandHref?: string;
}

export function Header({
  brandHref = "/",
}: HeaderProps) {
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

        <div className={styles.actions}>
          <span className={styles.marketStatus}>
            <span className={styles.pulse} />
            Markets live
          </span>
          <a
            className={styles.githubLink}
            href="https://github.com/Flow3Pay/pay3flow"
            target="_blank"
            rel="noreferrer noopener"
            aria-label="Open Pay3Flow on GitHub"
          >
            <img
              src="https://cdn-icons-png.flaticon.com/512/2111/2111432.png"
              alt=""
            />
          </a>
        </div>
      </div>
    </header>
  );
}
