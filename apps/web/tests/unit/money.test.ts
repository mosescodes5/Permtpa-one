import { describe, expect, it } from "vitest";
import { formatMoney, formatMoneyWhole, naira } from "@/lib/money";
import { PLANS } from "@/lib/plans";

describe("money (integer minor units)", () => {
  it("formats kobo as naira with two decimals", () => {
    expect(formatMoney(150_000_00)).toBe("₦150,000.00");
    expect(formatMoney(5)).toBe("₦0.05");
    expect(formatMoney(0)).toBe("₦0.00");
  });
  it("formats negatives", () => { expect(formatMoney(-1250)).toBe("-₦12.50"); });
  it("compacts large values without float drift on small ones", () => {
    expect(formatMoney(184_250_00, "NGN", { compact: true })).toBe("₦184.2K");
    expect(formatMoney(2_500_000_00, "NGN", { compact: true })).toBe("₦2.5M");
    expect(formatMoney(99_999, "NGN", { compact: true })).toBe("₦999.99");
  });
  it("rejects fractional minor units", () => { expect(() => formatMoney(10.5)).toThrow(RangeError); });
  it("converts whole naira to kobo exactly", () => {
    expect(naira(3000)).toBe(300_000n);
    expect(() => naira(1.5)).toThrow(RangeError);
  });
  it("sums without floating-point error (0.1 + 0.2 style)", () => { expect(formatMoney(10 + 20)).toBe("₦0.30"); });
  it("handles bigint", () => { expect(formatMoneyWhole(2_000_000n)).toBe("₦20,000"); });
});

describe("plans", () => {
  it("matches the proposed prices", () => {
    expect(PLANS.map((p) => formatMoneyWhole(p.monthly))).toEqual(["₦0", "₦3,000", "₦8,000", "₦20,000"]);
  });
  it("annual is ten months of the monthly price", () => {
    for (const p of PLANS) expect(p.annual).toBe(p.monthly * 10n);
  });
});
