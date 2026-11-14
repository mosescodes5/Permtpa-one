"use client";
import { useEffect, useState } from "react";
import { Loader2 } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input, Label } from "@/components/ui/input";
import { Banner } from "@/components/auth/forms";
import { useAuth } from "@/components/auth/auth-provider";
import { BUSINESS_TYPES, ROLE_LABELS, api, problemMessage, type OrgProfile } from "@/lib/api";

export function Organization() {
  const { activeOrg, refresh } = useAuth();
  const orgId = activeOrg?.orgId;
  const [org, setOrg] = useState<OrgProfile | null>(null);
  const [name, setName] = useState("");
  const [type, setType] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    if (!orgId) return;
    api<OrgProfile>(`/v1/orgs/${orgId}`).then((o) => { if (alive) { setOrg(o); setName(o.name); setType(o.businessType); } }).catch((e) => { if (alive) setError(problemMessage(e)); });
    return () => { alive = false; };
  }, [orgId]);

  const canEdit = org?.yourRole === "owner" || org?.yourRole === "administrator";
  async function save(e: React.FormEvent) {
    e.preventDefault(); setBusy(true); setError(null); setSaved(false);
    try { const o = await api<OrgProfile>(`/v1/orgs/${orgId}`, { method: "PATCH", body: { name, businessType: type } }); setOrg(o); await refresh(); setSaved(true); }
    catch (x) { setError(problemMessage(x)); } finally { setBusy(false); }
  }
  return (
    <div className="space-y-6">
      <div><h1 className="font-display text-4xl">Organization</h1><p className="mt-1 text-sm text-muted">Your business profile.</p></div>
      {error && <Banner tone="error">{error}</Banner>}
      {saved && <Banner tone="success">Your changes were saved.</Banner>}
      <Card>
        <CardHeader><CardTitle>Business profile</CardTitle></CardHeader>
        <CardContent>
          {!org ? <div className="skeleton h-24 w-full" /> : (
            <form noValidate onSubmit={save} className="grid max-w-xl gap-4">
              <div><Label htmlFor="org-name">Business name</Label><Input id="org-name" value={name} disabled={!canEdit} onChange={(e) => setName(e.target.value)} /></div>
              <div><Label htmlFor="org-type">Business type</Label>
                <select id="org-type" value={type} disabled={!canEdit} onChange={(e) => setType(e.target.value)} className="h-11 w-full cursor-pointer rounded-lg border border-border-strong bg-surface px-3 text-sm shadow-sm focus-visible:border-accent focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-60">
                  {BUSINESS_TYPES.map(([v, l]) => <option key={v} value={v}>{l}</option>)}</select></div>
              <dl className="grid grid-cols-2 gap-4 text-sm"><div><dt className="text-xs text-subtle">Currency</dt><dd className="mt-1 font-medium">{org.currency}</dd></div><div><dt className="text-xs text-subtle">Your role</dt><dd className="mt-1 font-medium">{ROLE_LABELS[org.yourRole]}</dd></div></dl>
              {canEdit ? <Button type="submit" variant="navy" size="lg" className="w-fit" disabled={busy}>{busy && <Loader2 className="animate-spin" />}Save changes</Button> : <p className="text-xs text-subtle">Only owners and administrators can edit the business profile.</p>}
            </form>)}
        </CardContent>
      </Card>
    </div>
  );
}
