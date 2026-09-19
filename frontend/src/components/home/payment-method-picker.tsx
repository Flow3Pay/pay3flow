"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  PaymentMethod,
  paymentCountry,
  paymentMethodsFor,
} from "@/lib/payment-methods";

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
  locations: PaymentLocation[];
  selectedLocation: PaymentLocation | null;
  selected: PaymentMethod | null;
  onClose: () => void;
  onLocationSelect: (location: PaymentLocation) => void;
  onSelect: (method: PaymentMethod) => void;
}

export function PaymentMethodPicker({
  open,
  title,
  role,
  locations,
  selectedLocation,
  selected,
  onClose,
  onLocationSelect,
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

  const methods = useMemo(() => {
    if (!selectedLocation) return [];
    return paymentMethodsFor(
      selectedLocation.country,
      selectedLocation.currency,
      role,
    );
  }, [role, selectedLocation]);

  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return methods;
    return methods.filter((method) =>
      [method.name, method.currency, method.kind, method.p2pQuery]
        .join(" ")
        .toLowerCase()
        .includes(normalized),
    );
  }, [methods, query]);

  const popular = filtered.filter((method) => method.popular);
  const all = filtered.filter((method) => !method.popular);

  if (!open) return null;

  const renderMethod = (method: PaymentMethod) => {
    const isSelected = selected?.id === method.id;
    return (
      <button
        key={method.id}
        type="button"
        role="option"
        aria-selected={isSelected}
        className={styles.methodRow}
        data-selected={isSelected || undefined}
        onClick={() => onSelect(method)}
      >
        <BankLogo className={styles.methodLogo} method={method} fallback={method.initials} />
        <span className={styles.methodCopy}>
          <span className={styles.methodName}>{method.name}</span>
          <span className={styles.methodMeta}>
            {method.kind === "bank" ? "Bank transfer" : "Digital wallet"} · {method.currency}
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
                <path d="m15 18-6-6 6-6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
            <h2 className={styles.title}>{title}</h2>
          </div>
          <span className={styles.modeBadge}>{role === "sender" ? "Pay with" : "Receive with"}</span>
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
              placeholder="Search banks and payment methods…"
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
            {all.length > 0 && (
              <section className={styles.section}>
                <h3>All payment methods</h3>
                {all.map(renderMethod)}
              </section>
            )}
          </div>

          <aside className={styles.countries} aria-label="Select country">
            <div className={styles.countryHead}>
              <h3>Select country</h3>
              <span>Available corridor</span>
            </div>
            <div className={styles.countryList}>
              {locations.map((location) => {
                const country = paymentCountry(location.country, location.currency);
                const isActive =
                  selectedLocation?.country === location.country &&
                  selectedLocation.currency === location.currency;
                return (
                  <button
                    key={`${location.country}:${location.currency}`}
                    type="button"
                    className={styles.countryButton}
                    aria-pressed={isActive}
                    onClick={() => onLocationSelect(location)}
                  >
                    <span className={styles.countryMark}>{country?.mark ?? location.country}</span>
                    <span className={styles.countryCopy}>
                      <strong>{country?.name ?? location.country}</strong>
                      <span>{location.currency}</span>
                    </span>
                    {isActive && (
                      <svg className={styles.countryCheck} width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
                        <circle cx="9" cy="9" r="9" fill="currentColor" />
                        <path d="M5.5 9.2 8 11.5l4.5-5" stroke="white" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" />
                      </svg>
                    )}
                  </button>
                );
              })}
            </div>
          </aside>
        </div>
      </div>
    </div>
  );
}
