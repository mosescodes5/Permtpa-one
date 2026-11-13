/** Typed client for the PERMTPA ONE API (same-origin via the Next proxy). Sessions live in an HttpOnly cookie; JS never sees it. */

export class ApiProblem extends Error {
  constructor(public status: number, public code: string, message: string) {
    super(message);
    this.name = "ApiProblem";
  }
}

let csrfToken: string | null = null;
export const setCsrfToken = (t: string | null) => { csrfToken = t; };

type Opts = { method?: "GET" | "POST" | "DELETE"; body?: unknown };

async function raw(path: string, opts: Opts, withCsrf: boolean): Promise<Response> {
  const headers: Record<string, string> = {};
  if (opts.body !== undefined) headers["content-type"] = "application/json";
  if (withCsrf && csrfToken) headers["x-csrf-token"] = csrfToken;
  return fetch(`/api${path}`, { method: opts.method ?? "GET", headers, credentials: "same-origin", body: opts.body === undefined ? undefined : JSON.stringify(opts.body) });
}

async function toProblem(res: Response): Promise<ApiProblem> {
  const j = (await res.json().catch(() => ({}))) as { code?: string; detail?: string };
  return new ApiProblem(res.status, j.code ?? "unknown", j.detail ?? "Something went wrong.");
}

export async function api<T = unknown>(path: string, opts: Opts = {}): Promise<T> {
  const mutating = (opts.method ?? "GET") !== "GET";
  const isPreAuth = /^\/v1\/auth\/(login|register|verify-email|resend-verification|forgot-password|reset-password)$/.test(path);
  if (mutating && !isPreAuth && !csrfToken) {
    const r = await raw("/v1/auth/csrf", {}, false);
    if (r.ok) setCsrfToken(((await r.json()) as { csrfToken: string }).csrfToken);
  }
  let res = await raw(path, opts, mutating && !isPreAuth);
  if (res.status === 403 && mutating && !isPreAuth) {
    const problem = await toProblem(res.clone());
    if (problem.code === "csrf_token") {
      const r = await raw("/v1/auth/csrf", {}, false);
      if (r.ok) { setCsrfToken(((await r.json()) as { csrfToken: string }).csrfToken); res = await raw(path, opts, true); }
    }
  }
  if (!res.ok) throw await toProblem(res);
  return res.status === 204 ? (undefined as T) : ((await res.json()) as T);
}

export type Me = {
  user: { id: string; email: string; fullName: string; emailVerified: boolean; mfaEnabled: boolean };
  memberships: { orgId: string; name: string; slug: string; role: string }[];
};
export type SessionInfo = { id: string; createdAt: string; lastSeenAt: string; userAgent: string | null; ip: string | null; current: boolean };

export const BUSINESS_TYPES = [
  ["retail", "Retail"], ["agriculture", "Agriculture"], ["restaurant", "Restaurant"], ["distribution", "Distribution"],
  ["services", "Services"], ["agency", "Agency"], ["other", "Other"],
] as const;

/** Friendly copy for API error codes. Never echoes server internals. */
export function problemMessage(e: unknown): string {
  if (!(e instanceof ApiProblem)) return "Could not reach the server. Check your connection and try again.";
  switch (e.code) {
    case "invalid_credentials": return "Incorrect email or password.";
    case "invalid_mfa_code": return "That verification code is not correct.";
    case "rate_limited": return "Too many attempts. Please wait a few minutes and try again.";
    case "invalid_token": return "This link is invalid or has expired. Request a new one.";
    case "weak_password": return "Choose a less predictable password.";
    case "unavailable": return "The service is temporarily unavailable. Please try again shortly.";
    default: return e.status >= 500 ? "Something went wrong on our side. Please try again." : e.message;
  }
}
