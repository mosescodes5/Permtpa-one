import { expect, test } from "@playwright/test";
import { PASSWORD, alertOf, createVerifiedUser, createWorkspace, emailedToken, loginViaUi, randomIp, totp, uniqueEmail } from "./helpers";

test.beforeEach(({ context }) => context.setExtraHTTPHeaders({ "x-forwarded-for": randomIp() }));

test("full journey: register → verify → login → onboarding → dashboard → logout", async ({ page, request, isMobile }) => {
  const email = uniqueEmail("journey");
  await page.goto("/register");
  await page.getByLabel("Full name").fill("Adaeze Okafor");
  await page.getByLabel("Work email").fill(email);
  await page.getByLabel("Password", { exact: true }).fill(PASSWORD);
  await page.getByRole("checkbox").check();
  await page.getByRole("button", { name: /create account/i }).click();
  await expect(page.getByText("Check your email")).toBeVisible();

  // Unverified accounts cannot log in, and the UI offers a resend.
  await loginViaUi(page, email);
  await expect(page.getByText(/verify your email address/i)).toBeVisible();

  const token = await emailedToken(request, email);
  await page.goto(`/verify-email?token=${token}`);
  await expect(page.getByRole("heading", { name: "Email verified" })).toBeVisible();
  await page.goto(`/verify-email?token=${token}`);
  await expect(page.getByText(/couldn.t verify that link/i)).toBeVisible(); // single-use

  await loginViaUi(page, email);
  await createWorkspace(page, "Okafor Trading");
  await expect(page.getByRole("heading", { level: 1 })).toContainText("Good morning, Adaeze");
  await expect(page.getByTestId("active-org")).toHaveText("Okafor Trading");
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth);
  expect(overflow).toBe(false);

  // The session cookie is HttpOnly: page JavaScript must not be able to read it.
  expect(await page.evaluate(() => document.cookie)).not.toContain("permtpa_session");

  if (!isMobile) {
    await page.keyboard.press("Control+k");
    await page.getByLabel("Search commands").fill("settings");
    await page.keyboard.press("Enter");
    await expect(page).toHaveURL(/\/settings$/);
  }
  await page.goto("/dashboard");
  await page.getByRole("button", { name: "Account menu" }).click();
  await page.getByRole("menuitem", { name: "Log out" }).click();
  await expect(page).toHaveURL(/\/login$/);

  await page.goto("/dashboard");
  await expect(page).toHaveURL(/\/login\?next=%2Fdashboard/);
});

test("wrong password gives one generic message and stays on the page", async ({ page, request }) => {
  const email = uniqueEmail("wrongpw");
  await createVerifiedUser(request, email);
  await loginViaUi(page, email, "not-the-right-password");
  await expect(alertOf(page)).toHaveText(/Incorrect email or password\./);
  await expect(page).toHaveURL(/\/login/);
  await loginViaUi(page, uniqueEmail("ghost"), "not-the-right-password");
  await expect(alertOf(page)).toHaveText(/Incorrect email or password\./); // identical for unknown accounts
});

test("protected pages redirect anonymous visitors", async ({ page }) => {
  for (const p of ["/dashboard", "/settings", "/inventory"]) {
    await page.goto(p);
    await expect(page).toHaveURL(new RegExp(`/login\\?next=${encodeURIComponent(p).replace(/%/g, "%")}`));
  }
});

test("password reset: link works once, then the new password logs in", async ({ page, request }) => {
  const email = uniqueEmail("reset");
  await createVerifiedUser(request, email);
  await page.goto("/forgot-password");
  await page.getByLabel("Email").fill(email);
  await page.getByRole("button", { name: /send reset link/i }).click();
  await expect(page.getByText(/we have sent a password reset link/i)).toBeVisible();

  const token = await emailedToken(request, email);
  await page.goto(`/reset-password?token=${token}`);
  await page.getByLabel("New password", { exact: true }).fill("a brand new passphrase 42");
  await page.getByLabel("Confirm new password").fill("a brand new passphrase 42");
  await page.getByRole("button", { name: /change password/i }).click();
  await expect(page.getByText(/password has been changed/i)).toBeVisible();

  await page.goto(`/reset-password?token=${token}`);
  await page.getByLabel("New password", { exact: true }).fill("yet another passphrase 99");
  await page.getByLabel("Confirm new password").fill("yet another passphrase 99");
  await page.getByRole("button", { name: /change password/i }).click();
  await expect(alertOf(page)).toContainText(/invalid or has expired/i);

  await loginViaUi(page, email, "a brand new passphrase 42");
  await page.waitForURL(/\/onboarding$/);
});

test("two-factor: enrol in settings, then log in with a code", async ({ page, request, isMobile }) => {
  test.skip(isMobile, "covered on desktop; identical flow");
  const email = uniqueEmail("mfa");
  await createVerifiedUser(request, email, "Mfa Person");
  await loginViaUi(page, email);
  await createWorkspace(page, "Mfa Trading");

  await page.goto("/settings");
  await expect(page.getByText("This device")).toBeVisible();
  await page.getByRole("button", { name: /set up two-factor/i }).click();
  const secret = (await page.getByTestId("mfa-secret").innerText()).trim();
  await expect(page.getByAltText(/QR code/i)).toBeVisible();
  await page.getByLabel("6-digit code").fill(totp(secret));
  await page.getByRole("button", { name: "Turn on" }).click();
  await expect(page.getByTestId("recovery-codes")).toBeVisible();
  const codes = (await page.getByTestId("recovery-codes").locator("li").allInnerTexts()).map((c) => c.trim());
  expect(codes).toHaveLength(10);
  await page.getByRole("button", { name: /i have saved them/i }).click();
  await expect(page.getByText("Enabled")).toBeVisible();

  await page.getByRole("button", { name: "Account menu" }).click();
  await page.getByRole("menuitem", { name: "Log out" }).click();
  await expect(page).toHaveURL(/\/login$/);

  await loginViaUi(page, email);
  await page.getByLabel("Verification code").fill(totp(secret, 1)); // next step: the enrolment step cannot be replayed
  await page.getByRole("button", { name: /verify and log in/i }).click();
  await page.waitForURL(/\/dashboard$/);
  await expect(page.getByTestId("active-org")).toHaveText("Mfa Trading");
});
