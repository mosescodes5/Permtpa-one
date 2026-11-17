import { describe, expect, it } from "vitest";
import { formatQty, parseQty, qtyToInput } from "@/lib/quantity";

describe("parseQty", () => {
  it("scales whole numbers to thousandths", () => { expect(parseQty("1", false)).toBe(1000); expect(parseQty(" 25 ", false)).toBe(25000); expect(parseQty("0", false)).toBe(0); });
  it("rejects decimals for whole-unit products", () => { for (const s of ["2.5", "1.", ".5", "0.001"]) expect(parseQty(s, false), s).toBeNull(); });
  it("accepts up to three decimals for fractional products, exactly", () => {
    expect(parseQty("2.5", true)).toBe(2500); expect(parseQty("0.001", true)).toBe(1); expect(parseQty("0.1", true)).toBe(100); expect(parseQty("7", true)).toBe(7000); expect(parseQty("1.125", true)).toBe(1125);
  });
  it("rejects a 4th decimal, signs, exponents and junk", () => { for (const s of ["1.2345", "-1", "+1", "1e3", "1,5", "abc", "", "  ", "1.2.3"]) expect(parseQty(s, true), s).toBeNull(); });
  it("enforces the cap", () => { expect(parseQty("100000000", true)).toBe(100_000_000_000); expect(parseQty("100000000.001", true)).toBeNull(); expect(parseQty("1000000000", true)).toBeNull(); });
  it("has no floating-point drift (0.1+0.2 style inputs)", () => { expect(parseQty("0.3", true)).toBe(300); expect(parseQty("4.35", true)).toBe(4350); expect(parseQty("1.005", true)).toBe(1005); });
});

describe("formatQty / qtyToInput", () => {
  it("round-trips", () => { for (const n of [0, 1, 999, 1000, 2500, 1234567, 100_000_000_000]) expect(parseQty(qtyToInput(n), true)).toBe(n); });
  it("trims trailing zeros", () => { expect(qtyToInput(2500)).toBe("2.5"); expect(qtyToInput(10000)).toBe("10"); expect(qtyToInput(1)).toBe("0.001"); });
  it("groups digits and keeps the sign", () => { expect(formatQty(1234500)).toBe("1,234.5"); expect(formatQty(-2500)).toBe("-2.5"); expect(formatQty(7000)).toBe("7"); });
});
