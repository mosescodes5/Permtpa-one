import { createHmac, randomUUID } from "node:crypto";
import { expect, type APIRequestContext, type Locator, type Page } from "@playwright/test";

export const API = "http://127.0.0.1:8081";
export const WEB_ORIGIN = "http://localhost:3100";
export const PASSWORD = "correct horse battery staple";

export const uniqueEmail = (p: string) => `${p}-${randomUUID().slice(0, 10)}@example.test`;
/** All e2e traffic comes from localhost, so rate limits are shared; clear them (development-only endpoint) before each test. */
export async function resetRateLimits(request: APIRequestContext) {
  const r = await request.post(`${API}/v1/dev/reset-rate-limits`, { headers: { origin: WEB_ORIGIN } });
  expect(r.status()).toBe(204);
}

/** Latest emailed token for an address, read from the API's development-only outbox. */
export async function emailedToken(request: APIRequestContext, to: string): Promise<string> {
  for (let i = 0; i < 20; i++) {
    const r = await request.get(`${API}/v1/dev/outbox`);
    const emails = ((await r.json()) as { emails: { to: string; text: string }[] }).emails.filter((e) => e.to === to);
    const last = emails.at(-1);
    const m = last?.text.match(/token=([A-Za-z0-9_-]+)/);
    if (m) return m[1]!;
    await new Promise((res) => setTimeout(res, 150));
  }
  throw new Error(`no email token for ${to}`);
}

/** Create + verify an account through the public API (fast setup for tests that are not about sign-up). */
export async function createVerifiedUser(request: APIRequestContext, email: string, fullName = "Test Person") {
  const h = { origin: WEB_ORIGIN };
  const reg = await request.post("/api/v1/auth/register", { data: { fullName, email, password: PASSWORD }, headers: h });
  expect(reg.status()).toBe(202);
  const v = await request.post("/api/v1/auth/verify-email", { data: { token: await emailedToken(request, email) }, headers: h });
  expect(v.status()).toBe(200);
}

export async function loginViaUi(page: Page, email: string, password = PASSWORD) {
  await page.goto("/login");
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password", { exact: true }).fill(password);
  await page.getByRole("button", { name: /log in/i }).click();
}

export async function createWorkspace(page: Page, name: string) {
  await page.waitForURL(/\/onboarding$/);
  await page.getByLabel("Business name").fill(name);
  await page.getByLabel(/what kind of business/i).selectOption("retail");
  await page.getByRole("button", { name: /create workspace/i }).click();
  await page.waitForURL(/\/dashboard$/);
}

function hotp(secret: Buffer, counter: number): string {
  const msg = Buffer.alloc(8); msg.writeBigUInt64BE(BigInt(counter));
  const h = createHmac("sha1", secret).update(msg).digest();
  const off = h[h.length - 1]! & 0xf;
  return String((h.readUInt32BE(off) & 0x7fffffff) % 1_000_000).padStart(6, "0");
}
function base32Decode(s: string): Buffer {
  const A = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"; let bits = 0, val = 0; const out: number[] = [];
  for (const c of s) { val = (val << 5) | A.indexOf(c); bits += 5; if (bits >= 8) { out.push((val >>> (bits - 8)) & 0xff); bits -= 8; } }
  return Buffer.from(out);
}
export const totp = (secretB32: string, stepOffset = 0) => hotp(base32Decode(secretB32), Math.floor(Date.now() / 30_000) + stepOffset);

/** Visible alerts only (Next also renders an empty route-announcer with role=alert). */
export const alertOf = (scope: Page | Locator) => scope.getByRole("alert").filter({ hasText: /\S/ });

/** Owner (already in a workspace) invites `hireEmail` through the UI; the hire opens the emailed link, logs in and accepts. */
export async function inviteAndJoin(owner: Page, hire: Page, request: APIRequestContext, hireEmail: string, roleValue: string) {
  await owner.goto("/team");
  await owner.getByLabel("Email address").fill(hireEmail);
  await owner.getByLabel("Role", { exact: true }).selectOption(roleValue);
  await owner.getByRole("button", { name: "Send invitation" }).click();
  await expect(owner.getByText(`Invitation sent to ${hireEmail}`)).toBeVisible();
  const token = await emailedToken(request, hireEmail);
  await hire.goto(`/accept-invite?token=${token}`);
  await expect(hire).toHaveURL(/\/login\?next=/);
  await hire.getByLabel("Email").fill(hireEmail);
  await hire.getByLabel("Password", { exact: true }).fill(PASSWORD);
  await hire.getByRole("button", { name: /log in/i }).click();
  await hire.getByRole("button", { name: "Accept invitation" }).click();
  await hire.waitForURL(/\/dashboard$/);
}
