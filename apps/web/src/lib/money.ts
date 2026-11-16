/**
 * Money is ALWAYS an integer count of minor units (kobo for NGN). Never floats.
 * Display formatting is the only place a decimal appears.
 */
export type Currency = "NGN";
const MINOR_PER_MAJOR: Record<Currency, bigint> = { NGN: 100n };
const SYMBOL: Record<Currency, string> = { NGN: "₦" };

export function formatMoney(minor: number | bigint, currency: Currency = "NGN", opts: { compact?: boolean } = {}): string {
  const value = typeof minor === "bigint" ? minor : BigInt(minor);
  if (typeof minor === "number" && !Number.isInteger(minor)) {
    throw new RangeError("Money must be an integer number of minor units");
  }
  const negative = value < 0n;
  const abs = negative ? -value : value;
  const per = MINOR_PER_MAJOR[currency];
  const major = abs / per;
  const frac = abs % per;
  let body: string;
  if (opts.compact && major >= 1000n) {
    const n = Number(major);
    const units: [number, string][] = [[1e9, "B"], [1e6, "M"], [1e3, "K"]];
    const [div, suffix] = units.find(([d]) => n >= d)!;
    body = `${(Math.floor((n / div) * 10) / 10).toString()}${suffix}`;
  } else {
    body = major.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
    body += `.${frac.toString().padStart(2, "0")}`;
  }
  return `${negative ? "-" : ""}${SYMBOL[currency]}${body}`;
}

/** Whole-naira helper for price lists (₦3,000). */
export function naira(wholeNaira: number): bigint {
  if (!Number.isInteger(wholeNaira)) throw new RangeError("Whole naira required");
  return BigInt(wholeNaira) * 100n;
}

export function formatMoneyWhole(minor: bigint): string {
  return formatMoney(minor).replace(/\.00$/, "");
}

/** Parse a user-typed naira amount ("1,600.50") into integer kobo WITHOUT floating-point. Returns null if invalid. */
export function parseNairaToMinor(input: string): number | null {
  const t = input.replace(/[,\s₦]/g, "");
  if (!/^\d+(\.\d{1,2})?$/.test(t)) return null;
  const [whole = "0", frac = ""] = t.split(".");
  const minor = Number(whole) * 100 + Number(frac.padEnd(2, "0"));
  return Number.isSafeInteger(minor) && minor <= 1_000_000_000_000 ? minor : null;
}

/** Integer kobo -> "1600.50" for editing inputs. */
export function minorToInput(minor: number): string {
  const v = BigInt(minor);
  return `${v / 100n}.${(v % 100n).toString().padStart(2, "0")}`;
}
