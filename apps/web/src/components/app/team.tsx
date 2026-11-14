"use client";
import { useCallback, useEffect, useState } from "react";
import { Loader2, MailCheck, Users, X } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { FieldError, Input, Label } from "@/components/ui/input";
import { Banner } from "@/components/auth/forms";
import { useAuth } from "@/components/auth/auth-provider";
import { ASSIGNABLE_ROLES, ROLE_HELP, ROLE_LABELS, api, problemMessage, type Invitation, type Member } from "@/lib/api";
import { inviteSchema } from "@/lib/validation";

const selectCls = "h-10 cursor-pointer rounded-lg border border-border-strong bg-surface px-2.5 text-sm shadow-sm focus-visible:border-accent focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50";
const initials = (n: string) => n.split(/\s+/).filter(Boolean).slice(0, 2).map((p) => p[0]!.toUpperCase()).join("");

export function Team() {
  const { me, activeOrg, refresh } = useAuth();
  const orgId = activeOrg?.orgId;
  const myRole = activeOrg?.role ?? "";
  const isAdmin = myRole === "owner" || myRole === "administrator";
  const isOwner = myRole === "owner";
  const [members, setMembers] = useState<Member[] | null>(null);
  const [invites, setInvites] = useState<Invitation[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!orgId) return;
    const m = await api<{ members: Member[] }>(`/v1/orgs/${orgId}/members`);
    setMembers(m.members);
    if (isAdmin) setInvites((await api<{ invitations: Invitation[] }>(`/v1/orgs/${orgId}/invitations`)).invitations);
  }, [orgId, isAdmin]);

  useEffect(() => {
    let alive = true;
    if (!orgId) return;
    (async () => {
      try {
        const m = await api<{ members: Member[] }>(`/v1/orgs/${orgId}/members`);
        const inv = isAdmin ? (await api<{ invitations: Invitation[] }>(`/v1/orgs/${orgId}/invitations`)).invitations : [];
        if (alive) { setMembers(m.members); setInvites(inv); }
      } catch (e) { if (alive) setError(problemMessage(e)); }
    })();
    return () => { alive = false; };
  }, [orgId, isAdmin]);

  async function act(key: string, fn: () => Promise<unknown>, done?: string) {
    setBusy(key); setError(null); setNotice(null);
    try { await fn(); await load(); await refresh(); if (done) setNotice(done); } catch (e) { setError(problemMessage(e)); } finally { setBusy(null); setConfirm(null); }
  }

  const setRole = (m: Member, role: string) => act(`role-${m.userId}`, () => api(`/v1/orgs/${orgId}/members/${m.userId}`, { method: "PATCH", body: { role } }), `${m.fullName} is now ${ROLE_LABELS[role]}.`);
  const setStatus = (m: Member, status: string) => act(`st-${m.userId}`, () => api(`/v1/orgs/${orgId}/members/${m.userId}`, { method: "PATCH", body: { status } }), status === "suspended" ? `${m.fullName} was suspended.` : `${m.fullName} was reinstated.`);
  const remove = (m: Member) => act(`rm-${m.userId}`, () => api(`/v1/orgs/${orgId}/members/${m.userId}`, { method: "DELETE" }), `${m.fullName} was removed.`);
  const revoke = (i: Invitation) => act(`rv-${i.id}`, () => api(`/v1/orgs/${orgId}/invitations/${i.id}`, { method: "DELETE" }), `Invitation to ${i.email} was revoked.`);

  return (
    <div className="space-y-6">
      <div><h1 className="font-display text-4xl">Team</h1><p className="mt-1 text-sm text-muted">People who can access {activeOrg?.name ?? "your business"}, and what they can do.</p></div>
      {error && <Banner tone="error">{error}</Banner>}
      {notice && <Banner tone="success">{notice}</Banner>}

      <Card>
        <CardHeader><div><CardTitle>Members</CardTitle><p className="mt-1 text-xs text-subtle">{members ? `${members.length} ${members.length === 1 ? "person" : "people"}` : "Loading…"}</p></div></CardHeader>
        <CardContent>
          {!members && <div className="skeleton h-16 w-full" />}
          <ul className="divide-y divide-border" aria-label="Team members">
            {members?.map((m) => {
              const isMe = m.userId === me?.user.id;
              const lockedOwner = m.role === "owner" && !isOwner;
              const roles = isOwner ? ["owner", ...ASSIGNABLE_ROLES] : ASSIGNABLE_ROLES;
              return (
                <li key={m.userId} data-testid="member-row" className="flex flex-wrap items-center gap-4 py-4">
                  <span className="flex size-10 items-center justify-center rounded-full bg-accent-soft text-sm font-semibold text-accent">{initials(m.fullName)}</span>
                  <div className="min-w-0 flex-1">
                    <p className="text-sm font-medium">{m.fullName}{isMe && <Badge className="ml-2" tone="accent">You</Badge>}{m.status === "suspended" && <Badge className="ml-2" tone="warning">Suspended</Badge>}</p>
                    <p className="truncate text-xs text-subtle">{m.email ?? ROLE_HELP[m.role]}</p>
                  </div>
                  {isAdmin ? (
                    <div className="flex flex-wrap items-center gap-2">
                      <select aria-label={`Role for ${m.fullName}`} className={selectCls} value={m.role} disabled={lockedOwner || busy !== null} onChange={(e) => void setRole(m, e.target.value)}>
                        {(roles.includes(m.role as never) ? roles : [m.role, ...roles]).map((r) => <option key={r} value={r}>{ROLE_LABELS[r]}</option>)}
                      </select>
                      {!lockedOwner && (confirm === m.userId ? (
                        <>
                          <Button variant="danger" size="sm" disabled={busy !== null} onClick={() => void remove(m)}>{busy === `rm-${m.userId}` && <Loader2 className="animate-spin" />}Confirm remove</Button>
                          <Button variant="ghost" size="sm" onClick={() => setConfirm(null)}>Cancel</Button>
                        </>
                      ) : (
                        <>
                          <Button variant="ghost" size="sm" disabled={busy !== null} onClick={() => void setStatus(m, m.status === "active" ? "suspended" : "active")}>{m.status === "active" ? "Suspend" : "Reinstate"}</Button>
                          <Button variant="ghost" size="sm" aria-label={`Remove ${m.fullName}`} onClick={() => setConfirm(m.userId)}><X className="size-4" /></Button>
                        </>
                      ))}
                    </div>
                  ) : <Badge>{ROLE_LABELS[m.role]}</Badge>}
                </li>
              );
            })}
          </ul>
        </CardContent>
      </Card>

      {isAdmin && (
        <>
          <InviteForm orgId={orgId!} isOwner={isOwner} onSent={async (email) => { await load(); setNotice(`Invitation sent to ${email}.`); }} />
          {invites.length > 0 && (
            <Card>
              <CardHeader><div><CardTitle>Pending invitations</CardTitle><p className="mt-1 text-xs text-subtle">Links expire after 7 days and only work for the invited address.</p></div></CardHeader>
              <CardContent>
                <ul className="divide-y divide-border" aria-label="Pending invitations">
                  {invites.map((i) => (
                    <li key={i.id} className="flex items-center gap-4 py-3.5">
                      <span className="flex size-10 items-center justify-center rounded-lg bg-bg-subtle text-muted"><MailCheck className="size-5" /></span>
                      <div className="min-w-0 flex-1"><p className="truncate text-sm font-medium">{i.email}</p><p className="text-xs text-subtle">{ROLE_LABELS[i.role]} · expires {new Date(i.expiresAt).toLocaleDateString()}</p></div>
                      <Button variant="ghost" size="sm" disabled={busy !== null} aria-label={`Revoke invitation for ${i.email}`} onClick={() => void revoke(i)}>Revoke</Button>
                    </li>
                  ))}
                </ul>
              </CardContent>
            </Card>
          )}
        </>
      )}
      {!isAdmin && <p className="flex items-center gap-2 text-xs text-subtle"><Users className="size-4" />Only owners and administrators can invite people or change roles.</p>}
    </div>
  );
}

