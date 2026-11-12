import { expect, test } from "@playwright/test";

test("landing page communicates the product and brand", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle(/PERMTPA ONE/);
  await expect(page.getByRole("heading", { level: 1 })).toContainText("one place");
  await expect(page.getByText("technology product by PERMTPA").first()).toBeVisible();
  await expect(page.locator("body")).not.toContainText("BizOS");
});

test("pricing shows configured plans and toggles to annual", async ({ page }) => {
  await page.goto("/#pricing");
  for (const price of ["₦0", "₦3,000", "₦8,000", "₦20,000"]) await expect(page.getByText(price, { exact: true }).first()).toBeVisible();
  await page.getByRole("button", { name: /annual/i }).click();
  await expect(page.getByText("₦30,000", { exact: true })).toBeVisible();
});

test("login validates and is honest about missing backend", async ({ page }) => {
  await page.goto("/login");
  await page.getByRole("button", { name: "Log in" }).click();
  await expect(page.getByText("Enter your email address")).toBeVisible();
  await page.getByLabel("Email").fill("a@b.co");
  await page.getByLabel("Password", { exact: true }).fill("a-long-enough-pass");
  await page.getByRole("button", { name: "Log in" }).click();
  await expect(page.getByRole("status")).toContainText("not connected yet");
});

test("dashboard shell: sample-data notice, range filter, command palette, module placeholder", async ({ page, isMobile }) => {
  await page.goto("/dashboard");
  await expect(page.getByRole("heading", { level: 1 })).toContainText("Good morning");
  await expect(page.getByRole("note")).toContainText("sample data");
  await page.getByRole("button", { name: "7 days" }).click();
  await expect(page.getByRole("button", { name: "7 days" })).toHaveAttribute("aria-pressed", "true");
  if (isMobile) {
    await page.getByRole("button", { name: "Open navigation" }).click();
    await expect(page.getByRole("dialog", { name: "Navigation" })).toBeVisible();
    await page.getByRole("dialog", { name: "Navigation" }).getByRole("link", { name: "Inventory" }).click();
  } else {
    await page.keyboard.press("Control+k");
    await expect(page.getByRole("dialog", { name: "Command palette" })).toBeVisible();
    await page.getByLabel("Search commands").fill("inventory");
    await page.keyboard.press("Enter");
  }
  await expect(page).toHaveURL(/\/inventory$/);
  await expect(page.getByText("Planned for Milestone 3")).toBeVisible();
});

test("no horizontal overflow on key pages", async ({ page }) => {
  for (const path of ["/", "/login", "/dashboard"]) {
    await page.goto(path);
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth);
    expect(overflow, `${path} overflows horizontally`).toBe(false);
  }
});

test("security headers are set", async ({ request }) => {
  const res = await request.get("/");
  expect(res.headers()["x-content-type-options"]).toBe("nosniff");
  expect(res.headers()["x-frame-options"]).toBe("DENY");
});
