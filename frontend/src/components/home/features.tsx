"use client";

import { useI18n } from "@/i18n/context";

import styles from "./features.module.css";

type TagColor = "orange" | "purple" | "blue";
type Seg = string | [string, TagColor];

function HeadlineSegments({ segments }: { segments: Seg[] }) {
  return (
    <>
      {segments.map((segment, i) =>
        typeof segment === "string" ? (
          <span key={i}>{segment}</span>
        ) : (
          <span key={i} className={styles[segment[1]]}>
            {segment[0]}
          </span>
        )
      )}
    </>
  );
}

function NetworkArt() {
  return (
    <svg className={styles.cardImage} viewBox="0 0 446 236" fill="none" aria-hidden="true">
      <circle cx="118" cy="118" r="54" fill="#f0dede" />
      <circle cx="118" cy="118" r="40" fill="#65d9ff" />
      <circle cx="118" cy="118" r="18" fill="#f996ee" />
      <path d="M166 96 C 220 66, 252 66, 306 92" stroke="#65d9ff" strokeWidth="3" strokeDasharray="1 12" strokeLinecap="round" />
      <circle cx="232" cy="78" r="6" fill="#f996ee" />
      <circle cx="270" cy="74" r="4" fill="#f0dede" />
      <circle cx="296" cy="84" r="4" fill="#ec4612" />
      <rect x="318" y="66" width="110" height="62" rx="31" fill="#ec4612" />
      <rect x="340" y="88" width="66" height="20" rx="10" fill="#fff8f7" />
      <circle cx="382" cy="220" r="12" fill="#f0dede" />
      <path d="M64 186 C 88 212, 112 212, 132 186" stroke="#f996ee" strokeWidth="2.5" fill="none" />
    </svg>
  );
}

function ExchangeArt() {
  return (
    <svg className={styles.cardImage} viewBox="0 0 446 236" fill="none" aria-hidden="true">
      <rect x="70" y="52" width="150" height="36" rx="18" fill="#65d9ff" />
      <rect x="96" y="100" width="240" height="36" rx="18" fill="#f996ee" />
      <rect x="70" y="148" width="120" height="36" rx="18" fill="#ec4612" />
      <path
        d="M380 70v90m0 0-16-16m16 16 16-16"
        stroke="#f0dede"
        strokeWidth="6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx="392" cy="40" r="5" fill="#f996ee" />
      <circle cx="392" cy="182" r="5" fill="#65d9ff" />
    </svg>
  );
}

export function Features() {
  const { d } = useI18n();

  return (
    <section className={styles.section} id="how">
      <div className={styles.inner}>
        <h2 className={styles.headline}>
          <HeadlineSegments segments={d.features.headline} />
        </h2>
        <div className={styles.list}>
          {d.features.cards.map((card, i) => (
            <a key={card.title} href={card.href} className={styles.card}>
              <div className={styles.cardInner}>
                <h3 className={styles.cardTitle}>{card.title}</h3>
                <p className={styles.cardText}>{card.text}</p>
                <span className={styles.cardLink}>
                  {card.link}
                  {i === 0 ? (
                    <svg width="18" height="18" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                      <path d="M3 8h10m0 0L9 4m4 4-4 4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                    </svg>
                  ) : (
                    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                      <path d="m7.07 3 .72-.72a2.55 2.55 0 1 1 3.61 3.6L9.49 7.73" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                    </svg>
                  )}
                </span>
              </div>
              {i === 0 ? <NetworkArt /> : <ExchangeArt />}
            </a>
          ))}
        </div>
      </div>
    </section>
  );
}