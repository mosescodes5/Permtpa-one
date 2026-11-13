import type { Metadata } from "next";
import { AuthGate, AuthProvider } from "@/components/auth/auth-provider";
import { AppShell } from "@/components/app/app-shell";

export const metadata: Metadata = { robots: { index: false, follow: false } };

export default function AppLayout({ children }: { children: React.ReactNode }) {
  return <AuthProvider><AuthGate><AppShell>{children}</AppShell></AuthGate></AuthProvider>;
}
