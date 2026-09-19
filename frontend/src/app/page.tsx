"use client";

import { useCallback, useState } from "react";

import { Converter } from "@/components/home/converter";
import { Header } from "@/components/home/header";

export default function HomePage() {
  const [token, setToken] = useState<string | null>(() =>
    typeof window === "undefined" ? null : window.localStorage.getItem("pay3flow-token"),
  );
  const [email, setEmail] = useState(() =>
    typeof window === "undefined" ? "" : window.localStorage.getItem("pay3flow-email") ?? "",
  );

  const handleAuthenticated = useCallback((freshToken: string, freshEmail: string) => {
    window.localStorage.setItem("pay3flow-token", freshToken);
    window.localStorage.setItem("pay3flow-email", freshEmail);
    setToken(freshToken);
    setEmail(freshEmail);
  }, []);

  const handleConnect = useCallback(() => {
    document.querySelector("[data-testid='auth-form']")?.scrollIntoView({ behavior: "smooth" });
  }, []);

  const handleDisconnect = useCallback(() => {
    window.localStorage.removeItem("pay3flow-token");
    window.localStorage.removeItem("pay3flow-email");
    setToken(null);
    setEmail("");
  }, []);

  return (
    <div className="cowTheme">
      <Header
        connected={Boolean(token)}
        address={email || "Профиль"}
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
    </div>
  );
}
