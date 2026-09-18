"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import {
  NETWORK_FEE_PERCENT,
  Token,
  TOKENS,
  convert,
  formatBalance,
  formatNumber,
  formatUsd,
} from "./tokens";
import { TokenPicker } from "./token-picker";
import { SidePanel } from "./side-panel";

import {
  createRatesSocket,
  Quote,
  QuoteRequest,
  RatesSocket,
  RatesStatus,
} from "@/lib/rates";

import styles from "./converter.module.css";

type Mode = "swap" | "payment" | "limit";
type Field = "sell" | "buy";

const MODES: { id: Mode; label: string }[] = [
  { id: "swap", label: "Обмен" },
  { id: "payment", label: "Оплата" },
  { id: "limit", label: "Лимит" },
];

const RATE_INTERVALS = [5, 10, 15, 20];

const BALANCES: Record<string, number> = {
  EUR: 2438.45,
  USD: 3200,
  TRY: 18420,
  USDT: 1250.8,
  USDC: 1864.25,
  DAI: 903,
  TON: 42.5,
  ETH: 1.24,
};

const SLIPPAGE_OPTIONS = [0.1, 0.5, 1, 2, 5];
const DEADLINE_OPTIONS = [10, 20, 30, 60, 1440];
const QUOTE_DEBOUNCE_MS = 300;

function parseAmount(value: string): number {
  const n = parseFloat(value.replace(/\s+/g, "").replace(",", "."));
  return Number.isFinite(n) && n > 0 ? n : 0;
}

function sanitizeInput(value: string): string {
  const cleaned = value.replace(/[^\d.,]/g, "").replace(/,/g, ".");
  const [head, ...rest] = cleaned.split(".");
  return rest.length ? `${head}.${rest.join("")}` : head;
}

const EMPTY_USD = formatUsd(0);

function SlotLabel({ label, balance, showBalance }: { label: string; balance?: number; showBalance: boolean }) {
  return (
    <div className={styles.slotTop}>
      <span className={styles.slotLabel}>{label}</span>
      {showBalance && balance !== undefined && (
        <span className={styles.slotBalance}>Баланс: {formatBalance(balance)}</span>
      )}
    </div>
  );
}

function TokenButton({ token, onOpen }: { token: Token; onOpen: () => void }) {
  return (
    <button
      type="button"
      className={styles.tokenButton}
      onClick={onOpen}
      aria-label={`Выбрать валюту ${token.symbol}`}
    >
      <span className={styles.tokenAvatar} style={{ background: token.color }}>
        {token.symbol.slice(0, 2)}
      </span>
      <span className={styles.tokenSymbol}>{token.symbol}</span>
      <svg className={styles.tokenChevron} width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </button>
  );
}

const STATUS_LABEL: Record<RatesStatus, string> = {
  open: "онлайн",
  connecting: "подключение…",
  closed: "офлайн",
};

function formatPercentage(value: number): string {
  return `${formatNumber(value, 4)}%`;
}

interface ConverterProps {
  connected: boolean;
  connecting: boolean;
  onConnect: () => void;
}

