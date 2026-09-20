"use client";

import { useEffect, useRef } from "react";

import { Converter } from "@/components/home/converter";
import { Header } from "@/components/home/header";
import { generatePuzzleBackground } from "@/lib/puzzle-background";

export default function HomePage() {
  const shellRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    shellRef.current?.style.setProperty("--puzzle-pattern", generatePuzzleBackground());
  }, []);

  return (
    <div className="appShell" ref={shellRef}>
      <Header brandHref="/" />
      <main>
        <Converter />
      </main>
      <footer className="siteFooter">
        <span>Pay3Flow</span>
        <span>Live routing infrastructure · Public market estimates</span>
      </footer>
    </div>
  );
}
