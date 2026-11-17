import { expect, test } from "@playwright/test";
import { alertOf, createVerifiedUser, createWorkspace, inviteAndJoin, loginViaUi, resetRateLimits, uniqueEmail } from "./helpers";

test.beforeEach(({ request }) => resetRateLimits(request));

test("inventory workflow: add → low-stock → adjust (with reason) → history → edit price → search → archive", async ({ page, request, isMobile }) => {
  test.skip(isMobile, "desktop workflow");
  const email = uniqueEmail("inv");
  await createVerifiedUser(request, email, "Inventory Owner");
  await loginViaUi(page, email);
  await createWorkspace(page, "Rice Traders");
  await page.goto("/inventory");
  await expect(page.getByRole("heading", { name: "No products yet" })).toBeVisible();

  // add a product with one variant, 10 in stock, reorder level 12 (so it is immediately "low")
  await page.getByRole("button", { name: "Add product" }).first().click();
  const add = page.getByRole("dialog", { name: "Add product" });
  await add.getByRole("button", { name: "Save product" }).click();
  await expect(add.getByText("Enter a product name.")).toBeVisible();     // client-side validation first
  await add.getByLabel("Product name").fill("Premium Rice");
  await add.getByLabel("Unit").fill("bag");
  await add.getByLabel("SKU").fill("RIC-50");
  await add.getByLabel("Selling price (₦)").fill("1,600.50");
  await add.getByLabel("Cost price (₦)").fill("1200");
  await add.getByLabel("Reorder at").fill("12");
  await add.getByLabel("In stock").fill("10");
  await add.getByRole("button", { name: "Save product" }).click();
  await expect(page.getByText("Premium Rice was added.")).toBeVisible();

  const row = page.getByTestId("variant-row").filter({ hasText: "Premium Rice" });
  await expect(row).toContainText("₦1,600.50");
  await expect(row).toContainText("Low · 10");
  await expect(page.getByTestId("kpi-low")).toHaveText("1");
  await expect(page.getByTestId("kpi-value")).toContainText("₦12");   // 10 × ₦1,200 = ₦12,000 at cost

  // a damage write-off needs a reason, and cannot take stock below zero
  await page.getByRole("button", { name: "Adjust stock for Premium Rice RIC-50" }).click();
  const adj = page.getByRole("dialog", { name: "Adjust stock" });
  await adj.getByLabel("Reason").selectOption("damage");
  await adj.getByLabel("Quantity").fill("3");
  await adj.getByRole("button", { name: "Update stock" }).click();
  await expect(alertOf(adj)).toContainText(/explain why/i);
  await adj.getByLabel("Note").fill("Leaking bags");
  await adj.getByRole("button", { name: "Update stock" }).click();
  await expect(row).toContainText("Low · 7");

  await page.getByRole("button", { name: "Adjust stock for Premium Rice RIC-50" }).click();
  const adj2 = page.getByRole("dialog", { name: "Adjust stock" });
  await adj2.getByLabel("Reason").selectOption("damage");
  await adj2.getByLabel("Quantity").fill("50");
  await adj2.getByLabel("Note").fill("Too many");
  await adj2.getByRole("button", { name: "Update stock" }).click();
  await expect(alertOf(adj2)).toContainText(/below zero/i);
  await adj2.getByRole("button", { name: "Close" }).click();
  await expect(row).toContainText("Low · 7");   // unchanged

  // history is append-only and explains the balance
  await page.getByRole("button", { name: "History for Premium Rice RIC-50" }).click();
  const hist = page.getByRole("dialog", { name: /History/ });
  const moves = hist.getByTestId("movement-row");
  await expect(moves).toHaveCount(2);
  await expect(moves.first()).toContainText("Damaged / lost");
  await expect(moves.first()).toContainText("-3");
  await expect(moves.first()).toContainText("Leaking bags");
  await expect(moves.last()).toContainText("Opening stock");
  await hist.getByRole("button", { name: "Close" }).click();

  // edit price (exact money handling)
  await page.getByRole("button", { name: "Edit Premium Rice RIC-50" }).click();
  const edit = page.getByRole("dialog", { name: "Edit prices" });
  await edit.getByLabel("Selling price (₦)").fill("1700");
  await edit.getByRole("button", { name: "Save changes" }).click();
  await expect(row).toContainText("₦1,700.00");

  // search and filters
  await page.getByLabel("Search products").fill("RIC-50");
  await expect(page.getByTestId("variant-row")).toHaveCount(1);
  await page.getByLabel("Search products").fill("zzz-nothing");
  await expect(page.getByRole("heading", { name: "Nothing matches your search" })).toBeVisible();
  await page.getByLabel("Search products").fill("");
  await expect(row).toBeVisible();

  // the dashboard's low-stock card is live
  await page.goto("/dashboard");
  await expect(page.getByText("Live from your inventory")).toBeVisible();
  await expect(page.getByText("Premium Rice").first()).toBeVisible();
  await expect(page.getByText("7 left")).toBeVisible();

  // archive and restore
  await page.goto("/inventory");
  await page.getByRole("button", { name: "Archive Premium Rice" }).click();
  await expect(page.getByText("Premium Rice was archived.")).toBeVisible();
  await expect(page.getByRole("heading", { name: "No products yet" })).toBeVisible();
  await page.getByRole("button", { name: "archived" }).click();
  await expect(page.getByTestId("variant-row")).toHaveCount(1);
  await page.getByRole("button", { name: "Restore Premium Rice" }).click();
  await expect(page.getByText("Premium Rice was restored.")).toBeVisible();
});

