"use client";
import Link from "next/link";
import { useState } from "react";
import { Check } from "@/components/icons";
import { PLANS } from "@/lib/plans";
import { formatMoneyWhole } from "@/lib/money";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export function Pricing() {
  const [annual, setAnnual] = useState(false);
  return (
    <div>
      <div className="mx-auto flex w-fit items-center rounded-full border border-border-strong bg-surface p-1 text-sm" role="group" aria-label="Billing period">
        {([false, true] as const).map((a) => (
          <button key={String(a)} type="button" aria-pressed={annual === a} onClick={() => setAnnual(a)} className={cn("cursor-pointer rounded-full px-4 py-1.5 font-medium transition-colors", annual === a ? "bg-navy text-[#f4f1ea] dark:bg-[#f4f1ea] dark:text-[#0b1426]" : "text-muted hover:text-fg")}>
            {a ? "Annual" : "Monthly"}{a && <span className="ml-1.5 text-xs text-accent">2 months free</span>}
          </button>
        ))}
      </div>
      <div className="mt-10 grid gap-5 sm:grid-cols-2 xl:grid-cols-4">
        {PLANS.map((p) => (
          <div key={p.id} data-hover-scope className={cn("lift relative flex flex-col rounded-2xl border p-7", p.highlight ? "border-accent bg-surface shadow-lg ring-1 ring-accent" : "border-border bg-surface shadow-sm")}>
            {p.highlight && <span className="absolute -top-3 left-7 rounded-full bg-accent px-3 py-0.5 text-xs font-medium text-accent-fg">Most popular</span>}
            <h3 className="text-lg font-semibold">{p.name}</h3>
            <p className="mt-1 text-sm text-muted">{p.tagline}</p>
            <p className="mt-6 flex items-baseline gap-1">
              <span className="tabular font-display text-5xl">{formatMoneyWhole(annual ? p.annual : p.monthly)}</span>
              <span className="text-sm text-muted">{p.monthly === 0n ? "forever" : annual ? "/year" : "/month"}</span>
            </p>
            <ul className="mt-7 flex-1 space-y-3">
              {p.features.map((f) => <li key={f} className="flex gap-2.5 text-sm text-muted"><Check className="mt-0.5 size-4 shrink-0 text-accent" />{f}</li>)}
            </ul>
            <Link href={`/register?plan=${p.id}`} className="mt-8"><Button variant={p.highlight ? "primary" : "secondary"} className="w-full">{p.monthly === 0n ? "Start free" : `Choose ${p.name}`}</Button></Link>
          </div>
        ))}
      </div>
      <p className="mt-6 text-center text-xs text-subtle">Prices in Nigerian naira. Plan prices are initial proposals and may change before launch.</p>
    </div>
  );
}
