"use client";
import { Suspense, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { AuthShell } from "@/components/auth/auth-shell";
import { AuthGate, useAuth } from "@/components/auth/auth-provider";
import { Banner } from "@/components/auth/forms";
import { Button } from "@/components/ui/button";
import { Loader2, MailCheck } from "@/components/icons";
import { ApiProblem, api, problemMessage } from "@/lib/api";

function Accept() {
  const token = useSearchParams().get("token");
  const { me, refresh, setActiveOrg } = useAuth();
  const router = useRouter();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function accept() {
    setBusy(true); setError(null);
    try { const r = await api<{ orgId: string }>("/v1/invitations/accept", { method: "POST", body: { token } }); setActiveOrg(r.orgId); await refresh(); router.replace("/dashboard"); }
    catch (e) { setError(e instanceof ApiProblem && e.code === "invalid_token" ? "This invitation is invalid, has expired, or was sent to a different email address. Check that you are signed in with the address it was sent to." : problemMessage(e)); setBusy(false); }
  }
  if (!token) return <Banner tone="error">This invitation link is incomplete. Ask the person who invited you to send it again.</Banner>;
  return (
    <div className="rounded-2xl border border-border-strong bg-surface p-6 text-center shadow-sm">
      <span className="mx-auto flex size-12 items-center justify-center rounded-full bg-accent-soft text-accent"><MailCheck className="size-6" /></span>
      <h2 className="mt-4 text-lg font-semibold">You have been invited</h2>
      <p className="mt-2 text-sm text-muted">You are signed in as <strong className="font-medium text-fg">{me?.user.email}</strong>. Invitations only work for the address they were sent to.</p>
      {error && <div className="mt-4 text-left"><Banner tone="error">{error}</Banner></div>}
      <Button variant="navy" size="lg" className="mt-5 w-full" onClick={accept} disabled={busy}>{busy && <Loader2 className="animate-spin" />}Accept invitation</Button>
      <Link href="/dashboard" className="mt-4 inline-block text-sm text-muted hover:text-fg">Not now</Link>
    </div>
  );
}

export default function Page() {
  return (
    <AuthGate>
      <AuthShell title="Join a team" subtitle="Accept your invitation to start working together." footer={<>Wrong account? Log out and open the link again.</>}>
        <Suspense><Accept /></Suspense>
      </AuthShell>
    </AuthGate>
  );
}
