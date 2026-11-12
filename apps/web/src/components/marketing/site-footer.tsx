import Link from "next/link";
import { Logo } from "@/components/ui/logo";

export function SiteFooter() {
  return (
    <footer className="border-t border-border bg-bg-subtle">
      <div className="mx-auto grid max-w-7xl gap-10 px-5 py-14 sm:px-8 md:grid-cols-[1.4fr_1fr_1fr_1fr]">
        <div>
          <Logo />
          <p className="mt-4 max-w-xs text-sm text-muted">One business. One platform. Total control.</p>
          <p className="mt-6 text-xs text-subtle">PERMTPA ONE is a technology product by PERMTPA.</p>
        </div>
        {[
          ["Product", [["Platform", "/#platform"], ["Intelligence", "/#intelligence"], ["Pricing", "/#pricing"]]],
          ["Company", [["About PERMTPA", "/about"], ["Contact", "/contact"], ["Security", "/#security"]]],
          ["Legal", [["Privacy policy", "/privacy"], ["Terms of service", "/terms"]]],
        ].map(([h, items]) => (
          <div key={h as string}>
            <p className="text-sm font-semibold">{h as string}</p>
            <ul className="mt-4 space-y-2.5">{(items as string[][]).map(([l, href]) => <li key={l}><Link href={href!} className="text-sm text-muted hover:text-fg">{l}</Link></li>)}</ul>
          </div>
        ))}
      </div>
      <div className="border-t border-border py-5 text-center text-xs text-subtle">© {new Date().getFullYear()} PERMTPA. All rights reserved.</div>
    </footer>
  );
}
