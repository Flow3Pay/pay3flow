"use client";

import { Business } from "@/components/home/business";
import { Features } from "@/components/home/features";
import { Footer } from "@/components/home/footer";
import { Header } from "@/components/home/header";
import { Hero } from "@/components/home/hero";
import { Technology } from "@/components/home/technology";
import { LanguageProvider } from "@/i18n/context";

export default function Home() {
  return (
    <LanguageProvider>
      <div className="cowTheme">
        <Header
          landing
          connected={false}
          address=""
          onConnect={() => {}}
          onDisconnect={() => {}}
        />
        <Hero />
        <main>
          <Features />
          <Technology />
          <Business />
        </main>
        <Footer />
      </div>
    </LanguageProvider>
  );
}