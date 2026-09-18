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
  tokenForCurrency,
} from "./tokens";
import { TokenPicker } from "./token-picker";
import { PairPicker } from "./pair-picker";
import { SidePanel } from "./side-panel";

import { Bank } from "@/lib/banks";

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
  { id: "swap", label: "Swap" },
  { id: "payment", label: "Payment" },
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

/** A token for a pair currency, taking the card scheme from the chosen route
 *  and falling back to a neutral 1:1 "bank" token for currencies the local
 *  catalog does not know (KZT, BYN, AMD, …). */
function pairToken(currency: string, scheme: string): Token {
  const base = tokenForCurrency(currency);
  return {
    symbol: currency.toUpperCase(),
    name: base?.name ?? currency.toUpperCase(),
    nameRu: base?.nameRu ?? currency.toUpperCase(),
    color: base?.color ?? "#6b7280",
    priceUsd: base?.priceUsd ?? 1,
    rail: scheme || base?.rail || "Bank",
    group: base?.group ?? "fiat",
  };
}

/** A bank slot: everything the converter needs to preview a route. */
interface BankOption {
  name: string;
  icon: string;
  scheme: string;
  currency: string;
}

function toBankOption(bank: Bank): BankOption {
  return {
    name: bank.name,
    icon: bank.icon_url,
    scheme: bank.schemes[0] ?? "",
    currency: bank.currency,
  };
}

const EMPTY_USD = formatUsd(0);

