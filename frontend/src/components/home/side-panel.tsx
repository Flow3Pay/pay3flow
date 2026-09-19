"use client";

import { RouteCandidate } from "@/lib/exchange";

import styles from "./side-panel.module.css";

const money = (minor: number | undefined, currency: string | undefined) =>
  minor == null ? "—" : `${(minor / 100).toLocaleString("ru-RU", { maximumFractionDigits: 2 })} ${currency ?? ""}`;

function spreadLabel(bps: number): string {
  const value = bps / 100;
  if (Math.abs(value) < 0.005) return "по себестоимости";
  return `${value > 0 ? "+" : ""}${value.toFixed(2)}%`;
}

interface SidePanelProps {
  active: boolean;
  routes: RouteCandidate[];
  searching: boolean;
  selectedQuoteId: string | null;
  onSelect: (route: RouteCandidate) => void;
}

export function SidePanel({ active, routes, searching, selectedQuoteId, onSelect }: SidePanelProps) {
  const groups = routes.reduce<Map<string, RouteCandidate[]>>((result, route) => {
    const key = `${route.entry_asset} ${route.entry_network}`;
    result.set(key, [...(result.get(key) ?? []), route]);
    return result;
  }, new Map());

  return (
    <aside className={`${styles.side}${active ? ` ${styles.active}` : ""}`} aria-label="Найденные связки">
      <div className={styles.stage}>
        <div className={styles.quote}>
          <div className={styles.quoteInner}>
            <div className={styles.quoteHead}>
              <div>
                <div className={styles.quoteTitle}>Связки обмена</div>
                <div className={styles.routeEmpty}>Результаты появляются по мере поиска</div>
              </div>
              {searching && <span className={styles.spinner} aria-label="Поиск" />}
            </div>

            <div className={styles.routeGroups} data-testid="route-groups">
              {[...groups.entries()].map(([group, candidates]) => (
                <section key={group} className={styles.routeGroup}>
                  <h3 className={styles.groupTitle}>{group}</h3>
                  <ul className={styles.routeList}>
                    {candidates.map((route) => {
                      const complete = route.status === "complete";
                      const selected = route.quote_id === selectedQuoteId;
                      return (
                        <li key={route.route_id}>
                          <button
                            type="button"
                            className={`${route.is_current_best ? styles.routeBest : styles.routeRow}${selected ? ` ${styles.selected}` : ""}`}
                            disabled={!complete}
                            onClick={() => onSelect(route)}
                            data-testid={complete ? "complete-route" : "partial-route"}
                          >
                            <span className={styles.routeMain}>
                              <span className={styles.routeAmount}>
                                {complete
                                  ? money(route.target_amount_minor, route.target_currency)
                                  : `${route.source_currency} → ${route.entry_asset}`}
                              </span>
                              <span className={styles.routeMeta}>
                                {complete
                                  ? `Комиссия ${money(route.fee_minor, route.source_currency)} · ${route.eta_minutes} мин`
                                  : "Вход найден, ищем выход в валюту получателя"}
                              </span>
                              <span className={styles.legs}>
                                {route.legs.map((leg) => `${leg.from} → ${leg.to}`).join(" · ")}
                              </span>
                            </span>
                            <span className={styles.routeSide}>
                              {route.is_current_best && <span className={styles.routeBadgeBest}>ЛУЧШИЙ</span>}
                              {!route.is_current_best && complete && (
                                <span className={route.spread_bps < 0 ? styles.routeBadgeDelta : styles.routeBadgeFlat}>
                                  {spreadLabel(route.spread_bps)}
                                </span>
                              )}
                              {!complete && <span className={styles.partialBadge}>ИЩЕМ</span>}
                              <span className={styles.routeName}>{complete ? "Выбрать" : "Недоступно"}</span>
                            </span>
                          </button>
                        </li>
                      );
                    })}
                  </ul>
                </section>
              ))}
            </div>

            {routes.length === 0 && (
              <div className={styles.emptyState}>Укажите сумму и запустите поиск маршрута.</div>
            )}

            <div className={styles.quoteFooter}>
              {searching && <span className={styles.spinner} aria-hidden="true" />}
              Pay3Flow сравнивает полные маршруты, комиссии, сроки и риск
            </div>
          </div>
        </div>
      </div>
    </aside>
  );
}
