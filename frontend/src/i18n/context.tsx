"use client";

import { createContext, useContext, useEffect, useState, type ReactNode } from "react";

import { dicts, type Dict, type Lang } from "./dicts";

interface I18nContextValue {
  lang: Lang;
  setLang: (lang: Lang) => void;
  d: Dict;
}

const I18nContext = createContext<I18nContextValue | null>(null);

const STORAGE_KEY = "pay3flow-lang";

export function LanguageProvider({ children }: { children: ReactNode }) {
  const [lang, setLang] = useState<Lang>("en");

  useEffect(() => {
    try {
      const saved = window.localStorage.getItem(STORAGE_KEY);
      // eslint-disable-next-line react-hooks/set-state-in-effect -- hydrate saved language once on mount
      if (saved === "en" || saved === "ru") setLang(saved);
    } catch {
      // ignore localStorage access errors
    }
  }, []);

  const switchLang = (next: Lang) => {
    setLang(next);
    try {
      window.localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // ignore localStorage access errors
    }
  };

  return (
    <I18nContext.Provider value={{ lang, setLang: switchLang, d: dicts[lang] }}>
      {children}
    </I18nContext.Provider>
  );
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used inside <LanguageProvider>");
  return ctx;
}

export function useI18nSafe(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (ctx) return ctx;
  return { lang: "en", setLang: () => undefined, d: dicts.en };
}