function SlotLabel({ label, balance, showBalance }: { label: string; balance?: number; showBalance: boolean }) {
  return (
    <div className={styles.slotTop}>
      <span className={styles.slotLabel}>{label}</span>
      {showBalance && balance !== undefined && (
        <span className={styles.slotBalance}>Balance: {formatBalance(balance)}</span>
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
      aria-label={`Select ${token.symbol}`}
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

function PairButton({
  label,
  placeholder,
  fallbackSymbol,
  icon,
  disabled,
  onOpen,
}: {
  label?: string;
  placeholder: string;
  fallbackSymbol: string;
  icon?: string;
  disabled?: boolean;
  onOpen: () => void;
}) {
  const [failed, setFailed] = useState(false);
  return (
    <button
      type="button"
      className={styles.tokenButton}
      onClick={onOpen}
      disabled={disabled}
      aria-label={label ? `Payment route via ${label}` : placeholder}
    >
      {icon && !failed ? (
        <img className={styles.bankAvatar} src={icon} alt="" onError={() => setFailed(true)} />
      ) : (
        <span className={styles.tokenAvatar} style={{ background: label ? "#6b7280" : "var(--color-border)" }}>
          {label ? fallbackSymbol.slice(0, 2) : "—"}
        </span>
      )}
      <span className={styles.bankLabel}>{label ?? placeholder}</span>
      <svg className={styles.tokenChevron} width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </button>
  );
}

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
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsClosing, setSettingsClosing] = useState(false);
  const [autoSlippage, setAutoSlippage] = useState(true);
  const [slippage, setSlippage] = useState(0.5);
  const [deadline, setDeadline] = useState(20);
  const [deadlineOpen, setDeadlineOpen] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  // Bank exchange routes: the swap form picks a real sending bank, then a
  // receiving bank; the exchange combination is the user's.
  const [fromBank, setFromBank] = useState<BankOption | null>(null);
  const [toBank, setToBank] = useState<BankOption | null>(null);
  const [pickerSide, setPickerSide] = useState<Field | null>(null);

  // Live quoting: fmatch answers the exchange pair over WebSocket.
  const [quote, setQuote] = useState<Quote | null>(null);
  const [ratesStatus, setRatesStatus] = useState<RatesStatus>("connecting");
  const [intervalSec, setIntervalSec] = useState(10);

  const settingsRef = useRef<HTMLDivElement>(null);
  const slippageRowRef = useRef<HTMLButtonElement>(null);
  const deadlineRef = useRef<HTMLDivElement>(null);
  const popTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const noticeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const socketRef = useRef<RatesSocket | null>(null);
  const requestRef = useRef<QuoteRequest | null>(null);
  const quoteIdRef = useRef(0);
  const latestQuoteIdRef = useRef(0);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const feePercent = quote?.best?.price != null ? quote.best.price * 100 : NETWORK_FEE_PERCENT;

  const bestResolved = quote?.best != null;

  const sellNum =
    indep === "sell" ? parseAmount(sellText) : parseAmount(buyText) > 0 ? convert(parseAmount(buyText), buy, sell) : 0;
  const buyNum =
    indep === "buy" ? parseAmount(buyText) : bestResolved ? convert(sellNum, sell, buy, feePercent) : 0;

  const sellDisplay = indep === "sell" ? sellText : sellNum > 0 ? formatNumber(sellNum) : "";
  const buyDisplay = indep === "buy" ? buyText : buyNum > 0 ? formatNumber(buyNum) : "";

  const usdIn = sellNum * sell.priceUsd;
  const usdOut = buyNum * buy.priceUsd;
  const rate = sell.priceUsd / buy.priceUsd;

  const hasAmount = sellNum > 0;

  const feeLabel = quote?.best?.price != null ? formatPercentage(feePercent) : `~${NETWORK_FEE_PERCENT}%`;

  const ctaLabel = !connected
    ? "Connect wallet"
    : !hasAmount
      ? "Enter amount"
      : mode === "swap"
        ? `Exchange ${sell.symbol} → ${buy.symbol}`
        : mode === "payment"
          ? `Pay ${sell.symbol} → ${buy.symbol}`
          : `Lock the rate ${sell.symbol} → ${buy.symbol}`;

  const closeSettings = useCallback(() => {
    if (settingsClosing) return;
    setSettingsClosing(true);
    if (popTimer.current) clearTimeout(popTimer.current);
    popTimer.current = setTimeout(() => {
      setSettingsOpen(false);
      setSettingsClosing(false);
    }, 170);
  }, [settingsClosing]);

  const toggleSettings = useCallback(() => {
    if (settingsOpen) {
      closeSettings();
    } else {
      setSettingsOpen(true);
    }
  }, [settingsOpen, closeSettings]);

  useEffect(() => {
    const onClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (settingsRef.current && !settingsRef.current.contains(target)) {
        closeSettings();
      }
      if (slippageRowRef.current?.contains(target)) return; // row toggles the popup itself
      if (deadlineRef.current && !deadlineRef.current.contains(target)) {
        setDeadlineOpen(false);
      }
    };
    if (settingsOpen || settingsClosing || deadlineOpen)
      document.addEventListener("mousedown", onClickOutside);
    return () => document.removeEventListener("mousedown", onClickOutside);
  }, [settingsOpen, settingsClosing, deadlineOpen, closeSettings]);

  useEffect(() => {
    return () => {
      if (popTimer.current) clearTimeout(popTimer.current);
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

  const selectFromBank = (bank: Bank) => {
    setFromBank(toBankOption(bank));
    setToBank(null);
    // Preview the sender's currency/scheme; the user opens the receiving bank
    // picker explicitly from the route button.
    setSell(pairToken(bank.currency, bank.schemes[0] ?? ""));
    setPickerSide(null);
  };

  const selectToBank = (bank: Bank) => {
    setPickerSide(null);
    if (!fromBank) return;
    setToBank(toBankOption(bank));
    setSell(pairToken(fromBank.currency, fromBank.scheme));
    setBuy(pairToken(bank.currency, bank.schemes[0] ?? ""));
    setSellText("");
    setBuyText("");
    setIndep("sell");
  };

  const chooseMode = (next: Mode) => {
    setMode(next);
    setPicker(null);
    if (next !== "swap") setPickerSide(null);
  };

  const handleCta = () => {
    if (!connected) {
      onConnect();
      return;
    }
    if (!hasAmount) return;
    const route = quote?.best ? `, route ${quote.best.name}` : "";
    showNotice(
      `Demo: ${formatNumber(sellNum)} ${sell.symbol} → ${formatNumber(buyNum)} ${buy.symbol} (fee ${feeLabel}${route}). A real quote will be available once payment providers are connected.`,
    );
  };

  const isCtaEnabled = !connected || hasAmount;

  return (
    <section className={styles.shell} id="swap">
      <div className={styles.dock}>
        <div className={styles.card}>
        <div className={styles.head}>
          <div className={styles.tabs} role="tablist" aria-label="Modes">
            {MODES.map((m) => (
              <button
                key={m.id}
                type="button"
                role="tab"
                aria-selected={mode === m.id}
                className={mode === m.id ? styles.tabActive : styles.tab}
                onClick={() => chooseMode(m.id)}
              >
                {m.label}
              </button>
            ))}
          </div>

          <div className={styles.headRight}>
            <div className={styles.settingsAnchor} ref={settingsRef}>
              <button
                type="button"
                className={styles.gear}
                aria-label="Settings"
                aria-expanded={settingsOpen}
                onClick={toggleSettings}
              >
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M22 6.5H16" stroke="currentColor" strokeWidth="1.5" strokeMiterlimit="10" strokeLinecap="round" strokeLinejoin="round" />
                  <path d="M6 6.5H2" stroke="currentColor" strokeWidth="1.5" strokeMiterlimit="10" strokeLinecap="round" strokeLinejoin="round" />
                  <path d="M13.5 6.5C13.5 8.43 11.93 10 10 10C8.07 10 6.5 8.43 6.5 6.5C6.5 4.57 8.07 3 10 3C10.34 3 10.67 3.05 10.98 3.14" stroke="currentColor" strokeWidth="1.5" strokeMiterlimit="10" strokeLinecap="round" strokeLinejoin="round" />
                  <path d="M22 17.5H18" stroke="currentColor" strokeWidth="1.5" strokeMiterlimit="10" strokeLinecap="round" strokeLinejoin="round" />
                  <path d="M8 17.5H2" stroke="currentColor" strokeWidth="1.5" strokeMiterlimit="10" strokeLinecap="round" strokeLinejoin="round" />
                  <path d="M14 21C15.933 21 17.5 19.433 17.5 17.5C17.5 15.567 15.933 14 14 14C12.067 14 10.5 15.567 10.5 17.5C10.5 19.433 12.067 21 14 21Z" stroke="currentColor" strokeWidth="1.5" strokeMiterlimit="10" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              </button>

              {(settingsOpen || settingsClosing) && (
                <div className={settingsClosing ? styles.settingsPopClosing : styles.settingsPop}>
                  <div className={styles.popHead}>
                    <span className={styles.popTitle}>Exchange settings</span>
                  </div>

                  <div className={styles.popSection}>
                    <span className={styles.popLabel}>Rate update interval</span>
                    <div className={styles.optionRow}>
                      {RATE_INTERVALS.map((value) => (
                        <button
                          key={value}
                          type="button"
                          className={intervalSec === value ? styles.pillActive : styles.pill}
                          onClick={() => setIntervalSec(value)}
                        >
                          {value} s
                        </button>
                      ))}
                    </div>
                  </div>

                  <div className={styles.popSection}>
                    <label className={styles.popLabel} htmlFor="deadline">
                      Deadline
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
                        <span>{deadline === 1440 ? "24 hours" : `${deadline} min`}</span>
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
                              <span>{value === 1440 ? "24 hours" : `${value} min`}</span>
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
          label="You sell"
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
              aria-label={`Amount in ${sell.symbol}`}
            />
            <span className={styles.fiat}>{usdIn > 0 ? `≈ ${formatUsd(usdIn)}` : EMPTY_USD}</span>
          </span>
          {mode === "swap" ? (
            <PairButton
              label={fromBank?.name}
              placeholder="Sending bank"
              fallbackSymbol={sell.symbol}
              icon={fromBank?.icon}
              onOpen={() => setPickerSide("sell")}
            />
          ) : (
            <TokenButton token={sell} onOpen={() => setPicker("sell")} />
          )}
        </label>

        {mode === "swap" ? (
          <div className={styles.separatorFlat} aria-hidden="true" />
        ) : (
          <div className={styles.separator} aria-hidden="true">
            <button
              type="button"
              className={styles.swapBtn}
              onClick={switchTokens}
              aria-label="Swap currencies"
              title="Swap"
            >
              <svg width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true">
                <path d="M6 13.5 13.5 6" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" />
                <path d="M13.5 6H8.6M13.5 6v4.9" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" />
                <path d="M14 6.5 6.5 14" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" />
                <path d="M6.5 14h4.9M6.5 14V9.1" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
          </div>
        )}

        <SlotLabel label={`You receive · fee ${feeLabel}${quote?.best ? ` · ${quote.best.name}` : ""}`} showBalance={false} />
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
              aria-label={`Amount in ${buy.symbol}`}
            />
            <span className={styles.fiat}>{usdOut > 0 ? `≈ ${formatUsd(usdOut)}` : EMPTY_USD}</span>
          </span>
          {mode === "swap" ? (
            <PairButton
              label={toBank?.name}
              placeholder={fromBank ? "Receiving bank" : "Pick sending bank"}
              fallbackSymbol={buy.symbol}
              icon={toBank?.icon}
              disabled={!fromBank}
              onOpen={() => fromBank && setPickerSide("buy")}
            />
          ) : (
            <TokenButton token={buy} onOpen={() => setPicker("buy")} />
          )}
        </label>

        <button
          type="button"
          className={styles.slippageRow}
          ref={slippageRowRef}
          onClick={toggleSettings}
          aria-label="Slippage settings"
        >
          <span className={styles.slippageLabel}>
            Slippage
            {autoSlippage && <span className={styles.slippageHint}> (auto)</span>}
          </span>
          <span className={styles.slippageValue}>{autoSlippage ? "Auto" : `${slippage}%`}</span>
        </button>

        <div className={styles.popSection}>
          <div className={styles.popRow}>
            <span>Smart slippage</span>
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

        {hasAmount && (
          <div className={styles.warnRow}>
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" />
              <path d="M8 5v3.5m0 2.5v.01" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
            </svg>
            <span>Minimum payment amount — from 2 {sell.symbol}</span>
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
          {connecting ? "Connecting wallet…" : ctaLabel}
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
        </div>

        <SidePanel
          active={hasAmount}
          sell={sell}
          buy={buy}
          rate={rate}
          feeLabel={feeLabel}
          quote={quote}
          amount={sellNum}
        />
      </div>

      <TokenPicker
        open={picker !== null}
        title={picker === "sell" ? "Choose currency to pay with" : "Choose currency to receive"}
        selected={picker === "sell" ? sell : buy}
        onClose={() => setPicker(null)}
        onSelect={(token) => picker && selectToken(picker, token)}
      />

      <PairPicker
        open={pickerSide !== null}
        title={pickerSide === "buy" ? "Buy" : "Sell"}
        mode={pickerSide === "buy" ? "receiver" : "sender"}
        emptyText={pickerSide === "buy" ? "No receiving banks found" : "No banks available"}
        selectedName={pickerSide === "buy" ? toBank?.name ?? null : fromBank?.name ?? null}
        onClose={() => setPickerSide(null)}
        onSelect={pickerSide === "buy" ? selectToBank : selectFromBank}
      />
    </section>
  );
}
