import { expect, test } from "@playwright/test";
import { PASSWORD, createVerifiedUser, createWorkspace, emailedToken, loginViaUi, resetRateLimits, uniqueEmail } from "./helpers";

test.beforeEach(({ request }) => resetRateLimits(request));

test("team lifecycle: invite → accept → change role → read-only view → suspend → remove", async ({ browser, request, isMobile }) => {
  test.skip(isMobile, "two-user desktop flow");
  const ownerEmail = uniqueEmail("boss"), hireEmail = uniqueEmail("hire");
  await createVerifiedUser(request, ownerEmail, "Boss Person");
  await createVerifiedUser(request, hireEmail, "Hire Person");

  const ownerCtx = await browser.newContext();
  const hireCtx = await browser.newContext();
  const owner = await ownerCtx.newPage();
  const hire = await hireCtx.newPage();

  await loginViaUi(owner, ownerEmail);
  await createWorkspace(owner, "Boss Trading");

  // owner invites the hire as an Accountant
  await owner.goto("/team");
  await expect(owner.getByTestId("member-row")).toHaveCount(1);
  await owner.getByLabel("Email address").fill(hireEmail);
  await owner.getByLabel("Role", { exact: true }).selectOption("accountant");
  await owner.getByRole("button", { name: "Send invitation" }).click();
  await expect(owner.getByText(`Invitation sent to ${hireEmail}`)).toBeVisible();
  await expect(owner.getByRole("list", { name: "Pending invitations" })).toContainText(hireEmail);

  // the hire opens the emailed link while logged out → login → the token survives the redirect → accept
  const token = await emailedToken(request, hireEmail);
  await hire.goto(`/accept-invite?token=${token}`);
  await expect(hire).toHaveURL(/\/login\?next=/);
  await hire.getByLabel("Email").fill(hireEmail);
  await hire.getByLabel("Password", { exact: true }).fill(PASSWORD);
  await hire.getByRole("button", { name: /log in/i }).click();
  await expect(hire.getByRole("heading", { name: "You have been invited" })).toBeVisible();
  await hire.getByRole("button", { name: "Accept invitation" }).click();
  await hire.waitForURL(/\/dashboard$/);
  await expect(hire.getByTestId("active-org")).toHaveText("Boss Trading");

  // the link is single-use
  await hire.goto(`/accept-invite?token=${token}`);
  await hire.getByRole("button", { name: "Accept invitation" }).click();
  await expect(hire.getByRole("alert").filter({ hasText: /\S/ })).toContainText(/invalid, has expired/i);

  // owner sees two members and changes the role
  await owner.goto("/team");
  await expect(owner.getByTestId("member-row")).toHaveCount(2);
  await owner.getByLabel("Role for Hire Person").selectOption("manager");
  await expect(owner.getByText("Hire Person is now Manager.")).toBeVisible();

  // a Manager gets a read-only team page and cannot see colleagues' emails
  await hire.goto("/team");
  await expect(hire.getByText(/only owners and administrators can invite/i)).toBeVisible();
  await expect(hire.getByLabel(/^Role for /)).toHaveCount(0);
  await expect(hire.getByText(ownerEmail)).toHaveCount(0);

  // suspension revokes access immediately; reinstating restores it
  const row = owner.getByTestId("member-row").filter({ hasText: "Hire Person" });
  await row.getByRole("button", { name: "Suspend" }).click();
  await expect(row.getByText("Suspended")).toBeVisible();
  await hire.goto("/dashboard");
  await hire.waitForURL(/\/onboarding$/);
  await row.getByRole("button", { name: "Reinstate" }).click();
  await expect(row.getByText("Suspended")).toHaveCount(0);
  await hire.goto("/dashboard");
  await expect(hire.getByTestId("active-org")).toHaveText("Boss Trading");

  // removal needs a confirmation step
  await owner.getByRole("button", { name: "Remove Hire Person" }).click();
  await owner.getByRole("button", { name: "Confirm remove" }).click();
  await expect(owner.getByText("Hire Person was removed.")).toBeVisible();
  await expect(owner.getByTestId("member-row")).toHaveCount(1);
  await ownerCtx.close(); await hireCtx.close();
});

test("an invitation link is useless to a different account", async ({ browser, request, isMobile }) => {
  test.skip(isMobile, "desktop flow");
  const ownerEmail = uniqueEmail("boss2"), hireEmail = uniqueEmail("hire2"), thiefEmail = uniqueEmail("thief");
  for (const [e, n] of [[ownerEmail, "Boss Two"], [hireEmail, "Hire Two"], [thiefEmail, "Thief Person"]] as const) await createVerifiedUser(request, e, n);
  const ctx = await browser.newContext();
  const owner = await ctx.newPage();
  await loginViaUi(owner, ownerEmail);
  await createWorkspace(owner, "Guarded Co");
  await owner.goto("/team");
  await owner.getByLabel("Email address").fill(hireEmail);
  await owner.getByRole("button", { name: "Send invitation" }).click();
  await expect(owner.getByText(`Invitation sent to ${hireEmail}`)).toBeVisible();
  const token = await emailedToken(request, hireEmail);

  const thiefCtx = await browser.newContext();
  const thief = await thiefCtx.newPage();
  await loginViaUi(thief, thiefEmail);
  await thief.waitForURL(/\/onboarding$/);
  await thief.goto(`/accept-invite?token=${token}`);
  await thief.getByRole("button", { name: "Accept invitation" }).click();
  await expect(thief.getByRole("alert").filter({ hasText: /\S/ })).toContainText(/different email address/i);
  await owner.reload();
  await expect(owner.getByTestId("member-row")).toHaveCount(1);
  await ctx.close(); await thiefCtx.close();
});

test("organization profile: owner can edit, change is audited-visible in the shell", async ({ page, request, isMobile }) => {
  test.skip(isMobile, "desktop flow");
  const email = uniqueEmail("org");
  await createVerifiedUser(request, email, "Org Owner");
  await loginViaUi(page, email);
  await createWorkspace(page, "Before Name");
  await page.goto("/organization");
  await page.getByLabel("Business name").fill("After Name Ltd");
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByText("Your changes were saved.")).toBeVisible();
  await expect(page.getByTestId("active-org")).toHaveText("After Name Ltd");
});
