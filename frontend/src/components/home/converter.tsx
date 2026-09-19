"use client";

import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  ConfirmOrderResponse,
  ExchangeCorridor,
  ExchangeOrder,
  FundingInstruction,
  LiveRouteEvent,
  RouteCandidate,
  authenticate,
  confirmFunding,
  confirmOrder,
  createOrder,
  fetchCorridors,
  fetchOrders,
  fetchQuotes,
  openLiveRoutes,
  quoteToCandidate,
  submitMockProof,
} from "@/lib/exchange";

import { SidePanel } from "./side-panel";
import styles from "./converter.module.css";

const STATUS_RU: Record<string, string> = {
  created: "Заявка создана",
  discovering: "Ищем исполнителей",
  quoting: "Собираем маршруты",
  quoted: "Маршрут выбран",
  locked: "Маршрут зафиксирован",
  token_settling: "Выполняется расчётный этап",
  money_settling: "Деньги отправляются получателю",
  proof_pending: "Проверяем подтверждение",
  done: "Перевод завершён",
  failed: "Перевод не выполнен",
  expired: "Заявка истекла",
  cancelled: "Заявка отменена",
  disputed: "Нужна ручная проверка",
};

const money = (minor: number, currency: string) =>
  `${(minor / 100).toLocaleString("ru-RU", { maximumFractionDigits: 2 })} ${currency}`;

function upsertRoute(routes: RouteCandidate[], fresh: RouteCandidate): RouteCandidate[] {
  const existing = routes.findIndex((route) => route.route_id === fresh.route_id);
  if (existing < 0) return [...routes, fresh];
  return routes.map((route, index) => (index === existing ? { ...route, ...fresh } : route));
}

interface ConverterProps {
  token: string | null;
  email: string;
  onAuthenticated: (token: string, email: string) => void;
  onRequireAuth: () => void;
}

