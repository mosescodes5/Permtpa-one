import { Fragment } from "react";
import Link from "next/link";
import { ArrowRight, BarChart3, Box, ClipboardList, Fingerprint, KeyRound, Landmark, LockKeyhole, ScrollText, ShieldCheck, ShoppingCart, Sparkles, Users, Wallet } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { Reveal } from "@/components/reveal";
import { ProductPreview } from "@/components/marketing/product-preview";
import { Pricing } from "@/components/marketing/pricing";

const industries = ["Retail", "Restaurants", "Agriculture", "Distribution", "Services", "Agencies"];

const modules = [
  { icon: ShoppingCart, title: "Sales & point of sale", body: "Fast, keyboard-driven checkout with discounts, tax, split payments, digital receipts, returns and refunds.", span: "lg:col-span-2" },
  { icon: Box, title: "Inventory you can trust", body: "Every movement is recorded and auditable. Variants, barcodes, reorder levels and valuation." },
  { icon: Users, title: "Customer relationships", body: "Purchase history, balances, follow-ups and lifetime value in a single profile." },
  { icon: Wallet, title: "Finance without the guesswork", body: "Revenue, gross profit, net profit and cash are kept distinct, so the numbers always mean what they say.", span: "lg:col-span-2" },
  { icon: ClipboardList, title: "Team & permissions", body: "Seven roles with precise access. Staff see what they need and nothing more." },
  { icon: BarChart3, title: "Analytics that explain", body: "Margins, turnover, retention and forecasts — with transparent, auditable calculations." },
];

const security = [
  { icon: Landmark, t: "Isolated by design", d: "Each organization's data is separated at the application and the database level." },
  { icon: KeyRound, t: "Modern authentication", d: "Argon2id password hashing, secure cookie sessions, optional multi-factor and recovery codes." },
  { icon: ScrollText, t: "Audit trail", d: "Sensitive actions — refunds, stock changes, permission edits — are recorded and tamper-resistant." },
  { icon: Fingerprint, t: "Verified payments", d: "Subscription status is confirmed server-side. A browser redirect alone never unlocks anything." },
];

