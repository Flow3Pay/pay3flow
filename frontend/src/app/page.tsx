"use client";

import { useEffect, useMemo, useRef, useSyncExternalStore } from "react";

import { Converter } from "@/components/home/converter";
import { Header } from "@/components/home/header";
import { RouteLocator } from "@/components/home/route-locator";
import { generatePuzzleBackground } from "@/lib/puzzle-background";
import { routeFromLocatorHash } from "@/lib/route-locator";

export default function HomePage() {
  const shellRef = useRef<HTMLDivElement>(null);
  const locatorHash = useSyncExternalStore(
    (onChange) => {
      window.addEventListener("hashchange", onChange);
      return () => window.removeEventListener("hashchange", onChange);
    },
    () => window.location.hash,
    () => "",
  );
  const locatorRoute = useMemo(() => routeFromLocatorHash(locatorHash), [locatorHash]);

  useEffect(() => {
    shellRef.current?.style.setProperty("--puzzle-pattern", generatePuzzleBackground());
  }, []);

  return (
    <div className="appShell" ref={shellRef}>
      <Header brandHref="/" />
      <main>
        {locatorRoute ? <RouteLocator route={locatorRoute} /> : <Converter />}
      </main>
      <footer className="siteFooter">
        <span>Pay3Flow</span>
        <span>Live routing infrastructure · Public market estimates</span>
      </footer>
    </div>
  );
}
