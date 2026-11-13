import type { Metadata } from "next";
import { Suspense } from "react";
import Link from "next/link";
import { AuthShell } from "@/components/auth/auth-shell";
import { ResetForm } from "@/components/auth/forms";

export const metadata: Metadata = { title: "Choose a new password", robots: { index: false } };
export default function Page() {
  return <AuthShell title="Choose a new password" subtitle="Use at least 12 characters. A long passphrase works well." footer={<Link href="/login" className="font-medium text-accent hover:underline">Back to log in</Link>}><Suspense><ResetForm /></Suspense></AuthShell>;
}
