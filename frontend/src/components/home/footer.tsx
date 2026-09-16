"use client";

import { useI18n } from "@/i18n/context";

import styles from "./footer.module.css";

export function Footer() {
  const { d } = useI18n();

  return (
    <footer className={styles.footer}>
      <div className={styles.inner}>
        <div className={styles.brand}>
          <a href="#top" className={styles.wordmark}>
            <span className={styles.logo}>
              <svg width="26" height="26" viewBox="0 0 32 32" fill="none" aria-hidden="true">
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
            <span>
              Pay3<span className={styles.accent}>Flow</span>
            </span>
          </a>
          <p className={styles.blurb}>{d.footer.blurb}</p>
        </div>

        <div className={styles.columns}>
          {d.footer.columns.map((column) => (
            <nav key={column.title} className={styles.column} aria-label={column.title}>
              <h4 className={styles.columnTitle}>{column.title}</h4>
              {column.links.map((link) => (
                <a key={link.label} href={link.href} className={styles.columnLink}>
                  {link.label}
                </a>
              ))}
            </nav>
          ))}
        </div>
      </div>

      <div className={styles.bottom}>
        <span>{d.footer.rights}</span>
        <span className={styles.version}>{d.footer.version}</span>
      </div>
    </footer>
  );
}