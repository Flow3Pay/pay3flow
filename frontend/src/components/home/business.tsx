"use client";

import { useI18n } from "@/i18n/context";

import styles from "./business.module.css";

export function Business() {
  const { d } = useI18n();

  return (
    <section className={styles.section} id="business">
      <div className={styles.inner}>
        <div className={styles.text}>
          <h2 className={styles.heading}>{d.business.heading}</h2>
          <p className={styles.sub}>{d.business.sub}</p>
          <a className={styles.cta} href={d.business.ctaHref}>
            {d.business.cta}
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path d="M3 8h10m0 0L9 4m4 4-4 4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </a>
        </div>
        <div className={styles.stats}>
          {d.business.stats.map((stat) => (
            <div key={stat.value + stat.label} className={styles.stat}>
              <span className={styles.statValue}>{stat.value}</span>
              <span className={styles.statLabel}>{stat.label}</span>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}