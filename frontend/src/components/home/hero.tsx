"use client";

import { useI18n } from "@/i18n/context";

import styles from "./hero.module.css";

export function Hero() {
  const { d } = useI18n();

  return (
    <section className={styles.hero} id="top">
      <div className={styles.heroInner}>
        <h1 className={styles.title}>{d.hero.title}</h1>
      </div>
    </section>
  );
}