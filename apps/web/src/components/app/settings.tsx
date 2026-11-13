"use client";
import { useCallback, useEffect, useState } from "react";
import QRCode from "qrcode";
import { Laptop, Loader2, LogOut, ShieldCheck, ShieldOff, Smartphone } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { FieldError, Input, Label } from "@/components/ui/input";
import { Banner } from "@/components/auth/forms";
import { useAuth } from "@/components/auth/auth-provider";
import { api, problemMessage, type SessionInfo } from "@/lib/api";

function describeAgent(ua: string | null) {
  const s = ua ?? "";
  const browser = /Edg\//.test(s) ? "Edge" : /Chrome\//.test(s) ? "Chrome" : /Firefox\//.test(s) ? "Firefox" : /Safari\//.test(s) ? "Safari" : "Unknown browser";
  const os = /Android/.test(s) ? "Android" : /iPhone|iPad/.test(s) ? "iOS" : /Windows/.test(s) ? "Windows" : /Mac OS X/.test(s) ? "macOS" : /Linux/.test(s) ? "Linux" : "Unknown device";
  return { label: `${browser} on ${os}`, mobile: /Android|iPhone|iPad/.test(s) };
}

function Sessions() {
  const [items, setItems] = useState<SessionInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => { try { setItems((await api<{ sessions: SessionInfo[] }>("/v1/auth/sessions")).sessions); } catch (e) { setError(problemMessage(e)); } }, []);
  useEffect(() => {
    let alive = true;
    api<{ sessions: SessionInfo[] }>("/v1/auth/sessions").then((r) => { if (alive) setItems(r.sessions); }).catch((e) => { if (alive) setError(problemMessage(e)); });
    return () => { alive = false; };
  }, []);
  const revoke = async (id: string) => { try { await api(`/v1/auth/sessions/${id}`, { method: "DELETE" }); await load(); } catch (e) { setError(problemMessage(e)); } };
  const revokeOthers = async () => { try { await api("/v1/auth/sessions/revoke-others", { method: "POST", body: {} }); await load(); } catch (e) { setError(problemMessage(e)); } };
  return (
    <Card>
      <CardHeader><div><CardTitle>Active sessions</CardTitle><p className="mt-1 text-xs text-subtle">Devices currently signed in to your account.</p></div>
        {items && items.length > 1 && <Button variant="secondary" size="sm" onClick={revokeOthers}><LogOut />Sign out other devices</Button>}</CardHeader>
      <CardContent>
        {error && <Banner tone="error">{error}</Banner>}
        {!items && !error && <div className="skeleton h-14 w-full" />}
        <ul className="divide-y divide-border" aria-label="Active sessions">
          {items?.map((s) => { const d = describeAgent(s.userAgent); const Icon = d.mobile ? Smartphone : Laptop; return (
            <li key={s.id} className="flex items-center gap-4 py-3.5">
              <span className="flex size-10 items-center justify-center rounded-lg bg-bg-subtle text-muted"><Icon className="size-5" /></span>
              <div className="min-w-0 flex-1"><p className="text-sm font-medium">{d.label} {s.current && <Badge tone="accent" className="ml-1.5">This device</Badge>}</p><p className="truncate text-xs text-subtle">{s.ip ?? "Unknown IP"} · last active {new Date(s.lastSeenAt).toLocaleString()}</p></div>
              {!s.current && <Button variant="ghost" size="sm" onClick={() => revoke(s.id)} aria-label={`Sign out ${d.label}`}>Sign out</Button>}
            </li>); })}
        </ul>
      </CardContent>
    </Card>
  );
}