test("sales staff see the catalogue but never costs, valuation or edit controls", async ({ browser, request, isMobile }) => {
  test.skip(isMobile, "two-user desktop flow");
  const ownerEmail = uniqueEmail("invboss"), staffEmail = uniqueEmail("cashier");
  await createVerifiedUser(request, ownerEmail, "Boss Person");
  await createVerifiedUser(request, staffEmail, "Cashier Person");
  const ownerCtx = await browser.newContext(), staffCtx = await browser.newContext();
  const owner = await ownerCtx.newPage(), staff = await staffCtx.newPage();
  await loginViaUi(owner, ownerEmail);
  await createWorkspace(owner, "Secret Costs Ltd");
  await owner.goto("/inventory");
  await owner.getByRole("button", { name: "Add product" }).first().click();
  const add = owner.getByRole("dialog", { name: "Add product" });
  await add.getByLabel("Product name").fill("Sugar");
  await add.getByLabel("SKU").fill("SUG-1");
  await add.getByLabel("Selling price (₦)").fill("500");
  await add.getByLabel("Cost price (₦)").fill("350");
  await add.getByLabel("In stock").fill("20");
  await add.getByRole("button", { name: "Save product" }).click();
  await expect(owner.getByText("Sugar was added.")).toBeVisible();
  await expect(owner.getByRole("columnheader", { name: "Cost" })).toBeVisible();
  await expect(owner.getByTestId("kpi-value")).toBeVisible();

  await inviteAndJoin(owner, staff, request, staffEmail, "sales_staff");
  await staff.goto("/inventory");
  const row = staff.getByTestId("variant-row").filter({ hasText: "Sugar" });
  await expect(row).toContainText("₦500.00");
  await expect(row).toContainText("20");
  await expect(staff.getByRole("columnheader", { name: "Cost" })).toHaveCount(0);
  await expect(staff.getByTestId("kpi-value")).toHaveCount(0);
  await expect(staff.getByText("₦350")).toHaveCount(0);
  await expect(staff.getByRole("button", { name: /Add product|Categories|Adjust stock|Edit |Archive /i })).toHaveCount(0);
  await expect(staff.getByText(/Only owners, administrators, managers and inventory managers can change them/i)).toBeVisible();
  await staff.getByRole("button", { name: "History for Sugar SUG-1" }).click();
  await expect(staff.getByTestId("movement-row")).toHaveCount(1);   // read-only history is allowed
  await ownerCtx.close(); await staffCtx.close();
});

test("inventory pages fit the screen (no horizontal page scroll) on every device", async ({ page, request }) => {
  const email = uniqueEmail("fit");
  await createVerifiedUser(request, email, "Fit Person");
  await loginViaUi(page, email);
  await createWorkspace(page, "Fit Co");
  // create a product through the API from the logged-in browser context (CSRF token + Origin as the UI would send)
  const csrf = ((await (await page.request.get("/api/v1/auth/csrf")).json()) as { csrfToken: string }).csrfToken;
  const orgId = ((await (await page.request.get("/api/v1/me")).json()) as { memberships: { orgId: string }[] }).memberships[0]!.orgId;
  const r = await page.request.post(`/api/v1/orgs/${orgId}/products`, { headers: { "x-csrf-token": csrf, origin: "http://localhost:3100" }, data: { name: "A fairly long product name to stress the table layout on narrow screens", variants: [{ sku: "FIT-1", priceMinor: 123456789, costMinor: 100, openingStock: 5000, reorderLevel: 10000 }] } });
  expect(r.status()).toBe(201);
  // Wait for REAL content before measuring (a loading skeleton is narrow and would hide an overflow bug).
  const ready = { "/inventory": page.getByTestId("variant-row").first(), "/team": page.getByTestId("member-row").first(), "/dashboard": page.getByRole("heading", { name: "Top products" }) } as const;
  for (const path of ["/inventory", "/team", "/dashboard"] as const) {
    await page.goto(path);
    await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
    await expect(ready[path]).toBeVisible();
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth);
    expect(overflow, `${path} overflows horizontally`).toBe(false);
  }
});
