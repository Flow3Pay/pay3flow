"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { GROUP_LABEL, Token, TokenGroup, TOKENS, formatUsd } from "./tokens";

import styles from "./token-picker.module.css";

const GROUPS: TokenGroup[] = ["fiat", "stable", "crypto"];

interface TokenPickerProps {
  open: boolean;
  title: string;
  selected: Token | null;
  onClose: () => void;
  onSelect: (token: Token) => void;
}

export function TokenPicker({ open, title, selected, onClose, onSelect }: TokenPickerProps) {
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  const handleClose = useCallback(() => {
    setQuery("");
    onClose();
  }, [onClose]);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") handleClose();
    };
    window.addEventListener("keydown", onKey);
    document.body.style.overflow = "hidden";
    const timer = setTimeout(() => inputRef.current?.focus(), 60);
    return () => {
      window.removeEventListener("keydown", onKey);
      document.body.style.overflow = "";
      clearTimeout(timer);
    };
  }, [open, handleClose]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return TOKENS;
    return TOKENS.filter(
      (token) =>
        token.symbol.toLowerCase().includes(q) ||
        token.name.toLowerCase().includes(q) ||
        token.nameRu.toLowerCase().includes(q) ||
        token.rail.toLowerCase().includes(q),
    );
  }, [query]);

  const grouped = useMemo(() => {
    const map = new Map<TokenGroup, Token[]>();
    for (const group of GROUPS) map.set(group, []);
    for (const token of filtered) map.get(token.group)?.push(token);
    return map;
  }, [filtered]);

  if (!open) return null;

  return (
    <div className={styles.backdrop} onMouseDown={handleClose}>
      <div
        className={styles.panel}
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className={styles.head}>
          <h2 className={styles.title}>{title}</h2>
          <button type="button" className={styles.close} onClick={handleClose} aria-label="Закрыть">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path d="M4 4l8 8m0-8-8 8" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
            </svg>
          </button>
        </div>

        <div className={styles.search}>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <circle cx="7" cy="7" r="5" stroke="currentColor" strokeWidth="1.6" />
            <path d="m11 11 3 3" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
          </svg>
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Поиск по названию или символу…"
            aria-label="Поиск валюты"
          />
        </div>

        <div className={styles.list}>
          {filtered.length === 0 && <p className={styles.empty}>Ничего не найдено</p>}
          {GROUPS.map((group) => {
            const tokens = grouped.get(group) ?? [];
            if (tokens.length === 0) return null;
            return (
              <div key={group} className={styles.group}>
                <span className={styles.groupLabel}>{GROUP_LABEL[group]}</span>
                {tokens.map((token) => {
                  const isSelected = selected?.symbol === token.symbol;
                  return (
                    <button
                      key={token.symbol}
                      type="button"
                      className={styles.row}
                      data-selected={isSelected || undefined}
                      onClick={() => onSelect(token)}
                    >
                      <span className={styles.avatar} style={{ background: token.color }}>
                        {token.symbol.slice(0, 2)}
                        <span className={styles.rail}>{token.rail === "ERC-20" ? "ETH" : token.rail === "TRC-20" ? "TRX" : token.rail.slice(0, 3)}</span>
                      </span>
                      <span className={styles.meta}>
                        <span className={styles.name}>{token.nameRu}</span>
                        <span className={styles.symbol}>
                          {token.symbol} · {token.rail}
                        </span>
                      </span>
                      <span className={styles.price}>{formatUsd(token.priceUsd)}</span>
                      {isSelected && (
                        <svg className={styles.check} width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
                          <circle cx="9" cy="9" r="9" fill="var(--color-primary)" />
                          <path d="M5.5 9.2 8 11.5l4.5-5" stroke="white" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                        </svg>
                      )}
                    </button>
                  );
                })}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}