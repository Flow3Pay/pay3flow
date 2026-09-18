"use client";

import { formatNumber, Token } from "./tokens";
import { Quote, RatesStatus } from "@/lib/rates";

import styles from "./side-panel.module.css";

const STATUS_LABEL: Record<RatesStatus, string> = {
  open: "онлайн",
  connecting: "подключение…",
  closed: "офлайн",
};

function formatPercentage(value: number): string {
  return `${formatNumber(value, 4)}%`;
}

interface SidePanelProps {
  active: boolean;
  sell: Token;
  buy: Token;
  rate: number;
  feeLabel: string;
  quote: Quote | null;
  ratesStatus: RatesStatus;
}

export function SidePanel({
  active,
  sell,
  buy,
  rate,
  feeLabel,
  quote,
  ratesStatus,
}: SidePanelProps) {
  const bestName = quote?.best?.name ?? null;
  const sourceLabel =
    quote?.source === "fmatch" ? "fmatch" : quote?.source === "fallback" ? "локально" : null;
  const candidates = quote?.candidates ?? [];

  return (
    <div className={`${styles.side}${active ? ` ${styles.active}` : ""}`}>
      <div className={styles.stage}>
        <div className={styles.quote} aria-hidden={!active || undefined}>
          <div className={styles.quoteInner}>
            <div className={styles.quoteHead}>
              <span className={styles.quoteTitle}>Сводка обмена</span>
              <span className={styles.live}>
                <span className={styles.liveDot} aria-hidden="true" />
                {STATUS_LABEL[ratesStatus]}
              </span>
            </div>

            <div className={styles.details}>
              <div className={styles.detailRow}>
                <span>Курс</span>
                <strong>
                  1 {sell.symbol} = {formatNumber(rate)} {buy.symbol}
                </strong>
              </div>
              <div className={styles.detailRow}>
                <span>Комиссия{bestName ? ` · ${bestName}` : ""}</span>
                <strong>{feeLabel}</strong>
              </div>
              {quote?.quotedAt && (
                <div className={styles.detailRow}>
                  <span>Котировка</span>
                  <strong>{new Date(quote.quotedAt).toLocaleTimeString("ru-RU")}</strong>
                </div>
              )}
            </div>

            <div className={styles.routesBlock}>
              <div className={styles.routesHead}>
                <span className={styles.routesTitle}>
                  <span className={styles.routesDot} aria-hidden="true" />
                  Маршруты{sourceLabel ? ` · ${sourceLabel}` : ""}
                </span>
                {quote?.quotedAt && (
                  <span className={styles.routesTime}>
                    {new Date(quote.quotedAt).toLocaleTimeString("ru-RU")}
                  </span>
                )}
              </div>
              {candidates.length > 0 ? (
                <ol className={styles.routeList}>
                  {candidates.slice(0, 3).map((c) => {
                    const isBest = c.rank === quote?.best?.rank;
                    return (
                      <li
                        key={`${c.shortId ?? c.name}-${c.rank}`}
                        className={isBest ? styles.routeBest : styles.routeRow}
                      >
                        <span className={styles.routeIndex}>{c.rank}</span>
                        <span className={styles.routeName}>{c.name}</span>
                        <span className={styles.routeEnd}>
                          {isBest && <span className={styles.bestBadge}>ЛУЧШИЙ</span>}
                          {c.price != null && (
                            <span className={styles.routeFee}>{formatPercentage(c.price * 100)}</span>
                          )}
                        </span>
                      </li>
                    );
                  })}
                </ol>
              ) : (
                <div className={styles.routeEmpty}>
                  {quote ? "Подходящих эквайеров под пару не найдено" : "ожидаем котировку…"}
                </div>
              )}
            </div>

            <div className={styles.quoteFooter}>
              <span className={styles.spinner} aria-hidden="true" />
              Pay3Flow подбирает лучший маршрут автоматически
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}