"use client";

import { useCallback, useEffect } from "react";

import { CryptoNetwork } from "@/lib/networks";

import styles from "./payment-method-picker.module.css";

interface NetworkPickerProps {
  open: boolean;
  networks: CryptoNetwork[];
  selected: CryptoNetwork;
  onClose: () => void;
  onSelect: (network: CryptoNetwork) => void;
}

export function NetworkPicker({
  open,
  networks,
  selected,
  onClose,
  onSelect,
}: NetworkPickerProps) {
  const handleClose = useCallback(() => onClose(), [onClose]);

  useEffect(() => {
    if (!open) return;
    const previousOverflow = document.body.style.overflow;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") handleClose();
    };
    document.body.style.overflow = "hidden";
    window.addEventListener("keydown", onKeyDown);
    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [handleClose, open]);

  if (!open) return null;

  return (
    <div className={styles.backdrop} onMouseDown={handleClose}>
      <div
        className={styles.dialog}
        role="dialog"
        aria-modal="true"
        aria-label="Choose network"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className={styles.titleBar}>
          <div className={styles.titleGroup}>
            <button
              type="button"
              className={styles.backButton}
              onClick={handleClose}
              aria-label="Close network picker"
            >
              <svg width="21" height="21" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path d="m7 7 10 10m0-10L7 17" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
            </button>
            <h2 className={styles.title}>Choose network</h2>
          </div>
        </div>

        <div className={styles.body}>
          <div className={styles.methods} role="listbox" aria-label="Crypto networks">
            <section className={styles.section}>
              <h3>Available networks</h3>
              {networks.map((network) => {
                const isSelected = network.id === selected.id;
                return (
                  <button
                    key={network.id}
                    type="button"
                    role="option"
                    aria-selected={isSelected}
                    className={styles.methodRow}
                    data-selected={isSelected || undefined}
                    onClick={() => onSelect(network)}
                  >
                    <span
                      className={styles.methodLogo}
                      style={{ backgroundColor: "#627eea" }}
                      aria-hidden="true"
                    >
                      ♦
                    </span>
                    <span className={styles.methodCopy}>
                      <span className={styles.methodName}>{network.name}</span>
                      <span className={styles.methodMeta}>{network.currencies.join(" · ")}</span>
                    </span>
                    {isSelected && (
                      <svg className={styles.check} width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
                        <circle cx="10" cy="10" r="10" fill="currentColor" />
                        <path d="m6 10.2 2.7 2.5 5.3-5.6" stroke="white" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                      </svg>
                    )}
                  </button>
                );
              })}
            </section>
          </div>
        </div>
      </div>
    </div>
  );
}
