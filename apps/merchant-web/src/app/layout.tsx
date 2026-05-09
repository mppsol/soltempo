import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "soltempo · merchant dashboard",
  description: "Cross-VM settlement dashboard — Tempo Moderato + Solana devnet",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
