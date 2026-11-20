import { expect, test } from "@playwright/test";
import { createVerifiedUser, createWorkspace, inviteAndJoin, loginViaUi, resetRateLimits, uniqueEmail } from "./helpers";

test.beforeEach(({ request }) => resetRateLimits(request));

test("onboarding: choose a business type, load samples, see them flagged, delete them", async ({ page, request, isMobile }) => {
  test.skip(isMobile, "desktop workflow");
  const email = uniqueEmail("onb");
  await createVerifiedUser(request, email, "Onboarding Owner");
  await loginViaUi(page, email);

  // the 12 business-type cards, each with its description
  await page.waitForURL(/\/onboarding$/);
  await expect(page.getByRole("radio")).toHaveCount(12);
  await expect(page.getByText("Phones, gadgets, accessories")).toBeVisible();
  await expect(page.getByText(/sold by bag, weight or roll/)).toBeVisible();

  // validation comes before any request
  await page.getByRole("button", { name: /finish setup/i }).click();
  await expect(page.getByText("Enter your business name")).toBeVisible();
  await expect(page.getByText("Choose a business type")).toBeVisible();

  await createWorkspace(page, "Depot Builders", { type: "Building materials", phone: "+234 803 123 4567", samples: true });

  // the business profile kept the phone number
  await page.goto("/organization");
  await expect(page.getByLabel(/phone number/i)).toHaveValue("+234 803 123 4567");

  // six samples, flagged, with exact fractional stock for the by-weight items
  await page.goto("/inventory");
  await expect(page.getByText(/6 sample products are in your catalogue/)).toBeVisible();
  await expect(page.getByTestId("variant-row")).toHaveCount(6);
  await expect(page.getByTestId("variant-row").filter({ hasText: "Cement 50kg bag" })).toContainText("Sample");
  await expect(page.getByTestId("variant-row").filter({ hasText: "Sharp sand (per tonne)" })).toContainText("12.5");

  // delete needs an explicit second click; cancelling changes nothing
  await page.getByRole("button", { name: "Delete sample products" }).click();
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByTestId("variant-row")).toHaveCount(6);
  await page.getByRole("button", { name: "Delete sample products" }).click();
  await page.getByRole("button", { name: "Yes, delete samples" }).click();
  await expect(page.getByText("6 sample products were deleted.")).toBeVisible();
  await expect(page.getByRole("heading", { name: "No products yet" })).toBeVisible();

  // and they can be loaded again from the empty state
  await page.getByRole("button", { name: "Load sample catalogue" }).click();
  await expect(page.getByText("6 sample products were added.")).toBeVisible();
  await expect(page.getByTestId("variant-row")).toHaveCount(6);
});

test("sales staff see sample products but no costs and no sample controls", async ({ browser, request, isMobile }) => {
  test.skip(isMobile, "desktop workflow");
  const ownerEmail = uniqueEmail("boss"), staffEmail = uniqueEmail("clerk");
  await createVerifiedUser(request, ownerEmail, "Boss Person");
  await createVerifiedUser(request, staffEmail, "Clerk Person");
  const ownerCtx = await browser.newContext(), staffCtx = await browser.newContext();
  const owner = await ownerCtx.newPage(), staff = await staffCtx.newPage();
  await loginViaUi(owner, ownerEmail);
  await createWorkspace(owner, "Phone Hub", { type: "Electronics", samples: true });
  await owner.goto("/inventory");
  await expect(owner.getByRole("columnheader", { name: "Cost" })).toBeVisible();

  await inviteAndJoin(owner, staff, request, staffEmail, "sales_staff");
  await staff.goto("/inventory");
  await expect(staff.getByTestId("variant-row")).toHaveCount(5);
  await expect(staff.getByRole("columnheader", { name: "Cost" })).toHaveCount(0);
  await expect(staff.getByRole("button", { name: /Delete sample products|Load sample catalogue/ })).toHaveCount(0);
  await ownerCtx.close();
  await staffCtx.close();
});
