import type { Metadata } from "next";
import Link from "next/link";
import { AuthShell } from "@/components/auth/auth-shell";
import { ForgotForm } from "@/components/auth/forms";

export const metadata: Metadata = { title: "Reset password" };
export default function Page() {
  return <AuthShell title="Reset your password" subtitle="Enter your email and we'll send a secure reset link." footer={<Link href="/login" className="font-medium text-accent hover:underline">Back to log in</Link>}><ForgotForm /></AuthShell>;
}
