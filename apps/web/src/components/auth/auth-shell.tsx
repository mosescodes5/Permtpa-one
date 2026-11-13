import Link from "next/link";
import { Logo } from "@/components/ui/logo";
import { ThemeToggle } from "@/components/theme-toggle";

export function AuthShell({ title, subtitle, children, footer }: { title: string; subtitle: string; children: React.ReactNode; footer: React.ReactNode }) {
  return (
    <div className="grid min-h-dvh lg:grid-cols-[1fr_1.05fr]">
      <div className="flex flex-col px-5 py-6 sm:px-10">
        <div className="flex items-center justify-between"><Link href="/" aria-label="PERMTPA ONE home"><Logo /></Link><ThemeToggle /></div>
        <main className="mx-auto flex w-full max-w-[400px] flex-1 flex-col justify-center py-10">
          <h1 className="font-display text-4xl">{title}</h1>
          <p className="mt-2 text-sm text-muted">{subtitle}</p>
          <div className="mt-8">{children}</div>
          <div className="mt-8 text-center text-sm text-muted">{footer}</div>
        </main>
        <p className="text-center text-xs text-subtle">PERMTPA ONE is a technology product by PERMTPA.</p>
      </div>
      <aside className="relative hidden overflow-hidden bg-navy p-12 text-[#f4f1ea] lg:flex lg:flex-col lg:justify-between" aria-hidden>
        <div className="animate-drift absolute -right-24 -top-24 size-[480px] rounded-full bg-[#2fc58f]/15 blur-[100px]" />
        <div className="bg-grid absolute inset-0 opacity-25" />
        <p className="relative text-xs font-medium uppercase tracking-[0.2em] text-[#2fc58f]">One business. One platform.</p>
        <div className="relative">
          <p className="font-display text-5xl leading-[1.08]">Open it in the morning.<br />Know your business by breakfast.</p>
          <div className="mt-10 grid max-w-sm grid-cols-2 gap-3">
            {[["Sales", "Every transaction, live"], ["Stock", "Every movement, audited"], ["Cash", "Always reconciled"], ["Team", "Precise permissions"]].map(([a, b]) => (
              <div key={a} className="rounded-xl border border-white/10 bg-white/[0.04] p-4"><p className="text-sm font-semibold">{a}</p><p className="mt-1 text-xs text-[#a5aec2]">{b}</p></div>
            ))}
          </div>
        </div>
        <p className="relative text-xs text-[#6f7a92]">Argon2id hashing · HttpOnly sessions · Tenant isolation</p>
      </aside>
    </div>
  );
}