export function Converter({ connected, connecting, onConnect }: ConverterProps) {
  const [mode, setMode] = useState<Mode>("swap");
  const [sell, setSell] = useState<Token>(TOKENS[0]);
  const [buy, setBuy] = useState<Token>(TOKENS[6]);
  const [sellText, setSellText] = useState("");
  const [buyText, setBuyText] = useState("");
  const [indep, setIndep] = useState<Field>("sell");
  const [picker, setPicker] = useState<Field | null>(null);
  const [rateOpen, setRateOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [autoSlippage, setAutoSlippage] = useState(true);
  const [slippage, setSlippage] = useState(0.5);
  const [deadline, setDeadline] = useState(20);
  const [deadlineOpen, setDeadlineOpen] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  // Live quoting: fmatch answers the exchange pair over WebSocket.
  const [quote, setQuote] = useState<Quote | null>(null);
  const [ratesStatus, setRatesStatus] = useState<RatesStatus>("connecting");
  const [intervalSec, setIntervalSec] = useState(10);

  const settingsRef = useRef<HTMLDivElement>(null);
  const deadlineRef = useRef<HTMLDivElement>(null);
  const noticeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const socketRef = useRef<RatesSocket | null>(null);
  const requestRef = useRef<QuoteRequest | null>(null);
  const quoteIdRef = useRef(0);
  const latestQuoteIdRef = useRef(0);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const feePercent = quote?.best?.price != null ? quote.best.price * 100 : NETWORK_FEE_PERCENT;

  const sellNum =
    indep === "sell" ? parseAmount(sellText) : parseAmount(buyText) > 0 ? convert(parseAmount(buyText), buy, sell) : 0;
  const buyNum = indep === "buy" ? parseAmount(buyText) : convert(sellNum, sell, buy, feePercent);

  const sellDisplay = indep === "sell" ? sellText : sellNum > 0 ? formatNumber(sellNum) : "";
  const buyDisplay = indep === "buy" ? buyText : buyNum > 0 ? formatNumber(buyNum) : "";

  const usdIn = sellNum * sell.priceUsd;
  const usdOut = buyNum * buy.priceUsd;
  const rate = sell.priceUsd / buy.priceUsd;

  const hasAmount = sellNum > 0;

  const feeLabel = quote?.best?.price != null ? formatPercentage(feePercent) : `~${NETWORK_FEE_PERCENT}%`;
  const sourceLabel = quote?.source === "fmatch" ? "fmatch" : quote?.source === "fallback" ? "локально" : null;

  const ctaLabel = !connected
    ? "Подключить кошелёк"
    : !hasAmount
      ? "Введите сумму"
      : mode === "swap"
        ? `Обменять ${sell.symbol} → ${buy.symbol}`
        : mode === "payment"
          ? `Оплатить ${sell.symbol} → ${buy.symbol}`
          : `Зафиксировать курс ${sell.symbol} → ${buy.symbol}`;

  useEffect(() => {
    const onClickOutside = (event: MouseEvent) => {
      if (settingsRef.current && !settingsRef.current.contains(event.target as Node)) {
        setSettingsOpen(false);
      }
      if (deadlineRef.current && !deadlineRef.current.contains(event.target as Node)) {
        setDeadlineOpen(false);
      }
    };
    if (settingsOpen || deadlineOpen) document.addEventListener("mousedown", onClickOutside);
    return () => document.removeEventListener("mousedown", onClickOutside);
  }, [settingsOpen, deadlineOpen]);

  useEffect(() => {
    return () => {
      if (noticeTimer.current) clearTimeout(noticeTimer.current);
    };
  }, []);

  // One socket for the lifetime of the widget: polls are decided client-side,
  // the backend only answers each "quote" message it receives.
  useEffect(() => {
    const socket = createRatesSocket({
      onQuote: (fresh, id) => {
        const replyId = id ? Number(id) : 0;
        if (Number.isFinite(replyId) && replyId > 0 && replyId < latestQuoteIdRef.current) {
          return; // stale reply outran by a newer request
        }
        setQuote(fresh);
      },
      onStatus: setRatesStatus,
    });
    socketRef.current = socket;
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      if (intervalRef.current) clearInterval(intervalRef.current);
      socket.close();
      socketRef.current = null;
    };
  }, []);

  // Debounce on change, then keep refreshing every `intervalSec` while there
  // is an amount — the cadence is the user's choice, not the backend's.
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (intervalRef.current) clearInterval(intervalRef.current);
    if (sellNum <= 0) return;

    const request: QuoteRequest = {
      amount: sellNum,
      currency: sell.symbol,
      from: sell.rail,
      to: buy.rail,
      to_currency: buy.symbol,
    };
    requestRef.current = request;

    const sendNow = () => {
      const req = requestRef.current;
      const socket = socketRef.current;
      if (!req || !socket) return;
      quoteIdRef.current += 1;
      latestQuoteIdRef.current = quoteIdRef.current;
      socket.sendQuote(req, String(quoteIdRef.current));
    };

    debounceRef.current = setTimeout(sendNow, QUOTE_DEBOUNCE_MS);
    intervalRef.current = setInterval(sendNow, intervalSec * 1000);

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [sell, buy, sellNum, intervalSec]);

  const showNotice = useCallback((message: string) => {
    setNotice(message);
    if (noticeTimer.current) clearTimeout(noticeTimer.current);
    noticeTimer.current = setTimeout(() => setNotice(null), 4200);
  }, []);

  const selectToken = (field: Field, token: Token) => {
    if (field === "sell") {
      if (token.symbol === buy.symbol) setBuy(sell);
      setSell(token);
    } else {
      if (token.symbol === sell.symbol) setSell(buy);
      setBuy(token);
    }
    setPicker(null);
  };

  const switchTokens = () => {
    setSell((prev) => {
      setBuy(prev);
      return buy;
    });
    setSellText(buyText);
    setBuyText(sellText);
    setIndep((i) => (i === "sell" ? "buy" : "sell"));
  };

  const handleCta = () => {
    if (!connected) {
      onConnect();
      return;
    }
    if (!hasAmount) return;
    const route = quote?.best ? `, маршрут ${quote.best.name}` : "";
    showNotice(
      `Демо: ${formatNumber(sellNum)} ${sell.symbol} → ${formatNumber(buyNum)} ${buy.symbol} (комиссия ${feeLabel}${route}). Реальный расчёт появится после подключения платёжных провайдеров.`,
    );
  };

  const isCtaEnabled = !connected || hasAmount;

  return (
    <section className={styles.shell} id="swap">
      <div className={styles.dock}>
        <div className={styles.card} ref={settingsRef}>
        <div className={styles.head}>
          <div className={styles.tabs} role="tablist" aria-label="Режимы">
            {MODES.map((m) => (
              <button
                key={m.id}
                type="button"
                role="tab"
                aria-selected={mode === m.id}
                className={mode === m.id ? styles.tabActive : styles.tab}
                onClick={() => setMode(m.id)}
              >
                {m.label}
              </button>
            ))}
          </div>

          <div className={styles.headRight}>
            <div className={styles.settingsAnchor}>
              <button
                type="button"
                className={styles.gear}
                aria-label="Настройки"
                aria-expanded={settingsOpen}
                onClick={() => setSettingsOpen((v) => !v)}
              >
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path
                    d="M12 15.5A3.5 3.5 0 1 0 12 8a3.5 3.5 0 0 0 0 7.5Z"
                    stroke="currentColor"
                    strokeWidth="1.7"
                  />
                  <path
                    d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1.11-1.56 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.65 8.9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.54V4.45A1.7 1.7 0 0 0 10.92 3.4V3.3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87v.05A1.7 1.7 0 0 0 21.6 11h.09a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.2.53Z"
                    stroke="currentColor"
                    strokeWidth="1.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </svg>
              </button>

              {settingsOpen && (
                <div className={styles.settingsPop}>
                  <div className={styles.popHead}>
                    <span className={styles.popTitle}>Настройки обмена</span>
                  </div>

                  <div className={styles.popSection}>
                    <div className={styles.popRow}>
                      <span>Умное проскальзывание</span>
                      <button
                        type="button"
                        role="switch"
                        aria-checked={autoSlippage}
                        className={autoSlippage ? styles.switchOn : styles.switch}
                        onClick={() => setAutoSlippage((v) => !v)}
                      >
                        <span className={styles.knob} />
                      </button>
                    </div>
                    <div className={styles.optionRow} aria-disabled={autoSlippage || undefined}>
                      {SLIPPAGE_OPTIONS.map((value) => (
                        <button
                          key={value}
                          type="button"
                          disabled={autoSlippage}
                          className={slippage === value ? styles.pillActive : styles.pill}
                          onClick={() => setSlippage(value)}
                        >
                          {value}%
                        </button>
                      ))}
                    </div>
                  </div>

                  <div className={styles.popSection}>
                    <span className={styles.popLabel}>Периодичность обновления курса</span>
                    <div className={styles.optionRow}>
                      {RATE_INTERVALS.map((value) => (
                        <button
                          key={value}
                          type="button"
                          className={intervalSec === value ? styles.pillActive : styles.pill}
                          onClick={() => setIntervalSec(value)}
                        >
                          {value} сек
                        </button>
                      ))}
                    </div>
                  </div>

                  <div className={styles.popSection}>
                    <label className={styles.popLabel} htmlFor="deadline">
                      Срок действия
                    </label>
                    <div className={styles.deadlineWrap} ref={deadlineRef}>
                      <button
                        type="button"
                        id="deadline"
                        className={styles.deadlineTrigger}
                        aria-haspopup="listbox"
                        aria-expanded={deadlineOpen}
                        onClick={() => setDeadlineOpen((v) => !v)}
                      >
                        <span>{deadline === 1440 ? "24 часа" : `${deadline} мин`}</span>
                        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                          <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                        </svg>
                      </button>
                      {deadlineOpen && (
                        <div className={styles.deadlineMenu} role="listbox">
                          {DEADLINE_OPTIONS.map((value) => (
                            <button
                              key={value}
                              type="button"
                              className={styles.deadlineOption}
                              role="option"
                              aria-selected={deadline === value}
                              onClick={() => {
                                setDeadline(value);
                                setDeadlineOpen(false);
                              }}
                            >
                              <span>{value === 1440 ? "24 часа" : `${value} мин`}</span>
                              {deadline === value && (
                                <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                                  <path d="M3 8l3.5 3.5L13 5" stroke="var(--color-accent)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                                </svg>
                              )}
                            </button>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>

        <SlotLabel
          label="Вы продаёте"
          balance={BALANCES[sell.symbol]}
          showBalance={connected}
        />
        <label className={styles.panel}>
          <span className={styles.panelMain}>
            <input
              className={styles.input}
              type="text"
              inputMode="decimal"
              placeholder="0"
              value={sellDisplay}
              onChange={(event) => {
                setSellText(sanitizeInput(event.target.value));
                setIndep("sell");
              }}
              aria-label={`Сумма в ${sell.symbol}`}
            />
            <span className={styles.fiat}>{usdIn > 0 ? `≈ ${formatUsd(usdIn)}` : EMPTY_USD}</span>
          </span>
          <TokenButton token={sell} onOpen={() => setPicker("sell")} />
        </label>

        <div className={styles.separator} aria-hidden="true">
          <button
            type="button"
            className={styles.swapBtn}
            onClick={switchTokens}
            aria-label="Поменять валюты местами"
            title="Поменять местами"
          >
            <svg width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true">
              <path d="M6 13.5 13.5 6" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" />
              <path d="M13.5 6H8.6M13.5 6v4.9" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" />
              <path d="M14 6.5 6.5 14" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" />
              <path d="M6.5 14h4.9M6.5 14V9.1" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
        </div>

        <SlotLabel label={`Вы получаете · комиссия ${feeLabel}${quote?.best ? ` · ${quote.best.name}` : ""}`} showBalance={false} />
        <label className={styles.panel}>
          <span className={styles.panelMain}>
            <input
              className={styles.input}
              type="text"
              inputMode="decimal"
              placeholder="0"
              value={buyDisplay}
              onChange={(event) => {
                setBuyText(sanitizeInput(event.target.value));
                setIndep("buy");
              }}
              aria-label={`Сумма в ${buy.symbol}`}
            />
            <span className={styles.fiat}>{usdOut > 0 ? `≈ ${formatUsd(usdOut)}` : EMPTY_USD}</span>
          </span>
          <TokenButton token={buy} onOpen={() => setPicker("buy")} />
        </label>

        <button
          type="button"
          className={styles.rateRow}
          aria-expanded={rateOpen}
          onClick={() => setRateOpen((v) => !v)}
        >
          <span className={styles.rateText}>
            1 {sell.symbol} = {formatNumber(rate)} {buy.symbol}
            {sourceLabel && <span className={styles.rateUsd}>· {sourceLabel}</span>}
          </span>
          <svg
            className={rateOpen ? styles.chevronUp : styles.chevron}
            width="16"
            height="16"
            viewBox="0 0 16 16"
            fill="none"
            aria-hidden="true"
          >
            <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </button>

        {rateOpen && (
          <div className={styles.rateDetails}>
            <div className={styles.rateDetailRow}>
              <span>Вы получаете (с учётом комиссии)</span>
              <strong>
                {formatNumber(buyNum, 10)} {buy.symbol}
              </strong>
            </div>
            <div className={styles.rateDetailRow}>
              <span>Комиссия{quote?.best ? ` · ${quote.best.name}` : " сети и сервиса"}</span>
              <strong>{feeLabel}</strong>
            </div>
            <div className={styles.rateDetailRow}>
              <span>Курс</span>
              <strong>
                1 {sell.symbol} = {formatNumber(rate, 8)} {buy.symbol}
              </strong>
            </div>
            <div className={styles.rateDetailRow}>
              <span>Маршруты</span>
              <strong>{sourceLabel ? `${sourceLabel} · ${STATUS_LABEL[ratesStatus]}` : "ожидаем ответ fmatch…"}</strong>
            </div>
          </div>
        )}

        <button
          type="button"
          className={styles.slippageRow}
          onClick={() => setSettingsOpen((v) => !v)}
          aria-label="Настройки проскальзывания"
        >
          <span className={styles.slippageLabel}>
            Проскальзывание
            {autoSlippage && <span className={styles.slippageHint}> (авто)</span>}
          </span>
          <span className={styles.slippageValue}>{autoSlippage ? "Авто" : `${slippage}%`}</span>
        </button>

        {hasAmount && (
          <div className={styles.warnRow}>
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" />
              <path d="M8 5v3.5m0 2.5v.01" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
            </svg>
            <span>Минимальная сумма платежа — от 2 {sell.symbol}</span>
          </div>
        )}

        <button
          type="button"
          className={styles.cta}
          disabled={!isCtaEnabled}
          onClick={handleCta}
        >
          {connecting && (
            <span className={styles.spinner} aria-hidden="true" />
          )}
          {!connecting && !connected && (
            <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
              <path
                d="M3.5 3.5h7.5a2 2 0 0 1 2 2v7a2 2 0 0 0 2 2h-11.5V6"
                stroke="currentColor"
                strokeWidth="1.6"
                strokeLinecap="round"
              />
              <circle cx="11.5" cy="10" r="1" fill="currentColor" />
            </svg>
          )}
          {connecting ? "Подключаем кошелёк…" : ctaLabel}
        </button>

        {notice && (
          <div className={styles.notice} role="status">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <circle cx="8" cy="8" r="8" fill="var(--color-good)" />
              <path d="M5 8.2 7.2 10.5 11 6.2" stroke="white" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
            {notice}
          </div>
        )}

        <div className={styles.powered}>Сеть Pay3Flow · рельсы SEBA · SEPA · ERC-20 · TRC-20</div>
        </div>

        <SidePanel
          active={hasAmount}
          sell={sell}
          buy={buy}
          rate={rate}
          feeLabel={feeLabel}
          quote={quote}
          ratesStatus={ratesStatus}
        />
      </div>

      <TokenPicker
        open={picker !== null}
        title={picker === "sell" ? "Выберите валюту оплаты" : "Выберите валюту получения"}
        selected={picker === "sell" ? sell : buy}
        onClose={() => setPicker(null)}
        onSelect={(token) => picker && selectToken(picker, token)}
      />
    </section>
  );
}