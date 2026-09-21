"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  DIGITAL_ASSETS,
  PaymentMethod,
  paymentMethodsFor,
} from "@/lib/payment-methods";
import type { CryptoNetwork } from "@/lib/networks";

import { BankLogo } from "./bank-logo";
import styles from "./payment-method-picker.module.css";

export interface PaymentLocation {
  country: string;
  currency: string;
}

interface PaymentMethodPickerProps {
  open: boolean;
  title: string;
  role: "sender" | "recipient";
  networks: CryptoNetwork[];
  selectedLocation: PaymentLocation | null;
  selected: PaymentMethod | null;
  selectedNetwork?: CryptoNetwork;
  onClose: () => void;
  onSelect: (method: PaymentMethod, network?: CryptoNetwork) => void;
}

interface PaymentMethodOption {
  method: PaymentMethod;
  network?: CryptoNetwork;
}

function normalizeSearch(value: string): string {
  return value
    .normalize("NFKC")
    .toLocaleLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim();
}

function matchesSearch(option: PaymentMethodOption, query: string): boolean {
  const { method, network } = option;
  const searchText = normalizeSearch([
    method.name,
    method.currency,
    method.kind,
    method.p2pQuery,
    network?.name,
    network?.id,
  ].filter(Boolean).join(" "));
  const compactSearchText = searchText.replaceAll(" ", "");

  return normalizeSearch(query)
    .split(" ")
    .filter(Boolean)
    .every((term) => searchText.includes(term) || compactSearchText.includes(term));
}

export function PaymentMethodPicker({
  open,
  title,
  role,
  networks,
  selectedLocation,
  selected,
  selectedNetwork,
  onClose,
  onSelect,
}: PaymentMethodPickerProps) {
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  const handleClose = useCallback(() => {
    setQuery("");
    onClose();
  }, [onClose]);

  useEffect(() => {
    if (!open) return;
    const previousOverflow = document.body.style.overflow;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") handleClose();
    };
    document.body.style.overflow = "hidden";
    window.addEventListener("keydown", onKeyDown);
    const focusTimer = window.setTimeout(() => inputRef.current?.focus(), 80);
    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", onKeyDown);
      window.clearTimeout(focusTimer);
    };
  }, [handleClose, open]);

  const options = useMemo(() => {
    const banks = selectedLocation
      ? paymentMethodsFor(selectedLocation.country, selectedLocation.currency, role)
      : [];
    const assets = DIGITAL_ASSETS.filter(
      (method) => method.role === role || method.role === "both",
    );
    return [
      ...banks.map((method): PaymentMethodOption => ({ method })),
      ...assets.flatMap((method): PaymentMethodOption[] => {
        const compatibleNetworks = networks.filter((network) =>
          network.currencies.includes(method.currency),
        );
        return compatibleNetworks.length > 0
          ? compatibleNetworks.map((network) => ({ method, network }))
          : [{ method }];
      }),
    ];
  }, [networks, role, selectedLocation]);

  const filtered = useMemo(() => {
    if (!query.trim()) return options;
    return options.filter((option) => matchesSearch(option, query));
  }, [options, query]);

  const popular = filtered.filter(({ method }) => method.popular);
  const assets = filtered.filter(({ method }) => method.kind === "wallet");
  const all = filtered.filter(({ method }) => !method.popular && method.kind === "bank");

  if (!open) return null;

  const renderMethod = ({ method, network }: PaymentMethodOption) => {
    const isSelected = selected?.id === method.id &&
      (method.kind !== "wallet" || network?.id === selectedNetwork?.id);
    return (
      <button
        key={`${method.id}:${network?.id ?? "default"}`}
        type="button"
        role="option"
        aria-selected={isSelected}
        className={styles.methodRow}
        data-selected={isSelected || undefined}
        onClick={() => onSelect(method, network)}
      >
        <BankLogo className={styles.methodLogo} method={method} fallback={method.initials} />
        <span className={styles.methodCopy}>
          <span className={styles.methodName}>{method.name}</span>
          <span className={styles.methodMeta}>
            {method.kind === "bank"
              ? `Bank transfer · ${method.currency}`
              : `${method.currency} · ${network?.name ?? "Digital wallet"}`}
          </span>
        </span>
        {isSelected && (
          <svg className={styles.check} width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
            <circle cx="10" cy="10" r="10" fill="currentColor" />
            <path d="m6 10.2 2.7 2.5 5.3-5.6" stroke="white" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        )}
      </button>
    );
  };

  return (
    <div className={styles.backdrop} onMouseDown={handleClose}>
      <div
        className={styles.dialog}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className={styles.titleBar}>
          <div className={styles.titleGroup}>
            <button type="button" className={styles.backButton} onClick={handleClose} aria-label="Close payment method picker">
              <svg width="21" height="21" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path d="m7 7 10 10m0-10L7 17" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
            </button>
            <h2 className={styles.title}>{title}</h2>
          </div>
        </div>

        <div className={styles.searchRow}>
          <label className={styles.searchBox}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <circle cx="11" cy="11" r="7" stroke="currentColor" strokeWidth="2" />
              <path d="m20 20-4-4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            </svg>
            <input
              ref={inputRef}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search banks, assets or networks…"
              aria-label="Search banks and payment methods"
            />
          </label>
        </div>

        <div className={styles.body}>
          <div className={styles.methods} role="listbox" aria-label="Payment methods">
            {filtered.length === 0 && (
              <div className={styles.empty}>
                <strong>No payment methods found</strong>
                <span>Try a different bank name.</span>
              </div>
            )}
            {popular.length > 0 && (
              <section className={styles.section}>
                <h3>Popular banks</h3>
                {popular.map(renderMethod)}
              </section>
            )}
            {assets.length > 0 && (
              <section className={styles.section}>
                <h3>Digital assets</h3>
                {assets.map(renderMethod)}
              </section>
            )}
            {all.length > 0 && (
              <section className={styles.section}>
                <h3>All payment methods</h3>
                {all.map(renderMethod)}
              </section>
            )}
          </div>

        </div>
      </div>
    </div>
  );
}
