/**
 * Stock quantities are integers in THOUSANDTHS of the product's unit (1 pcs = 1000, 2.5 kg = 2500).
 * The API sends and receives thousandths; this module is the only place that converts to and from text. No floats.
 */
export const QTY_SCALE = 1000;
export const MAX_QTY = 100_000_000_000;

/** Parses typed text into thousandths. Whole products accept digits only; fractional products accept up to 3 decimals. */
export function parseQty(input: string, fractional: boolean): number | null {
  const m = (fractional ? /^(\d{1,9})(?:\.(\d{1,3}))?$/ : /^(\d{1,9})$/).exec(input.trim());
  if (!m) return null;
  const n = Number(m[1]) * QTY_SCALE + Number((m[2] ?? "").padEnd(3, "0"));
  return n <= MAX_QTY ? n : null;
}

/** Plain text for an input box: 2500 -> "2.5", 10000 -> "10". */
export function qtyToInput(thousandths: number): string {
  const neg = thousandths < 0; const abs = Math.abs(thousandths);
  const frac = String(abs % QTY_SCALE).padStart(3, "0").replace(/0+$/, "");
  return `${neg ? "-" : ""}${Math.floor(abs / QTY_SCALE)}${frac ? `.${frac}` : ""}`;
}

/** Display text with thousands separators: 1234500 -> "1,234.5". */
export function formatQty(thousandths: number): string {
  const t = qtyToInput(thousandths);
  const neg = t.startsWith("-");
  const [w = "0", f] = (neg ? t.slice(1) : t).split(".");
  return `${neg ? "-" : ""}${w.replace(/\B(?=(\d{3})+(?!\d))/g, ",")}${f ? `.${f}` : ""}`;
}