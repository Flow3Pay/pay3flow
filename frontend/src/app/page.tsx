"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { Converter } from "@/components/home/converter";
import { Header } from "@/components/home/header";
import { generatePuzzleBackground } from "@/lib/puzzle-background";

export default function HomePage() {
  const shellRef = useRef<HTMLDivElement>(null);
  // Read browser storage after hydration. Reading it in the initial state
  // makes the server render signed out while the first client render is
  // signed in, which causes React hydration error #418.
  const [token, setToken] = useState<string | null>(null);
  const [email, setEmail] = useState("");

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setToken(window.localStorage.getItem("pay3flow-token"));
      setEmail(window.localStorage.getItem("pay3flow-email") ?? "");
    }, 0);
    return () => window.clearTimeout(timer);
  }, []);

  useEffect(() => {
    shellRef.current?.style.setProperty("--puzzle-pattern", generatePuzzleBackground());
  }, []);

  const handleAuthenticated = useCallback((freshToken: string, freshEmail: string) => {
    window.localStorage.setItem("pay3flow-token", freshToken);
    window.localStorage.setItem("pay3flow-email", freshEmail);
    setToken(freshToken);
    setEmail(freshEmail);
  }, []);

  const handleConnect = useCallback(() => {
    document.querySelector<HTMLInputElement>("[data-testid='auth-form'] input[type='email']")?.focus();
  }, []);

  const handleDisconnect = useCallback(() => {
    window.localStorage.removeItem("pay3flow-token");
    window.localStorage.removeItem("pay3flow-email");
    setToken(null);
    setEmail("");
  }, []);

  return (
    <div className="appShell" ref={shellRef}>
      <Header
        connected={Boolean(token)}
        address={email || "Profile"}
        onConnect={handleConnect}
        onDisconnect={handleDisconnect}
        brandHref="/"
      />
      <main>
        <Converter
          token={token}
          email={email}
          onAuthenticated={handleAuthenticated}
          onRequireAuth={handleConnect}
        />
      </main>
      <footer className="siteFooter">
        <span>Pay3Flow</span>
        <span>Live routing infrastructure · Public market estimates</span>
      </footer>
    </div>
  );
}
