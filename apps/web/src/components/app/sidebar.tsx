"use client";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { NAV, NAV_SECTIONS } from "@/lib/nav";
import { LogoMark, Logo } from "@/components/ui/logo";
import { cn } from "@/lib/utils";

export function Sidebar({ collapsed, onToggle, onNavigate }: { collapsed: boolean; onToggle?: () => void; onNavigate?: () => void }) {
  const path = usePathname();
  return (
    <div className="flex h-full flex-col bg-navy text-[#c9d0e0]">
      <div className={cn("flex h-16 items-center border-b border-white/10", collapsed ? "justify-center" : "px-5")}>
        {collapsed ? <LogoMark /> : <span className="text-[#f4f1ea]"><Logo /></span>}
      </div>
      <nav aria-label="Main" className="scroll-thin flex-1 overflow-y-auto px-3 py-4">
        {NAV_SECTIONS.map((s) => (
          <div key={s} className="mb-5">
            {!collapsed && <p className="mb-1.5 px-3 text-[10px] font-semibold uppercase tracking-[0.16em] text-[#6f7a92]">{s}</p>}
            {collapsed && <div className="mx-3 mb-2 h-px bg-white/10" />}
            <ul className="space-y-0.5">
              {NAV.filter((n) => n.section === s).map((n) => {
                const active = path === n.href || path.startsWith(n.href + "/");
                return (
                  <li key={n.slug}>
                    <Link href={n.href} onClick={onNavigate} title={collapsed ? n.label : undefined} aria-current={active ? "page" : undefined}
                      className={cn("group relative flex items-center gap-3 rounded-lg py-2 text-sm transition-colors", collapsed ? "justify-center px-0" : "px-3", active ? "bg-white/10 text-white" : "hover:bg-white/5 hover:text-white")}>
                      {active && <span className="absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-full bg-[#2fc58f]" />}
                      <n.icon className={cn("size-[18px] shrink-0", active && "text-[#2fc58f]")} />
                      {!collapsed && <span>{n.label}</span>}
                      {collapsed && <span className="sr-only">{n.label}</span>}
                    </Link>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </nav>
      {onToggle && (
        <button type="button" onClick={onToggle} aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"} className="m-3 flex cursor-pointer items-center justify-center gap-2 rounded-lg border border-white/10 py-2 text-xs text-[#8b95ab] hover:bg-white/5 hover:text-white">
          {collapsed ? <PanelLeftOpen className="size-4" /> : <><PanelLeftClose className="size-4" /> Collapse</>}
        </button>
      )}
    </div>
  );
}
