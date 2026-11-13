import { expect, test } from "@playwright/test";
import { randomIp } from "./helpers";

test.use({ extraHTTPHeaders: { "x-forwarded-for": randomIp() } });

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

test("login form validates on the client", async ({ page }) => {
  await page.goto("/login");
  await page.getByRole("button", { name: "Log in" }).click();
  await expect(page.getByText("Enter your email address")).toBeVisible();
});

test("no horizontal overflow on public pages", async ({ page }) => {
  for (const path of ["/", "/login", "/register", "/forgot-password"]) {
    await page.goto(path);
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth);
    expect(overflow, `${path} overflows horizontally`).toBe(false);
  }
});

test("security headers are set on the web app", async ({ request }) => {
  const res = await request.get("/");
  expect(res.headers()["x-content-type-options"]).toBe("nosniff");
  expect(res.headers()["x-frame-options"]).toBe("DENY");
});
