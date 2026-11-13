import { ArrowUpRight, Box, LayoutDashboard, Receipt, ShoppingCart, Sparkles, Users, Wallet } from "@/components/icons";
import { MoneyCountUp } from "@/components/count-up";

const series = [38, 44, 41, 52, 49, 61, 58, 70, 66, 79, 74, 92];

function area(values: number[], w: number, h: number) {
  const max = Math.max(...values) * 1.1;
  const pts = values.map((v, i) => [(i / (values.length - 1)) * w, h - (v / max) * h] as const);
  const line = pts.map(([x, y], i) => `${i ? "L" : "M"}${x.toFixed(1)} ${y.toFixed(1)}`).join(" ");
  return { line, fill: `${line} L${w} ${h} L0 ${h} Z` };
}

/** Static, illustrative product preview. All figures are SAMPLE data. */
export function ProductPreview() {
  const { line, fill } = area(series, 520, 150);
  const nav = [LayoutDashboard, ShoppingCart, Box, Users, Wallet, Receipt];
  return (
    <div className="overflow-hidden rounded-2xl border border-border-strong bg-surface shadow-lg" role="img" aria-label="Preview of the PERMTPA ONE dashboard with sample data">
      <div className="flex items-center gap-1.5 border-b border-border bg-bg-subtle px-4 py-2.5">
        <span className="size-2.5 rounded-full bg-[#ff6b5e]/70" /><span className="size-2.5 rounded-full bg-[#f5bd4f]/70" /><span className="size-2.5 rounded-full bg-[#39c36a]/70" />
        <span className="mx-auto rounded-md bg-surface px-3 py-0.5 font-mono text-[10px] text-subtle">one.permtpa.com/dashboard</span>
      </div>
      <div className="grid grid-cols-[52px_1fr] sm:grid-cols-[168px_1fr]">
        <aside className="hidden flex-col gap-1 border-r border-border bg-navy p-3 text-[#c9d0e0] sm:flex">
          <div className="mb-3 px-2 text-[11px] font-semibold tracking-[0.14em] text-white">PERMTPA <span className="text-[#2fc58f]">ONE</span></div>
          {["Dashboard", "Sales", "Inventory", "Customers", "Finance", "Reports"].map((l, i) => {
            const Icon = nav[i]!;
            return <div key={l} className={`flex items-center gap-2 rounded-md px-2 py-1.5 text-xs ${i === 0 ? "bg-white/10 text-white" : ""}`}><Icon className="size-3.5" />{l}</div>;
          })}
        </aside>
        <aside className="flex flex-col items-center gap-3 border-r border-border bg-navy py-4 text-[#c9d0e0] sm:hidden">
          {nav.map((Icon, i) => <Icon key={i} className={`size-4 ${i === 0 ? "text-white" : ""}`} />)}
        </aside>
        <div className="p-4 sm:p-5">
          <div className="mb-4 flex items-center justify-between">
            <div><p className="text-[11px] text-subtle">Good morning</p><p className="text-sm font-semibold">Business overview</p></div>
            <span className="rounded-full bg-accent-soft px-2.5 py-1 text-[11px] font-medium text-accent">Sample data</span>
          </div>
          <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
            {[["Revenue today", 184_250_00, "+12.4%"], ["Gross profit", 71_900_00, "+8.1%"], ["Expenses", 26_400_00, "-3.2%"], ["Owed to you", 312_000_00, "14 invoices"]].map(([k, v, d]) => (
              <div key={k} className="rounded-lg border border-border bg-bg p-3">
                <p className="text-[10px] text-subtle">{k}</p>
                <p className="tabular mt-1 text-base font-semibold sm:text-lg"><MoneyCountUp value={v as number} compact /></p>
                <p className="mt-0.5 text-[10px] text-accent">{d}</p>
              </div>
            ))}
          </div>
          <div className="mt-3 grid gap-3 lg:grid-cols-[1.6fr_1fr]">
            <div className="rounded-lg border border-border bg-bg p-3">
              <div className="flex items-center justify-between"><p className="text-xs font-medium">Revenue · last 12 weeks</p><ArrowUpRight className="size-3.5 text-accent" /></div>
              <svg viewBox="0 0 520 150" className="animate-wipe mt-2 h-28 w-full" preserveAspectRatio="none" aria-hidden>
                <defs><linearGradient id="pv" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stopColor="var(--accent)" stopOpacity="0.28" /><stop offset="1" stopColor="var(--accent)" stopOpacity="0" /></linearGradient></defs>
                <path d={fill} fill="url(#pv)" /><path d={line} fill="none" stroke="var(--accent)" strokeWidth="2.2" strokeLinejoin="round" vectorEffect="non-scaling-stroke" />
              </svg>
            </div>
            <div className="hidden rounded-lg border border-border bg-bg p-3 lg:block">
              <p className="flex items-center gap-1.5 text-xs font-medium"><Sparkles className="size-3.5 text-accent" />Intelligence</p>
              <p className="mt-2 text-[11px] leading-relaxed text-muted">Profit is up 8.1% on last month. 6 products are below reorder level.</p>
              <p className="mt-2 rounded-md bg-accent-soft px-2 py-1 text-[10px] text-accent">Period: 1–11 Nov · gross profit = revenue − cost of goods</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
