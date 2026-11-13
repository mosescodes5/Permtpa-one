"use client";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useEffect, useRef, useState } from "react";
import { CheckCircle2, Loader2, TriangleAlert } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { api, problemMessage } from "@/lib/api";

/** Verification is a POST triggered by the page (never by a GET), so mail scanners that prefetch links cannot consume the token. */
export function VerifyEmail() {
  const token = useSearchParams().get("token");
  const started = useRef(false);
  const [state, setState] = useState<"working" | "ok" | "error">(token ? "working" : "error");
  const [msg, setMsg] = useState(token ? "" : "This verification link is incomplete.");

  useEffect(() => {
    if (!token || started.current) return;
    started.current = true;
    api("/v1/auth/verify-email", { method: "POST", body: { token } })
      .then(() => setState("ok"))
      .catch((e) => { setMsg(problemMessage(e)); setState("error"); });
  }, [token]);

  return (
    <div role="status" aria-live="polite" className="rounded-2xl border border-border-strong bg-surface p-8 text-center shadow-sm">
      {state === "working" && <><Loader2 className="mx-auto size-8 animate-spin text-accent" /><p className="mt-4 text-sm text-muted">Verifying your email…</p></>}
      {state === "ok" && <><CheckCircle2 className="mx-auto size-10 text-accent" /><h2 className="mt-4 text-lg font-semibold">Email verified</h2><p className="mt-1 text-sm text-muted">Your account is ready.</p><Link href="/login" className="mt-6 block"><Button variant="navy" size="lg" className="w-full">Log in</Button></Link></>}
      {state === "error" && <><TriangleAlert className="mx-auto size-10 text-danger" /><h2 className="mt-4 text-lg font-semibold">We couldn&apos;t verify that link</h2><p className="mt-1 text-sm text-muted">{msg}</p><Link href="/login" className="mt-6 block"><Button variant="secondary" size="lg" className="w-full">Back to log in</Button></Link></>}
    </div>
  );
}
