"use client";

import { useCallback, useRef, useState } from "react";

import { Converter } from "@/components/home/converter";
import { Header } from "@/components/home/header";

const DEMO_ADDRESS = "0x1a2b3c…9fE4";

const NAV_LINKS = [{ href: "/", label: "Home" }];

export default function HomePage() {
  const [connected, setConnected] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const connectTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleConnect = useCallback(() => {
    if (connected || connecting) return;
    setConnecting(true);
    if (connectTimer.current) clearTimeout(connectTimer.current);
    connectTimer.current = setTimeout(() => {
      setConnected(true);
      setConnecting(false);
    }, 750);
  }, [connected, connecting]);

  const handleDisconnect = useCallback(() => {
    setConnected(false);
  }, []);

  return (
    <div className="cowTheme">
      <Header
        connected={connected}
        address={DEMO_ADDRESS}
        onConnect={handleConnect}
        onDisconnect={handleDisconnect}
        links={NAV_LINKS}
        brandHref="/"
      />
      <main>
        <Converter connected={connected} connecting={connecting} onConnect={handleConnect} />
      </main>
    </div>
  );
}