export function Converter({ token, email: sessionEmail, onAuthenticated, onRequireAuth }: ConverterProps) {
  const [corridors, setCorridors] = useState<ExchangeCorridor[]>([]);
  const [corridorId, setCorridorId] = useState("");
  const [termsVersion, setTermsVersion] = useState("");
  const [amount, setAmount] = useState("100000");
  const [sourceMethod, setSourceMethod] = useState("bank_card");
  const [targetMethod, setTargetMethod] = useState("bank_card");
  const [recipient, setRecipient] = useState("");
  const [authEmail, setAuthEmail] = useState(sessionEmail || "demo@pay3flow.dev");
  const [authCode, setAuthCode] = useState("1234");
  const [authBusy, setAuthBusy] = useState(false);
  const [order, setOrder] = useState<ExchangeOrder | null>(null);
  const [routes, setRoutes] = useState<RouteCandidate[]>([]);
  const [selected, setSelected] = useState<RouteCandidate | null>(null);
  const [funding, setFunding] = useState<FundingInstruction | null>(null);
  const [settlement, setSettlement] = useState<{ id: string; solver_id: string } | null>(null);
  const [consent, setConsent] = useState(false);
  const [searching, setSearching] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [socketFallback, setSocketFallback] = useState(false);
  const [history, setHistory] = useState<ExchangeOrder[]>([]);
  const closeSocket = useRef<(() => void) | null>(null);

  const corridor = useMemo(
    () => corridors.find((item) => item.id === corridorId) ?? corridors[0],
    [corridorId, corridors],
  );

  useEffect(() => {
    fetchCorridors()
      .then((response) => {
        setCorridors(response.items);
        setCorridorId((current) => current || response.items[0]?.id || "");
        setTermsVersion(response.terms_version);
      })
      .catch((cause: Error) => setError(cause.message));
  }, []);

  const refreshHistory = useCallback(() => {
    if (!token) return;
    fetchOrders(token).then(setHistory).catch(() => undefined);
  }, [token]);

  useEffect(refreshHistory, [refreshHistory]);
  useEffect(() => () => closeSocket.current?.(), []);

  const handleLiveEvent = useCallback((event: LiveRouteEvent) => {
    if (event.type === "entry_leg_found" || event.type === "route_candidate_found") {
      setRoutes((current) => upsertRoute(current, event));
      return;
    }
    if (event.type === "best_route_updated") {
      setRoutes((current) =>
        current.map((route) => ({ ...route, is_current_best: route.quote_id === event.quote_id })),
      );
      return;
    }
    if (event.type === "order_status") {
      setOrder((current) => (current ? { ...current, status: event.status } : current));
      return;
    }
    if (event.type === "search_finished") {
      setSearching(false);
      return;
    }
    if (event.type === "search_failed") {
      setSearching(false);
      setError(event.error);
    }
  }, []);

  useEffect(() => {
    if (!socketFallback || !token || !order || !searching) return;
    const timer = window.setInterval(() => {
      fetchQuotes(token, order.id)
        .then((quotes) => setRoutes(quotes.map(quoteToCandidate)))
        .catch(() => undefined);
    }, 1500);
    return () => window.clearInterval(timer);
  }, [order, searching, socketFallback, token]);

  const signIn = async (event: FormEvent) => {
    event.preventDefault();
    setAuthBusy(true);
    setError(null);
    try {
      const freshToken = await authenticate(authEmail.trim(), authCode.trim());
      onAuthenticated(freshToken, authEmail.trim());
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Не удалось войти");
    } finally {
      setAuthBusy(false);
    }
  };

  const startSearch = async () => {
    if (!token) {
      onRequireAuth();
      setError("Сначала войдите, чтобы создать заявку.");
      return;
    }
    if (!corridor) {
      setError("Backend не вернул доступный коридор.");
      return;
    }
    const numericAmount = Number(amount.replace(/\s/g, "").replace(",", "."));
    if (!Number.isFinite(numericAmount) || numericAmount <= 0) {
      setError("Введите сумму больше нуля.");
      return;
    }
    setBusy(true);
    setError(null);
    setRoutes([]);
    setSelected(null);
    setFunding(null);
    setSettlement(null);
    setConsent(false);
    closeSocket.current?.();
    try {
      const freshOrder = await createOrder(
        token,
        {
          source_country: corridor.source_country,
          source_currency: corridor.source_currency,
          source_amount_minor: Math.round(numericAmount * 100),
          source_method_type: sourceMethod,
          source_method_ref: null,
          target_country: corridor.target_country,
          target_currency: corridor.target_currency,
          target_amount_min_minor: null,
          target_method_type: targetMethod,
          target_method_ref: recipient.trim() || null,
        },
        crypto.randomUUID(),
      );
      setOrder(freshOrder);
      setSearching(true);
      setSocketFallback(false);
      closeSocket.current = openLiveRoutes(token, freshOrder.id, handleLiveEvent, () => setSocketFallback(true));
      refreshHistory();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Не удалось создать заявку");
    } finally {
      setBusy(false);
    }
  };

  const lockRoute = async () => {
    if (!token || !order || !selected?.quote_id) return;
    setBusy(true);
    setError(null);
    try {
      const response: ConfirmOrderResponse = await confirmOrder(token, order.id, selected.quote_id);
      setOrder(response.order);
      setFunding(response.funding_instruction);
      setSettlement(response.settlement);
      setSearching(false);
      closeSocket.current?.();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Не удалось зафиксировать маршрут");
    } finally {
      setBusy(false);
    }
  };

  const fundAndFinish = async () => {
    if (!token || !order || !selected || !settlement || !consent) return;
    setBusy(true);
    setError(null);
    try {
      const funded = await confirmFunding(token, order.id, termsVersion);
      setOrder(funded.order);
      const completed = await submitMockProof(
        token,
        order.id,
        settlement.id,
        settlement.solver_id,
        selected.target_amount_minor ?? 0,
        selected.target_currency ?? order.target_currency,
      );
      setOrder(completed.order);
      refreshHistory();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Не удалось подтвердить исполнение");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className={styles.shell} id="swap">
      <div className={styles.dock}>
        <div className={styles.card}>
          <div className={styles.head}>
            <div>
              <div className={styles.productTitle}>Перевод Армения → Россия</div>
              <div className={styles.productHint}>Вы задаёте сумму, Pay3Flow находит маршрут</div>
            </div>
            <span className={styles.statusPill}>{order ? STATUS_RU[order.status] ?? order.status : "Новая заявка"}</span>
          </div>

          {!token && (
            <form className={styles.authBox} onSubmit={signIn} data-testid="auth-form">
              <strong>Вход или регистрация</strong>
              <p>Для демо используется одноразовый код 1234.</p>
              <input className={styles.textInput} type="email" value={authEmail} onChange={(event) => setAuthEmail(event.target.value)} required aria-label="Email" />
              <input className={styles.textInput} value={authCode} onChange={(event) => setAuthCode(event.target.value)} required aria-label="Код входа" />
              <button className={styles.secondaryButton} disabled={authBusy} type="submit">{authBusy ? "Входим…" : "Войти"}</button>
            </form>
          )}

          <label className={styles.fieldLabel}>
            Коридор
            <select className={styles.select} value={corridor?.id ?? ""} onChange={(event) => setCorridorId(event.target.value)} disabled={searching || Boolean(funding)}>
              {corridors.map((item) => (
                <option key={item.id} value={item.id}>{item.source_country}/{item.source_currency} → {item.target_country}/{item.target_currency}</option>
              ))}
            </select>
          </label>

          <div className={styles.panel}>
            <div className={styles.panelMain}>
              <label className={styles.slotLabel} htmlFor="exchange-amount">Вы отправляете</label>
              <input id="exchange-amount" className={styles.input} inputMode="decimal" value={amount} onChange={(event) => setAmount(event.target.value)} disabled={searching || Boolean(funding)} />
            </div>
            <span className={styles.currencyBadge}>{corridor?.source_currency ?? "—"}</span>
          </div>

          <div className={styles.twoColumns}>
            <label className={styles.fieldLabel}>Способ отправки<select className={styles.select} value={sourceMethod} onChange={(event) => setSourceMethod(event.target.value)}><option value="bank_card">Банковская карта</option><option value="bank_transfer">Банковский перевод</option><option value="p2p">P2P</option></select></label>
            <label className={styles.fieldLabel}>Способ получения<select className={styles.select} value={targetMethod} onChange={(event) => setTargetMethod(event.target.value)}><option value="bank_card">На карту</option><option value="bank_transfer">На счёт</option><option value="wallet">На кошелёк</option></select></label>
          </div>

          <label className={styles.fieldLabel}>Получатель<input className={styles.textInput} value={recipient} onChange={(event) => setRecipient(event.target.value)} placeholder="Имя или сохранённый recipient ID" /></label>

          {!funding && (
            <button type="button" className={styles.cta} disabled={busy || searching || !corridor} onClick={startSearch} data-testid="start-search">
              {(busy || searching) && <span className={styles.spinner} />}
              {searching ? "Ищем связки…" : order ? "Новый поиск" : "Найти маршрут"}
            </button>
          )}

          {selected && !funding && (
            <div className={styles.selectionBox} data-testid="selected-route">
              <strong>{selected.entry_asset} · {selected.entry_network}</strong>
              <span>Получатель получит {money(selected.target_amount_minor ?? 0, selected.target_currency ?? corridor?.target_currency ?? "")}</span>
              <span>Комиссия {money(selected.fee_minor ?? 0, corridor?.source_currency ?? "")}, ETA {selected.eta_minutes} мин</span>
              <button type="button" className={styles.cta} disabled={busy || selected.status !== "complete"} onClick={lockRoute}>Подтвердить полную связку</button>
            </div>
          )}

          {funding && order?.status !== "done" && (
            <div className={styles.fundingBox} data-testid="funding-instruction">
              <strong>Инструкция по оплате</strong>
              <span>{money(funding.amount_minor, funding.currency)} через {funding.method_type}</span>
              <span className={styles.destination}>{funding.destination_ref}</span>
              <label className={styles.consentLabel}>
                <input type="checkbox" checked={consent} onChange={(event) => setConsent(event.target.checked)} />
                Я подтверждаю инструкцию и условия версии {termsVersion}. Маршрут может использовать TOKEN/crypto как расчётный актив; Pay3Flow не списывает фиат автоматически.
              </label>
              <button type="button" className={styles.cta} disabled={!consent || busy} onClick={fundAndFinish} data-testid="confirm-funding">{busy ? "Выполняем…" : "Подтвердить funding"}</button>
            </div>
          )}

          {order?.status === "done" && <div className={styles.successBox} data-testid="order-done"><strong>Перевод завершён</strong><span>Mock settlement и proof успешно проверены.</span></div>}
          {socketFallback && searching && <div className={styles.warning}>WebSocket недоступен — результаты обновляются polling-запросами.</div>}
          {error && <div className={styles.errorBox} role="alert">{error}</div>}

          {history.length > 0 && (
            <details className={styles.history}>
              <summary>История операций ({history.length})</summary>
              {history.map((item) => <div key={item.id} className={styles.historyRow}><span>{money(item.source_amount_minor, item.source_currency)} → {item.target_currency}</span><strong>{STATUS_RU[item.status] ?? item.status}</strong></div>)}
            </details>
          )}
        </div>

        <SidePanel active={Boolean(order)} routes={routes} searching={searching} selectedQuoteId={selected?.quote_id ?? null} onSelect={setSelected} />
      </div>
    </section>
  );
}
