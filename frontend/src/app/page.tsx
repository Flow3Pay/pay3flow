"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { Converter } from "@/components/home/converter";
import { Header } from "@/components/home/header";
import { generatePuzzleBackground } from "@/lib/puzzle-background";

export default function HomePage() {
  const shellRef = useRef<HTMLDivElement>(null);
  const [token, setToken] = useState<string | null>(() =>
    typeof window === "undefined" ? null : window.localStorage.getItem("pay3flow-token"),
  );
  const [email, setEmail] = useState(() =>
    typeof window === "undefined" ? "" : window.localStorage.getItem("pay3flow-email") ?? "",
  );

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
