/** SAMPLE DATA ONLY — used to demonstrate the dashboard shell. Real data arrives with the API (Milestones 3–5). Money is in kobo. */
export type Range = "7d" | "30d" | "90d";

function series(days: number) {
  const out: { label: string; revenue: number; profit: number }[] = [];
  const start = new Date(Date.UTC(2026, 10, 11));
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(start); d.setUTCDate(start.getUTCDate() - i);
    const wave = Math.sin((days - i) / 3.1) * 0.18 + ((days - i) / days) * 0.35;
    const revenue = Math.round((620_000 + wave * 520_000 + ((i * 7919) % 90_000)) / 100) * 10_000;
    out.push({ label: d.toLocaleDateString("en-NG", { day: "numeric", month: "short", timeZone: "UTC" }), revenue, profit: Math.round(revenue * 0.39) });
  }
  return out;
}

export const REVENUE_SERIES: Record<Range, ReturnType<typeof series>> = { "7d": series(7), "30d": series(30), "90d": series(90) };

export const sumSeries = (r: Range) => REVENUE_SERIES[r].reduce((a, p) => ({ revenue: a.revenue + p.revenue, profit: a.profit + p.profit }), { revenue: 0, profit: 0 });

export const TOP_PRODUCTS = [
  { name: "Premium Rice 50kg", sku: "RIC-050", units: 142, revenue: 2_272_000_00 },
  { name: "Vegetable Oil 5L", sku: "OIL-005", units: 311, revenue: 1_866_000_00 },
  { name: "Cement 50kg", sku: "CEM-050", units: 96, revenue: 960_000_00 },
  { name: "Bottled Water (24 pk)", sku: "WAT-024", units: 480, revenue: 720_000_00 },
];

export const LOW_STOCK = [
  { name: "Vegetable Oil 5L", left: 8, reorder: 25 },
  { name: "Tomato Paste 400g", left: 14, reorder: 40 },
  { name: "Premium Rice 50kg", left: 19, reorder: 30 },
];

export const RECENT = [
  { id: "SL-10482", customer: "Adaeze Okafor", method: "Transfer", amount: 48_500_00, status: "paid" as const, time: "09:42" },
  { id: "SL-10481", customer: "Walk-in", method: "Cash", amount: 12_000_00, status: "paid" as const, time: "09:31" },
  { id: "SL-10480", customer: "Musa Ibrahim", method: "Credit", amount: 156_000_00, status: "owing" as const, time: "09:08" },
  { id: "SL-10479", customer: "Chidi Foods Ltd", method: "Card", amount: 89_250_00, status: "paid" as const, time: "08:55" },
  { id: "SL-10478", customer: "Walk-in", method: "Cash", amount: 6_400_00, status: "refunded" as const, time: "08:30" },
];
