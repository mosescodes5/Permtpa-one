import { cn } from "@/lib/utils";

/** PERMTPA ONE mark: a rounded navy tile with a stylised "1" formed from an emerald ascent. */
export function LogoMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 32 32" aria-hidden className={cn("size-8", className)}>
      <rect width="32" height="32" rx="9" fill="#0B1426" />
      <rect x="0.5" y="0.5" width="31" height="31" rx="8.5" fill="none" stroke="#ffffff" strokeOpacity="0.1" />
      <path d="M12 12.5 17.5 8v16" fill="none" stroke="#2FC58F" strokeWidth="2.6" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M12.5 24h10" stroke="#F4F1EA" strokeWidth="2.6" strokeLinecap="round" />
    </svg>
  );
}

export function Logo({ className, showProduct = true }: { className?: string; showProduct?: boolean }) {
  return (
    <span className={cn("inline-flex items-center gap-2.5", className)}>
      <LogoMark />
      <span className="flex items-baseline gap-1.5 leading-none">
        <span className="text-[15px] font-semibold tracking-[0.14em]">PERMTPA</span>
        {showProduct && <span className="rounded bg-accent-soft px-1.5 py-0.5 text-[11px] font-semibold tracking-widest text-accent">ONE</span>}
      </span>
    </span>
  );
}
