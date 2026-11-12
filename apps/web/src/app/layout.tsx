import type { Metadata, Viewport } from "next";
import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "@fontsource/instrument-serif";
import "./globals.css";
import { ThemeProvider } from "@/components/theme-provider";

export const metadata: Metadata = {
  title: { default: "PERMTPA ONE — One business. One platform. Total control.", template: "%s · PERMTPA ONE" },
  description: "PERMTPA ONE is the business operating system by PERMTPA: sales, inventory, customers, finance and intelligence in one secure platform.",
  applicationName: "PERMTPA ONE",
  authors: [{ name: "PERMTPA" }],
};

export const viewport: Viewport = {
  themeColor: [{ media: "(prefers-color-scheme: light)", color: "#faf8f4" }, { media: "(prefers-color-scheme: dark)", color: "#070d1a" }],
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body>
        <ThemeProvider>{children}</ThemeProvider>
      </body>
    </html>
  );
}
