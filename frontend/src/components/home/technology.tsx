"use client";

import { useI18n } from "@/i18n/context";

import styles from "./technology.module.css";

export function Technology() {
  const { d } = useI18n();

  return (
    <section className={styles.section} id="tech">
      <div className={styles.inner}>
        <div className={styles.head}>
          <span className={styles.label}>{d.why.label}</span>
          <h2 className={styles.heading}>{d.why.heading}</h2>
          <p className={styles.sub}>{d.why.sub}</p>
        </div>
        <div className={styles.links}>
          {d.why.cards.map((card) => (
            <a key={card.title} href={card.href} className={styles.link}>
              <span className={styles.linkTitle}>{card.title}</span>
              <span className={styles.linkText}>{card.text}</span>
              <span className={styles.linkArrow}>
                <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                  <path d="M3 8h10m0 0L9 4m4 4-4 4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              </span>
            </a>
          ))}
        </div>
      </div>
    </section>
  );
}