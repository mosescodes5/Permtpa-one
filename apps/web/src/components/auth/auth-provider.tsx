"use client";
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { usePathname, useRouter } from "next/navigation";
import { ApiProblem, api, setCsrfToken, type Me } from "@/lib/api";

type State = { status: "loading" | "authenticated" | "anonymous"; me: Me | null };
type Membership = Me["memberships"][number];
type Ctx = State & { refresh: () => Promise<void>; logout: () => Promise<void>; activeOrg: Membership | null; setActiveOrg: (orgId: string) => void };
const ORG_KEY = "permtpa-one:active-org"; // UI preference only; the API re-verifies membership on every request.
const AuthCtx = createContext<Ctx | null>(null);

export function useAuth() {
  const c = useContext(AuthCtx);
  if (!c) throw new Error("useAuth must be used inside <AuthProvider>");
  return c;
}

/** UI-level gate only. Real authorization is enforced by the API on every request. */
export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [state, setState] = useState<State>({ status: "loading", me: null });
  const [activeId, setActiveId] = useState<string | null>(() => { try { return typeof window === "undefined" ? null : localStorage.getItem(ORG_KEY); } catch { return null; } });
  const router = useRouter();
  const path = usePathname();
  const explicitLogout = useRef(false); // after a deliberate logout we must not add ?next= to the login URL

  const refresh = useCallback(async () => {
    try {
      setState({ status: "authenticated", me: await api<Me>("/v1/me") });
    } catch (e) {
      if (e instanceof ApiProblem && e.status === 401) setState({ status: "anonymous", me: null });
      else setState((s) => (s.me ? s : { status: "anonymous", me: null }));
    }
  }, []);

  useEffect(() => {
    let alive = true;
    api<Me>("/v1/me")
      .then((me) => { if (alive) setState({ status: "authenticated", me }); })
      .catch(() => { if (alive) setState((s) => (s.me ? s : { status: "anonymous", me: null })); });
    return () => { alive = false; };
  }, []);

  useEffect(() => {
    if (state.status === "anonymous") router.replace(explicitLogout.current ? "/login" : `/login?next=${encodeURIComponent(path)}`);
    else if (state.status === "authenticated" && state.me && state.me.memberships.length === 0 && path !== "/onboarding") router.replace("/onboarding");
  }, [state, path, router]);

  const logout = useCallback(async () => {
    explicitLogout.current = true;
    try { await api("/v1/auth/logout", { method: "POST", body: {} }); } catch { /* cookie may already be invalid */ }
    setCsrfToken(null);
    setState({ status: "anonymous", me: null });
    router.replace("/login");
  }, [router]);

  const setActiveOrg = useCallback((orgId: string) => { setActiveId(orgId); try { localStorage.setItem(ORG_KEY, orgId); } catch { /* storage unavailable */ } }, []);
  const activeOrg = state.me ? (state.me.memberships.find((m) => m.orgId === activeId) ?? state.me.memberships[0] ?? null) : null;
  const value = useMemo<Ctx>(() => ({ ...state, refresh, logout, activeOrg, setActiveOrg }), [state, refresh, logout, activeOrg, setActiveOrg]);
  return <AuthCtx.Provider value={value}>{children}</AuthCtx.Provider>;
}

export function AuthGate({ children }: { children: React.ReactNode }) {
  const { status } = useAuth();
  if (status !== "authenticated") {
    return <div className="grid min-h-dvh place-items-center" role="status" aria-live="polite"><span className="sr-only">Loading your workspace…</span><div className="skeleton h-3 w-40" /></div>;
  }
  return <>{children}</>;
}