function InviteForm({ orgId, isOwner, onSent }: { orgId: string; isOwner: boolean; onSent: (email: string) => Promise<void> }) {
  const [email, setEmail] = useState("");
  const [role, setRole] = useState("manager");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  async function submit(e: React.FormEvent) {
    e.preventDefault(); setErr(null);
    const parsed = inviteSchema.safeParse({ email, role });
    if (!parsed.success) { setErr(parsed.error.issues[0]?.message ?? "Check the details."); return; }
    setBusy(true);
    try { await api(`/v1/orgs/${orgId}/invitations`, { method: "POST", body: parsed.data }); setEmail(""); await onSent(parsed.data.email); }
    catch (x) { setErr(problemMessage(x)); } finally { setBusy(false); }
  }
  void isOwner;
  return (
    <Card>
      <CardHeader><div><CardTitle>Invite someone</CardTitle><p className="mt-1 text-xs text-subtle">They will get an email with a link to join. Ownership cannot be granted by invitation.</p></div></CardHeader>
      <CardContent>
        <form noValidate onSubmit={submit} className="flex flex-col gap-3 sm:flex-row sm:items-end">
          <div className="flex-1"><Label htmlFor="invite-email">Email address</Label><Input id="invite-email" type="email" placeholder="teammate@business.com" value={email} onChange={(e) => setEmail(e.target.value)} aria-invalid={!!err} /></div>
          <div><Label htmlFor="invite-role">Role</Label>
            <select id="invite-role" className={`${selectCls} h-11 w-full sm:w-52`} value={role} onChange={(e) => setRole(e.target.value)}>{ASSIGNABLE_ROLES.map((r) => <option key={r} value={r}>{ROLE_LABELS[r]}</option>)}</select></div>
          <Button type="submit" variant="navy" size="lg" disabled={busy}>{busy && <Loader2 className="animate-spin" />}Send invitation</Button>
        </form>
        <p className="mt-2 text-xs text-subtle">{ROLE_HELP[role]}</p>
        <FieldError>{err ?? undefined}</FieldError>
      </CardContent>
    </Card>
  );
}
