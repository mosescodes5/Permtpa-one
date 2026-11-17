/** Typed client for the PERMTPA ONE API (same-origin via the Next proxy). Sessions live in an HttpOnly cookie; JS never sees it. */

export class ApiProblem extends Error {
  constructor(public status: number, public code: string, message: string) {
    super(message);
    this.name = "ApiProblem";
  }
}

let csrfToken: string | null = null;
export const setCsrfToken = (t: string | null) => { csrfToken = t; };

type Opts = { method?: "GET" | "POST" | "PATCH" | "DELETE"; body?: unknown };

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

export type Member = { userId: string; fullName: string; email: string | null; role: string; status: string };
export type Invitation = { id: string; email: string; role: string; createdAt: string; expiresAt: string };
export type OrgProfile = { id: string; name: string; slug: string; businessType: string; currency: string; yourRole: string };
export const ROLE_LABELS: Record<string, string> = { owner: "Owner", administrator: "Administrator", manager: "Manager", accountant: "Accountant", inventory_manager: "Inventory Manager", sales_staff: "Sales Staff", read_only: "Read Only" };
export const ROLE_HELP: Record<string, string> = { owner: "Full control, including billing and ownership", administrator: "Manages the team and settings", manager: "Runs day-to-day operations", accountant: "Finance and reports", inventory_manager: "Products and stock", sales_staff: "Records sales", read_only: "Can view, cannot change" };
export const ASSIGNABLE_ROLES = ["administrator", "manager", "accountant", "inventory_manager", "sales_staff", "read_only"] as const;

export type Variant = { id: string; name: string; sku: string; barcode: string | null; priceMinor: number; costMinor: number | null; reorderLevel: number; status: string; onHand: number };
export type Product = { id: string; name: string; description: string | null; unit: string; allowFractional: boolean; status: string; categoryId: string | null; category: string | null; variants: Variant[] };
export type Category = { id: string; name: string; productCount: number };
export type Movement = { id: number; delta: number; reason: string; note: string | null; resultingOnHand: number | null; actor: { id: string; name: string | null }; createdAt: string };
export type LowStockItem = { variantId: string; productId: string; productName: string; variantName: string; sku: string; onHand: number; reorderLevel: number };
export const REASON_LABELS: Record<string, string> = { opening: "Opening stock", purchase: "Purchase", return_in: "Customer return", adjustment: "Adjustment", damage: "Damaged / lost", correction: "Correction", sale: "Sale" };
export const canManageInventory = (role: string) => ["owner", "administrator", "manager", "inventory_manager"].includes(role);
export const canViewCosts = (role: string) => canManageInventory(role) || role === "accountant";

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
    case "forbidden": return "You do not have permission to do that.";
    case "last_owner": return "An organization must keep at least one owner.";
    case "already_member": return "That person is already on your team.";
    case "conflict": return "That conflicts with something that already exists.";
    case "invalid_role": return "Choose a valid role.";
    case "sku_taken": return "That SKU is already used by another item.";
    case "barcode_taken": return "That barcode is already used by another item.";
    case "name_taken": return "That name already exists.";
    case "insufficient_stock": return "There is not enough stock for that change.";
    case "note_required": return "Please explain why you are changing the stock.";
    case "category_in_use": return "This category still has products. Move or archive them first.";
    case "unavailable": return "The service is temporarily unavailable. Please try again shortly.";
    default: return e.status >= 500 ? "Something went wrong on our side. Please try again." : e.message;
  }
}
