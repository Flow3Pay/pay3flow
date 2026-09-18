import type { Metadata } from "next";
import { Inter, Syne_Tactile } from "next/font/google";
import "./globals.css";

const inter = Inter({
  variable: "--font-inter",
  subsets: ["latin", "cyrillic"],
  display: "swap",
});

const syneTactile = Syne_Tactile({
  variable: "--font-syne-tactile",
  weight: "400",
  display: "swap",
});

export const metadata: Metadata = {
  title: "Pay3Flow",
  description: "Cross-border payments via acquiring",
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" className={`${inter.variable} ${syneTactile.variable}`}>
      <body>{children}</body>
    </html>
  );
}