export default function HomePage() {
  return (
    <>
      <section className="relative overflow-hidden">
        <div className="bg-grid absolute inset-0 -z-10 opacity-70" aria-hidden />
        <div className="absolute left-1/2 top-0 -z-10 h-[420px] w-[820px] -translate-x-1/2 rounded-full bg-accent/10 blur-[110px] animate-drift" aria-hidden />
        <div className="mx-auto max-w-7xl px-5 pb-10 pt-16 text-center sm:px-8 sm:pt-24">
          <Reveal>
            <p className="mx-auto inline-flex items-center gap-2 rounded-full border border-border-strong bg-surface px-3.5 py-1.5 text-xs font-medium text-muted shadow-sm">
              <span className="size-1.5 rounded-full bg-accent" /> The business operating system by PERMTPA
            </p>
          </Reveal>
          <Reveal delay={0.06}>
            <h1 className="font-display mx-auto mt-7 max-w-4xl text-balance text-[2.75rem] leading-[1.02] sm:text-7xl">
              {["Run", "your", "whole", "business", "from"].map((w, i) => <Fragment key={w}><span className="rise inline-block" style={{ "--i": i } as React.CSSProperties}>{w}</span>{" "}</Fragment>)}
              <em className="rise inline-block text-accent not-italic" style={{ "--i": 5 } as React.CSSProperties}>one place.</em>
            </h1>
          </Reveal>
          <Reveal delay={0.12}>
            <p className="mx-auto mt-6 max-w-2xl text-base leading-relaxed text-muted sm:text-lg">
              PERMTPA ONE brings sales, stock, customers, money and your team into a single secure platform — so you can open it in the morning and instantly know how your business is doing.
            </p>
          </Reveal>
          <Reveal delay={0.18} className="mt-9 flex flex-col items-center justify-center gap-3 sm:flex-row">
            <Link href="/register"><Button variant="navy" size="lg">Start free <ArrowRight /></Button></Link>
            <Link href="/#platform"><Button variant="secondary" size="lg">Explore the platform</Button></Link>
          </Reveal>
          <Reveal delay={0.22}><p className="mt-5 text-xs text-subtle">Free plan available · No card required</p></Reveal>
          <Reveal delay={0.28} className="mx-auto mt-14 max-w-5xl text-left"><div className="animate-float"><ProductPreview /></div></Reveal>
        </div>
      </section>

      <section aria-label="Industries" className="border-y border-border bg-bg-subtle">
        <div className="marquee-pause mx-auto flex max-w-7xl items-center gap-8 overflow-hidden px-5 py-7 sm:px-8 [mask-image:linear-gradient(90deg,transparent,#000_12%,#000_88%,transparent)]">
          <span className="shrink-0 text-xs font-medium uppercase tracking-[0.18em] text-subtle">Built for</span>
          <div className="flex overflow-hidden"><div className="animate-marquee flex shrink-0 gap-12 pr-12">
            {[...industries, ...industries].map((i, n) => <span key={n} aria-hidden={n >= industries.length} className="font-display shrink-0 text-xl text-muted transition-colors hover:text-accent">{i}</span>)}
          </div></div>
        </div>
      </section>

      <section id="platform" className="mx-auto max-w-7xl scroll-mt-20 px-5 py-24 sm:px-8">
        <Reveal className="max-w-2xl">
          <p className="text-sm font-medium text-accent">The platform</p>
          <h2 className="font-display mt-3 text-4xl sm:text-5xl">Everything your business runs on, finally connected.</h2>
          <p className="mt-4 text-muted">Stop stitching together spreadsheets, notebooks and chat threads. Every module shares the same data, so a sale updates stock, customer balances and your books at once.</p>
        </Reveal>
        <div className="mt-12 grid gap-4 lg:grid-cols-4">
          {modules.map((m, i) => (
            <Reveal key={m.title} delay={i * 0.04} className={m.span ?? ""}>
              <div data-hover-scope className="group lift h-full rounded-2xl border border-border bg-surface p-7 shadow-sm">
                <span className="inline-flex size-11 items-center justify-center rounded-xl bg-accent-soft text-accent transition-[background,color,transform] duration-300 group-hover:rotate-[-4deg] group-hover:scale-110 group-hover:bg-accent group-hover:text-accent-fg"><m.icon className="size-6" /></span>
                <h3 className="mt-5 text-lg font-semibold">{m.title}</h3>
                <p className="mt-2 text-sm leading-relaxed text-muted">{m.body}</p>
              </div>
            </Reveal>
          ))}
        </div>
      </section>

      <section id="intelligence" className="scroll-mt-20 bg-navy text-[#f4f1ea]">
        <div className="mx-auto grid max-w-7xl items-center gap-14 px-5 py-24 sm:px-8 lg:grid-cols-2">
          <Reveal>
            <p className="flex items-center gap-2 text-sm font-medium text-[#2fc58f]"><Sparkles className="size-4" /> PERMTPA ONE Intelligence</p>
            <h2 className="font-display mt-3 text-4xl sm:text-5xl">Ask your business a question. Get a real answer.</h2>
            <p className="mt-4 max-w-lg text-[#a5aec2]">Intelligence reads only the data you&apos;re authorized to see, shows the reporting period and how each figure was calculated, and never invents a number.</p>
            <ul className="mt-8 space-y-3 text-sm text-[#c9d0e0]">
              {["Which customers owe me money?", "Which products need restocking?", "Compare this month with last month."].map((q) => <li key={q} className="flex gap-3"><span className="mt-2 size-1.5 shrink-0 rounded-full bg-[#2fc58f]" />{q}</li>)}
            </ul>
          </Reveal>
          <Reveal delay={0.1}>
            <div className="rounded-2xl border border-white/10 bg-white/[0.04] p-5 shadow-lg backdrop-blur-sm" role="img" aria-label="Example Intelligence conversation with sample data">
              <p className="ml-auto w-fit max-w-[85%] rounded-2xl rounded-br-md bg-[#2fc58f] px-4 py-2.5 text-sm text-[#04140f]">How much profit did I make this month?</p>
              <div className="mt-4 max-w-[92%] rounded-2xl rounded-bl-md border border-white/10 bg-white/[0.05] p-4 text-sm">
                <p>Your gross profit for November so far is <strong className="tabular">₦1,284,500</strong>, up 8.1% on October at the same point.</p>
                <p className="mt-3 rounded-lg bg-black/20 p-3 font-mono text-[11px] leading-relaxed text-[#a5aec2]">Period: 1–11 Nov 2026<br />Gross profit = sales revenue − cost of goods sold<br />Source: 214 completed sales · refunds excluded</p>
              </div>
              <p className="mt-3 text-center text-[11px] text-[#6f7a92]">Illustrative example · sample data</p>
            </div>
          </Reveal>
        </div>
      </section>

      <section id="security" className="mx-auto max-w-7xl scroll-mt-20 px-5 py-24 sm:px-8">
        <Reveal className="max-w-2xl">
          <p className="flex items-center gap-2 text-sm font-medium text-accent"><ShieldCheck className="size-4" /> Security</p>
          <h2 className="font-display mt-3 text-4xl sm:text-5xl">Security is a product requirement, not an afterthought.</h2>
          <p className="mt-4 text-muted">Your business data is sensitive. PERMTPA ONE is engineered with the OWASP Top 10 and ASVS as its baseline, and we document threats, mitigations and remaining risks openly. No system is unhackable — we design for defence in depth.</p>
        </Reveal>
        <div className="mt-12 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {security.map((s, i) => (
            <Reveal key={s.t} delay={i * 0.05}>
              <div data-hover-scope className="group lift h-full rounded-2xl border border-border bg-surface p-6"><s.icon className="size-6 text-accent" /><h3 className="mt-4 font-semibold">{s.t}</h3><p className="mt-2 text-sm leading-relaxed text-muted">{s.d}</p></div>
            </Reveal>
          ))}
        </div>
      </section>

      <section id="pricing" className="scroll-mt-20 border-t border-border bg-bg-subtle">
        <div className="mx-auto max-w-7xl px-5 py-24 sm:px-8">
          <Reveal className="mx-auto mb-10 max-w-2xl text-center">
            <h2 className="font-display text-4xl sm:text-5xl">Simple pricing that scales with you.</h2>
            <p className="mt-4 text-muted">Start free. Upgrade when your business needs more.</p>
          </Reveal>
          <Pricing />
        </div>
      </section>

      <section className="px-5 pb-24 pt-8 sm:px-8">
        <div className="relative mx-auto max-w-5xl overflow-hidden rounded-3xl bg-navy px-8 py-16 text-center text-[#f4f1ea]">
          <div className="absolute -top-24 left-1/2 h-64 w-[600px] -translate-x-1/2 rounded-full bg-[#2fc58f]/20 blur-[90px]" aria-hidden />
          <LockKeyhole className="relative mx-auto size-6 text-[#2fc58f]" />
          <h2 className="font-display relative mt-4 text-4xl sm:text-5xl">One business. One platform. Total control.</h2>
          <Link href="/register" className="relative mt-8 inline-block"><Button variant="primary" size="lg">Create your free account <ArrowRight /></Button></Link>
        </div>
      </section>
    </>
  );
}
