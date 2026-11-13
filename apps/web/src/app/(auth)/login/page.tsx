import type { Metadata } from "next";
import Link from "next/link";
import { Suspense } from "react";
import { AuthShell } from "@/components/auth/auth-shell";
import { LoginForm } from "@/components/auth/forms";

export const metadata: Metadata = { title: "Log in" };
export default function Page() {
  return <AuthShell title="Welcome back" subtitle="Log in to your PERMTPA ONE workspace." footer={<>New to PERMTPA ONE? <Link href="/register" className="font-medium text-accent hover:underline">Create an account</Link></>}><Suspense><LoginForm /></Suspense></AuthShell>;
}
