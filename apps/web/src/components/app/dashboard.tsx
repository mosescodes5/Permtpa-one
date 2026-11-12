"use client";
import { useMemo, useState } from "react";
import { Area, AreaChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { AlertTriangle, ArrowDownRight, ArrowUpRight, Info } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { formatMoney } from "@/lib/money";
import { LOW_STOCK, RECENT, REVENUE_SERIES, TOP_PRODUCTS, sumSeries, type Range } from "@/lib/sample-data";
import { cn } from "@/lib/utils";

const RANGES: { id: Range; label: string }[] = [{ id: "7d", label: "7 days" }, { id: "30d", label: "30 days" }, { id: "90d", label: "90 days" }];
const STATUS = { paid: { tone: "accent", label: "Paid" }, owing: { tone: "warning", label: "Owing" }, refunded: { tone: "danger", label: "Refunded" } } as const;

function Kpi({ label, value, delta, up, note }: { label: string; value: string; delta?: string; up?: boolean; note: string }) {
  return (
    <Card><CardContent>
      <p className="text-sm text-muted">{label}</p>
      <p className="tabular font-display mt-2 text-[2rem] leading-none">{value}</p>
      <div className="mt-3 flex items-center gap-2 text-xs">
        {delta && <span className={cn("inline-flex items-center gap-0.5 font-medium", up ? "text-accent" : "text-danger")}>{up ? <ArrowUpRight className="size-3.5" /> : <ArrowDownRight className="size-3.5" />}{delta}</span>}
        <span className="text-subtle">{note}</span>
      </div>
    </CardContent></Card>
  );
}

export function Dashboard() {
  const [range, setRange] = useState<Range>("30d");
  const data = REVENUE_SERIES[range];
  const totals = useMemo(() => sumSeries(range), [range]);
  const expenses = Math.round(totals.revenue * 0.18);
  const net = totals.profit - expenses;

  return (
    <div className="space-y-6">
      <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-end">
        <div><h1 className="font-display text-4xl">Good morning, Adaeze</h1><p className="mt-1 text-sm text-muted">Here&apos;s how Okafor &amp; Sons Trading is performing.</p></div>
        <div role="group" aria-label="Date range" className="flex w-fit rounded-lg border border-border-strong bg-surface p-0.5 shadow-sm">
          {RANGES.map((r) => <button key={r.id} type="button" aria-pressed={range === r.id} onClick={() => setRange(r.id)} className={cn("cursor-pointer rounded-md px-3.5 py-1.5 text-sm font-medium transition-colors", range === r.id ? "bg-navy text-[#f4f1ea] dark:bg-[#f4f1ea] dark:text-[#0b1426]" : "text-muted hover:text-fg")}>{r.label}</button>)}
        </div>
      </div>

      <div role="note" className="flex items-start gap-2.5 rounded-lg border border-border-strong bg-bg-subtle px-4 py-3 text-sm text-muted"><Info className="mt-0.5 size-4 shrink-0 text-accent" /><p>You&apos;re viewing <strong className="font-medium text-fg">sample data</strong> to demonstrate the interface. Live figures appear once sales, inventory and finance modules are connected.</p></div>

      <section aria-label="Key figures" className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <Kpi label="Revenue" value={formatMoney(totals.revenue, "NGN", { compact: true })} delta="12.4%" up note="vs previous period" />
        <Kpi label="Gross profit" value={formatMoney(totals.profit, "NGN", { compact: true })} delta="8.1%" up note="revenue − cost of goods" />
        <Kpi label="Net profit" value={formatMoney(net, "NGN", { compact: true })} delta="2.3%" up note="after expenses" />
        <Kpi label="Outstanding debts" value={formatMoney(312_000_00, "NGN", { compact: true })} delta="4 new" up={false} note="14 customers" />
      </section>

      <div className="grid gap-4 xl:grid-cols-[1.8fr_1fr]">
        <Card>
          <CardHeader><div><CardTitle>Revenue &amp; gross profit</CardTitle><p className="mt-1 text-xs text-subtle">Daily, {RANGES.find((r) => r.id === range)!.label} · NGN</p></div>
            <div className="flex gap-4 text-xs text-muted"><span className="flex items-center gap-1.5"><i className="size-2 rounded-full bg-[var(--chart-1)]" />Revenue</span><span className="flex items-center gap-1.5"><i className="size-2 rounded-full bg-[var(--chart-3)]" />Profit</span></div></CardHeader>
          <CardContent>
            <div className="h-72" role="img" aria-label={`Revenue and gross profit chart for the last ${range}`}>
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={data} margin={{ top: 8, right: 4, left: 0, bottom: 0 }}>
                  <defs><linearGradient id="rev" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stopColor="var(--chart-1)" stopOpacity={0.28} /><stop offset="1" stopColor="var(--chart-1)" stopOpacity={0} /></linearGradient></defs>
                  <CartesianGrid vertical={false} stroke="var(--border)" strokeDasharray="3 3" />
                  <XAxis dataKey="label" tickLine={false} axisLine={false} tick={{ fill: "var(--fg-subtle)", fontSize: 11 }} minTickGap={28} />
                  <YAxis tickLine={false} axisLine={false} width={52} tick={{ fill: "var(--fg-subtle)", fontSize: 11 }} tickFormatter={(v: number) => (v === 0 ? "₦0" : formatMoney(v, "NGN", { compact: true }))} />
                  <Tooltip cursor={{ stroke: "var(--border-strong)" }} contentStyle={{ background: "var(--surface-raised)", border: "1px solid var(--border-strong)", borderRadius: 10, fontSize: 12, boxShadow: "var(--shadow-md)" }} labelStyle={{ color: "var(--fg-muted)" }} formatter={(v) => formatMoney(Number(v))} />
                  <Area type="monotone" dataKey="revenue" name="Revenue" stroke="var(--chart-1)" strokeWidth={2.2} fill="url(#rev)" />
                  <Area type="monotone" dataKey="profit" name="Gross profit" stroke="var(--chart-3)" strokeWidth={2} fill="transparent" />
                </AreaChart>
              </ResponsiveContainer>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader><CardTitle>Low-stock alerts</CardTitle><Badge tone="warning"><AlertTriangle className="size-3" />{LOW_STOCK.length} items</Badge></CardHeader>
          <CardContent className="space-y-5">
            {LOW_STOCK.map((p) => (
              <div key={p.name}>
                <div className="flex items-baseline justify-between text-sm"><span className="font-medium">{p.name}</span><span className="tabular text-muted">{p.left} left</span></div>
                <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-bg-subtle" role="progressbar" aria-valuenow={p.left} aria-valuemin={0} aria-valuemax={p.reorder} aria-label={`${p.name} stock level`}><div className="h-full rounded-full bg-warning" style={{ width: `${Math.min(100, (p.left / p.reorder) * 100)}%` }} /></div>
                <p className="mt-1 text-[11px] text-subtle">Reorder at {p.reorder}</p>
              </div>
            ))}
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-4 xl:grid-cols-2">
        <Card>
          <CardHeader><CardTitle>Top products</CardTitle></CardHeader>
          <CardContent className="overflow-x-auto pt-3">
            <table className="w-full text-sm"><caption className="sr-only">Top products by revenue</caption>
              <thead><tr className="text-left text-xs text-subtle"><th className="pb-2 font-medium">Product</th><th className="pb-2 text-right font-medium">Units</th><th className="pb-2 text-right font-medium">Revenue</th></tr></thead>
              <tbody className="divide-y divide-border">{TOP_PRODUCTS.map((p) => <tr key={p.sku}><td className="py-3"><p className="font-medium">{p.name}</p><p className="font-mono text-[11px] text-subtle">{p.sku}</p></td><td className="tabular py-3 text-right text-muted">{p.units}</td><td className="tabular py-3 text-right font-medium">{formatMoney(p.revenue, "NGN", { compact: true })}</td></tr>)}</tbody>
            </table>
          </CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle>Recent transactions</CardTitle></CardHeader>
          <CardContent className="overflow-x-auto pt-3">
            <table className="w-full text-sm"><caption className="sr-only">Recent transactions</caption>
              <thead><tr className="text-left text-xs text-subtle"><th className="pb-2 font-medium">Sale</th><th className="pb-2 font-medium">Status</th><th className="pb-2 text-right font-medium">Amount</th></tr></thead>
              <tbody className="divide-y divide-border">{RECENT.map((t) => <tr key={t.id}><td className="py-3"><p className="font-medium">{t.customer}</p><p className="text-[11px] text-subtle">{t.id} · {t.method} · {t.time}</p></td><td className="py-3"><Badge tone={STATUS[t.status].tone}>{STATUS[t.status].label}</Badge></td><td className="tabular py-3 text-right font-medium">{formatMoney(t.amount)}</td></tr>)}</tbody>
            </table>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