function TwoFactor() {
  const { me, refresh } = useAuth();
  const enabled = !!me?.user.mfaEnabled;
  const [setup, setSetup] = useState<{ secret: string; qr: string } | null>(null);
  const [code, setCode] = useState("");
  const [codes, setCodes] = useState<string[] | null>(null);
  const [password, setPassword] = useState("");
  const [disabling, setDisabling] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function start() {
    setBusy(true); setError(null);
    try {
      const r = await api<{ secret: string; otpauthUri: string }>("/v1/auth/mfa/setup", { method: "POST", body: {} });
      setSetup({ secret: r.secret, qr: await QRCode.toDataURL(r.otpauthUri, { margin: 1, width: 176 }) });
    } catch (e) { setError(problemMessage(e)); } finally { setBusy(false); }
  }
  async function confirm() {
    setBusy(true); setError(null);
    try { const r = await api<{ recoveryCodes: string[] }>("/v1/auth/mfa/enable", { method: "POST", body: { code } }); setCodes(r.recoveryCodes); setSetup(null); setCode(""); await refresh(); }
    catch (e) { setError(problemMessage(e)); } finally { setBusy(false); }
  }
  async function disable() {
    setBusy(true); setError(null);
    try { await api("/v1/auth/mfa/disable", { method: "POST", body: { password, code } }); setDisabling(false); setPassword(""); setCode(""); await refresh(); }
    catch (e) { setError(problemMessage(e)); } finally { setBusy(false); }
  }

  return (
    <Card>
      <CardHeader><div><CardTitle>Two-factor authentication</CardTitle><p className="mt-1 text-xs text-subtle">Require a code from an authenticator app when you log in.</p></div>
        <Badge tone={enabled ? "accent" : "neutral"}>{enabled ? <><ShieldCheck className="size-3" />Enabled</> : <><ShieldOff className="size-3" />Off</>}</Badge></CardHeader>
      <CardContent className="space-y-4">
        {error && <Banner tone="error">{error}</Banner>}
        {codes && (
          <div className="rounded-lg border border-border-strong bg-bg-subtle p-4" data-testid="recovery-codes">
            <p className="text-sm font-medium">Save your recovery codes</p>
            <p className="mt-1 text-xs text-muted">Each code works once if you lose your device. They are shown only now — store them somewhere safe.</p>
            <ul className="mt-3 grid grid-cols-2 gap-2 font-mono text-sm">{codes.map((c) => <li key={c}>{c}</li>)}</ul>
            <Button variant="secondary" size="sm" className="mt-4" onClick={() => setCodes(null)}>I have saved them</Button>
          </div>
        )}
        {!enabled && !setup && !codes && <Button variant="navy" onClick={start} disabled={busy}>{busy && <Loader2 className="animate-spin" />}Set up two-factor authentication</Button>}
        {setup && (
          <div className="grid gap-5 sm:grid-cols-[auto_1fr]">
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img src={setup.qr} alt="QR code for your authenticator app" width={176} height={176} className="rounded-lg border border-border bg-white" />
            <div className="space-y-3">
              <p className="text-sm text-muted">Scan the QR code, or enter this key manually:</p>
              <p className="break-all rounded-md bg-bg-subtle px-3 py-2 font-mono text-sm" data-testid="mfa-secret">{setup.secret}</p>
              <div><Label htmlFor="totp">6-digit code</Label><Input id="totp" inputMode="numeric" autoComplete="one-time-code" value={code} onChange={(e) => setCode(e.target.value)} /></div>
              <Button variant="navy" onClick={confirm} disabled={busy || code.length !== 6}>{busy && <Loader2 className="animate-spin" />}Turn on</Button>
            </div>
          </div>
        )}
        {enabled && !disabling && !codes && <Button variant="secondary" onClick={() => { setDisabling(true); setError(null); }}>Turn off…</Button>}
        {enabled && disabling && (
          <div className="max-w-sm space-y-3">
            <p className="text-sm text-muted">Confirm with your password and a current code (or a recovery code).</p>
            <div><Label htmlFor="pw">Password</Label><Input id="pw" type="password" autoComplete="current-password" value={password} onChange={(e) => setPassword(e.target.value)} /></div>
            <div><Label htmlFor="c2">Code</Label><Input id="c2" autoComplete="one-time-code" value={code} onChange={(e) => setCode(e.target.value)} /></div>
            <div className="flex gap-2"><Button variant="danger" onClick={disable} disabled={busy || !password || !code}>Turn off</Button><Button variant="ghost" onClick={() => setDisabling(false)}>Cancel</Button></div>
            <FieldError>{undefined}</FieldError>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export function Settings() {
  const { me } = useAuth();
  return (
    <div className="space-y-6">
      <div><h1 className="font-display text-4xl">Settings</h1><p className="mt-1 text-sm text-muted">Your profile and account security.</p></div>
      <Card><CardHeader><CardTitle>Profile</CardTitle></CardHeader>
        <CardContent><dl className="grid gap-4 sm:grid-cols-2"><div><dt className="text-xs text-subtle">Name</dt><dd className="mt-1 text-sm font-medium">{me?.user.fullName}</dd></div><div><dt className="text-xs text-subtle">Email</dt><dd className="mt-1 text-sm font-medium">{me?.user.email}</dd></div></dl></CardContent></Card>
      <TwoFactor />
      <Sessions />
    </div>
  );
}
