import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiProblem, api, problemMessage, setCsrfToken } from "@/lib/api";

const res = (status: number, body?: unknown) => Promise.resolve(new Response(status === 204 ? null : JSON.stringify(body ?? {}), { status, headers: { "content-type": "application/json" } }));
let f: ReturnType<typeof vi.fn>;
beforeEach(() => { f = vi.fn(); vi.stubGlobal("fetch", f); setCsrfToken(null); });
afterEach(() => vi.unstubAllGlobals());
const headersOf = (i: number) => (f.mock.calls[i]![1].headers ?? {}) as Record<string, string>;

describe("api client", () => {
  it("always uses same-origin credentials and the /api proxy", async () => {
    f.mockReturnValueOnce(res(200, { ok: 1 }));
    await api("/v1/me");
    expect(f.mock.calls[0]![0]).toBe("/api/v1/me");
    expect(f.mock.calls[0]![1].credentials).toBe("same-origin");
  });

  it("fetches a CSRF token before the first authenticated mutation and attaches it", async () => {
    f.mockReturnValueOnce(res(200, { csrfToken: "T1" })).mockReturnValueOnce(res(201, { id: "x" }));
    await api("/v1/organizations", { method: "POST", body: { name: "A" } });
    expect(f.mock.calls[0]![0]).toBe("/api/v1/auth/csrf");
    expect(headersOf(1)["x-csrf-token"]).toBe("T1");
  });

  it("does not require a CSRF token for pre-auth endpoints", async () => {
    f.mockReturnValueOnce(res(202, {}));
    await api("/v1/auth/forgot-password", { method: "POST", body: { email: "a@b.co" } });
    expect(f).toHaveBeenCalledTimes(1);
    expect(headersOf(0)["x-csrf-token"]).toBeUndefined();
  });

  it("refreshes the token once and retries when the server says it is stale", async () => {
    setCsrfToken("OLD");
    f.mockReturnValueOnce(res(403, { code: "csrf_token" })).mockReturnValueOnce(res(200, { csrfToken: "NEW" })).mockReturnValueOnce(res(204));
    await api("/v1/auth/logout", { method: "POST", body: {} });
    expect(headersOf(2)["x-csrf-token"]).toBe("NEW");
    expect(f).toHaveBeenCalledTimes(3);
  });

  it("raises typed problems and maps them to safe copy", async () => {
    f.mockReturnValueOnce(res(429, { code: "rate_limited", detail: "internal detail" }));
    const e = (await api("/v1/auth/login", { method: "POST", body: {} }).catch((x: unknown) => x)) as ApiProblem;
    expect(e).toBeInstanceOf(ApiProblem);
    expect(e.status).toBe(429);
    expect(problemMessage(e)).toMatch(/too many attempts/i);
    expect(problemMessage(new ApiProblem(500, "internal", "stack trace"))).not.toContain("stack trace");
    expect(problemMessage(new TypeError("fetch failed"))).toMatch(/could not reach the server/i);
  });
});

describe("api client: PATCH", () => {
  it("sends PATCH with the session-bound CSRF token", async () => {
    setCsrfToken("T");
    f.mockReturnValueOnce(res(200, { ok: 1 }));
    await api("/v1/orgs/x/members/y", { method: "PATCH", body: { role: "manager" } });
    expect(f.mock.calls[0]![1].method).toBe("PATCH");
    expect(headersOf(0)["x-csrf-token"]).toBe("T");
  });
});
