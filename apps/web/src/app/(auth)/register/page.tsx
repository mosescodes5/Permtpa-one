import type { Metadata } from "next";
import Link from "next/link";
import { AuthShell } from "@/components/auth/auth-shell";
import { RegisterForm } from "@/components/auth/forms";

export const metadata: Metadata = { title: "Create account" };
export default function Page() {
  return <AuthShell title="Create your workspace" subtitle="Free to start. No card required." footer={<>Already have an account? <Link href="/login" className="font-medium text-accent hover:underline">Log in</Link></>}><RegisterForm /></AuthShell>;
}
