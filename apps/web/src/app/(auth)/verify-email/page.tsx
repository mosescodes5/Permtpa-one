import type { Metadata } from "next";
import { Suspense } from "react";
import Link from "next/link";
import { AuthShell } from "@/components/auth/auth-shell";
import { VerifyEmail } from "@/components/auth/verify-email";

export const metadata: Metadata = { title: "Verify email", robots: { index: false } };
export default function Page() {
  return <AuthShell title="Verify your email" subtitle="One quick step to activate your account." footer={<Link href="/login" className="font-medium text-accent hover:underline">Back to log in</Link>}><Suspense><VerifyEmail /></Suspense></AuthShell>;
}
