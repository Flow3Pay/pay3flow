"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { Bank, fetchBanks } from "@/lib/banks";

import styles from "./pair-picker.module.css";

const ROW_HEIGHT = 60;
const FIRST_PAGE = 30;
const PAGE = 15;
const MAX_ROWS = 60;
const LOAD_MORE_DIST = 120;
const EVICT_AT = 24;
const RESTORE_AT = 2;
const SEARCH_DEBOUNCE_MS = 250;

interface PairPickerProps {
  open: boolean;
  title: string;
  selectedName: string | null;
  onClose: () => void;
  onSelect: (bank: Bank) => void;
  mode: "sender" | "receiver";
  query?: string;
  emptyText?: string;
}

interface WindowData {
  items: Bank[];
  windowStart: number;
  total: number;
  hasMore: boolean;
}

function BankLogo({ src, name }: { src: string; name: string }) {
  const [failed, setFailed] = useState(false);
  if (!src || failed) {
    return <span className={styles.logoFallback}>{name.slice(0, 1).toUpperCase()}</span>;
  }
  return (
    <img
      className={styles.logo}
      src={src}
      alt=""
      loading="lazy"
      onError={() => setFailed(true)}
    />
  );
}

export function PairPicker({
  open,
  title,
  selectedName,
  onClose,
  onSelect,
  mode,
  query,
  emptyText = "No banks found",
}: PairPickerProps) {
  const [search, setSearch] = useState(query ?? "");
  const [serverQuery, setServerQuery] = useState("");
  const [items, setItems] = useState<Bank[]>([]);
  const [windowStart, setWindowStart] = useState(0);
  const [total, setTotal] = useState(0);
  const [hasMore, setHasMore] = useState(true);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const listRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const abortRef = useRef<AbortController | null>(null);
  const loadingRef = useRef(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const dataRef = useRef<WindowData>({ items: [], windowStart: 0, total: 0, hasMore: true });

  const commit = (next: WindowData) => {
    dataRef.current = next;
    setItems(next.items);
    setWindowStart(next.windowStart);
    setTotal(next.total);
    setHasMore(next.hasMore);
  };

  useEffect(() => {
    if (query !== undefined) setSearch(query);
  }, [query]);

  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => setServerQuery(search.trim()), SEARCH_DEBOUNCE_MS);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [search]);

  const role = mode === "sender" ? "sender" : "receiver";

  const fetchPage = useCallback(
    async (offset: number, limit: number, strategy: "replace" | "append" | "prepend") => {
      abortRef.current?.abort();
      const controller = new AbortController();
      abortRef.current = controller;
      loadingRef.current = true;
      setLoading(true);
      setError(null);
      try {
        const page = await fetchBanks({
          limit,
          offset,
          role,
          q: serverQuery || undefined,
          signal: controller.signal,
        });
        if (controller.signal.aborted) return;
        const cur = dataRef.current;
        let nextItems: Bank[];
        let nextStart: number;
        if (strategy === "replace") {
          nextItems = page.items;
          nextStart = page.offset;
        } else if (strategy === "append") {
          nextItems = [...cur.items, ...page.items];
          nextStart = cur.windowStart;
        } else {
          nextItems = [...page.items, ...cur.items];
          nextStart = page.offset;
        }
        if (nextItems.length > MAX_ROWS) {
          const excess = nextItems.length - MAX_ROWS;
          nextItems = nextItems.slice(excess);
          nextStart += excess;
        }
        commit({
          items: nextItems,
          windowStart: nextStart,
          total: page.total,
          hasMore: nextStart + nextItems.length < page.total,
        });
      } catch (err) {
        if (!controller.signal.aborted) {
          setError(err instanceof Error ? err.message : "Failed to load banks");
        }
      } finally {
        if (abortRef.current === controller) {
          loadingRef.current = false;
          setLoading(false);
        }
      }
    },
    [role, serverQuery],
  );

  // First page every time the drawer opens, the side flips, or the search
  // query settles; abort any in-flight load left behind.
  useEffect(() => {
    if (!open) return;
    fetchPage(0, FIRST_PAGE, "replace");
    return () => {
      abortRef.current?.abort();
    };
  }, [open, mode, serverQuery, fetchPage]);

  const handleClose = useCallback(() => {
    setSearch("");
    setServerQuery("");
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

  const handleScroll = () => {
    const list = listRef.current;
    if (!list) return;
    const data = dataRef.current;
    const first = Math.floor(list.scrollTop / ROW_HEIGHT);
    const pos = first - data.windowStart;

    // Close to the end of the loaded window → fetch the next page.
    if (
      data.hasMore &&
      !loadingRef.current &&
      list.scrollHeight > list.clientHeight &&
      list.scrollHeight - list.scrollTop - list.clientHeight < LOAD_MORE_DIST
    ) {
      fetchPage(data.windowStart + data.items.length, PAGE, "append");
      return;
    }

    // Scrolled well past the top of the window → drop the rows left above.
    if (data.items.length > FIRST_PAGE && pos > EVICT_AT) {
      const drop = Math.min(pos - EVICT_AT, data.items.length - FIRST_PAGE);
      if (drop > 0) {
        const evicted = data.items.slice(drop);
        const nextStart = data.windowStart + drop;
        commit({
          items: evicted,
          windowStart: nextStart,
          total: data.total,
          hasMore: nextStart + evicted.length < data.total,
        });
      }
      return;
    }

    // Scrolled back above the window → re-fetch the rows just above it.
    if (data.windowStart > 0 && !loadingRef.current && first <= data.windowStart + RESTORE_AT) {
      const backTo = Math.max(0, data.windowStart - PAGE);
      fetchPage(backTo, data.windowStart - backTo, "prepend");
    }
  };

  if (!open) return null;

  const firstOffset = windowStart * ROW_HEIGHT;
  const lastOffset = Math.max(0, (total - windowStart - items.length) * ROW_HEIGHT);

  return (
    <div className={styles.backdrop} onMouseDown={handleClose}>
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className={styles.head}>
          <h2 className={styles.title}>{title}</h2>
          <button type="button" className={styles.close} onClick={handleClose} aria-label="Close">
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
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Search bank…"
            aria-label="Search bank"
          />
        </div>

        <div className={styles.list} ref={listRef} onScroll={handleScroll} role="listbox">
          {items.length === 0 && loading && <p className={styles.empty}>Loading banks…</p>}
          {items.length === 0 && !loading && error && <p className={styles.empty}>{error}</p>}
          {items.length === 0 && !loading && !error && (
            <p className={styles.empty}>{emptyText}</p>
          )}

          {firstOffset > 0 && <div style={{ height: firstOffset }} aria-hidden="true" />}
          {items.map((bank) => {
            const isSelected = selectedName === bank.name;
            return (
              <button
                key={bank.id}
                type="button"
                role="option"
                aria-selected={isSelected || undefined}
                className={styles.row}
                data-selected={isSelected || undefined}
                onClick={() => onSelect(bank)}
              >
                <span className={styles.leg}>
                  <BankLogo src={bank.icon_url} name={bank.name} />
                  <span className={styles.bank}>{bank.name}</span>
                </span>
                {isSelected && (
                  <svg className={styles.check} width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
                    <circle cx="9" cy="9" r="9" fill="var(--color-primary)" />
                    <path d="M5.5 9.2 8 11.5l4.5-5" stroke="white" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                  </svg>
                )}
              </button>
            );
          })}
          {lastOffset > 0 && <div style={{ height: lastOffset }} aria-hidden="true" />}

          {items.length > 0 && loading && <p className={styles.loadingMore}>Loading…</p>}
          {items.length > 0 && !loading && error && <p className={styles.empty}>{error}</p>}
        </div>
      </div>
    </div>
  );
}