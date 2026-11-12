"use client";
import Link from "next/link";
import { AlertTriangle, Bell, Check, ChevronsUpDown, LogOut, Menu, Search, Settings, ShieldCheck } from "lucide-react";
import { Popover } from "@/components/ui/popover";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { ThemeToggle } from "@/components/theme-toggle";

/** Sample organizations/user. Real memberships come from the server in Milestone 2/3. */
const ORGS = [{ name: "Okafor & Sons Trading", role: "Owner", current: true }, { name: "Greenfield Farms", role: "Manager", current: false }];

export function Topbar({ onMenu, onSearch }: { onMenu: () => void; onSearch: () => void }) {
  return (
    <header className="sticky top-0 z-30 flex h-16 items-center gap-3 border-b border-border bg-bg/85 px-4 backdrop-blur-md sm:px-6">
      <Button variant="ghost" size="icon" className="lg:hidden" aria-label="Open navigation" onClick={onMenu}><Menu /></Button>

      <Popover align="left" trigger={({ toggle, props }) => (
        <button type="button" onClick={toggle} {...props} className="flex cursor-pointer items-center gap-2.5 rounded-lg border border-border-strong bg-surface px-2.5 py-1.5 text-left shadow-sm hover:bg-bg-subtle">
          <span className="flex size-7 items-center justify-center rounded-md bg-navy text-xs font-semibold text-[#f4f1ea]">O</span>
          <span className="hidden sm:block"><span className="block text-sm font-medium leading-tight">Okafor &amp; Sons Trading</span><span className="block text-[11px] leading-tight text-subtle">Owner</span></span>
          <ChevronsUpDown className="size-3.5 text-subtle" />
        </button>
      )}>
        {(close) => (<>
          <p className="px-2.5 py-1.5 text-[11px] font-semibold uppercase tracking-wider text-subtle">Organizations</p>
          {ORGS.map((o) => (
            <button key={o.name} role="menuitem" type="button" onClick={close} className="flex w-full cursor-pointer items-center gap-3 rounded-lg px-2.5 py-2 text-left text-sm hover:bg-bg-subtle">
              <span className="flex size-7 items-center justify-center rounded-md bg-bg-subtle text-xs font-semibold">{o.name[0]}</span>
              <span className="flex-1"><span className="block">{o.name}</span><span className="block text-[11px] text-subtle">{o.role}</span></span>
              {o.current && <Check className="size-4 text-accent" />}
            </button>
          ))}
          <p className="border-t border-border px-2.5 pb-1 pt-2 text-[11px] text-subtle">Sample data. Switching is enabled with the tenancy API.</p>
        </>)}
      </Popover>

      <button type="button" onClick={onSearch} className="ml-2 hidden h-10 max-w-md flex-1 cursor-pointer items-center gap-2.5 rounded-lg border border-border-strong bg-surface px-3 text-sm text-subtle shadow-sm hover:bg-bg-subtle md:flex" aria-label="Search (Command K)">
        <Search className="size-4" /><span className="flex-1 text-left">Search or jump to…</span><span className="flex gap-1"><Kbd>Ctrl</Kbd><Kbd>K</Kbd></span>
      </button>

      <div className="ml-auto flex items-center gap-1">
        <Button variant="ghost" size="icon" className="md:hidden" aria-label="Search" onClick={onSearch}><Search /></Button>
        <ThemeToggle />
        <Popover trigger={({ toggle, props }) => (
          <Button variant="ghost" size="icon" aria-label="Notifications, 2 unread" onClick={toggle} {...props} className="relative">
            <Bell /><span className="absolute right-2 top-2 size-2 rounded-full bg-accent ring-2 ring-bg" />
          </Button>
        )}>
          {() => (<div className="w-80 max-w-[calc(100vw-2rem)]">
            <p className="px-2.5 py-1.5 text-sm font-semibold">Notifications</p>
            {[{ i: AlertTriangle, t: "3 products are below reorder level", s: "Sample · 2 min ago" }, { i: ShieldCheck, t: "Daily summary is ready", s: "Sample · 1 hr ago" }].map((n) => (
              <div key={n.t} className="flex gap-3 rounded-lg px-2.5 py-2.5 hover:bg-bg-subtle"><n.i className="mt-0.5 size-4 text-accent" /><div><p className="text-sm">{n.t}</p><p className="text-[11px] text-subtle">{n.s}</p></div></div>
            ))}
          </div>)}
        </Popover>
        <Popover trigger={({ toggle, props }) => (
          <button type="button" onClick={toggle} {...props} aria-label="Account menu" className="ml-1 flex size-9 cursor-pointer items-center justify-center rounded-full bg-accent-soft text-sm font-semibold text-accent ring-1 ring-border-strong">AO</button>
        )}>
          {(close) => (<>
            <div className="px-2.5 py-2"><p className="text-sm font-medium">Adaeze Okafor</p><p className="text-xs text-subtle">adaeze@example.com</p></div>
            <div className="my-1 h-px bg-border" />
            <Link role="menuitem" href="/settings" onClick={close} className="flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm hover:bg-bg-subtle"><Settings className="size-4 text-muted" />Settings</Link>
            <Link role="menuitem" href="/login" onClick={close} className="flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm hover:bg-bg-subtle"><LogOut className="size-4 text-muted" />Log out</Link>
          </>)}
        </Popover>
      </div>
    </header>
  );
}
