"use client";
import { useEffect, useState, useSyncExternalStore } from "react";
import { X } from "lucide-react";
import { Sidebar } from "./sidebar";
import { Topbar } from "./topbar";
import { CommandPalette } from "./command-palette";
import { cn } from "@/lib/utils";

const KEY = "permtpa-one:sidebar-collapsed"; // UI preference only — never auth data.
const listeners = new Set<() => void>();
const read = () => { try { return localStorage.getItem(KEY) === "1"; } catch { return false; } };
const subscribe = (cb: () => void) => { listeners.add(cb); return () => { listeners.delete(cb); }; };
const write = (v: boolean) => { try { localStorage.setItem(KEY, v ? "1" : "0"); } catch { /* storage unavailable */ } listeners.forEach((l) => l()); };

export function AppShell({ children }: { children: React.ReactNode }) {
  const collapsed = useSyncExternalStore(subscribe, read, () => false);
  const [drawer, setDrawer] = useState(false);
  const [palette, setPalette] = useState(false);
  const toggle = () => write(!read());

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") { e.preventDefault(); setPalette((p) => !p); return; }
      const t = e.target as HTMLElement;
      if (t.closest("input, textarea, select, [contenteditable]")) return;
      if (e.key === "[") write(!read());
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="min-h-dvh">
      <a href="#app-main" className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-[70] focus:rounded-lg focus:bg-navy focus:px-4 focus:py-2 focus:text-white">Skip to content</a>
      <aside className={cn("fixed inset-y-0 left-0 z-40 hidden transition-[width] duration-200 lg:block", collapsed ? "w-[72px]" : "w-64")}>
        <Sidebar collapsed={collapsed} onToggle={toggle} />
      </aside>
      {drawer && (
        <div className="fixed inset-0 z-50 lg:hidden">
          <div className="absolute inset-0 bg-navy/60 backdrop-blur-sm" onClick={() => setDrawer(false)} />
          <div className="absolute inset-y-0 left-0 w-72 max-w-[85vw] shadow-lg" role="dialog" aria-modal="true" aria-label="Navigation">
            <Sidebar collapsed={false} onNavigate={() => setDrawer(false)} />
            <button type="button" aria-label="Close navigation" onClick={() => setDrawer(false)} className="absolute right-3 top-4 cursor-pointer rounded-md p-1.5 text-[#c9d0e0] hover:bg-white/10"><X className="size-5" /></button>
          </div>
        </div>
      )}
      <div className={cn("transition-[padding] duration-200", collapsed ? "lg:pl-[72px]" : "lg:pl-64")}>
        <Topbar onMenu={() => setDrawer(true)} onSearch={() => setPalette(true)} />
        <main id="app-main" className="mx-auto max-w-[1400px] px-4 py-6 sm:px-6 lg:py-8">{children}</main>
      </div>
      <CommandPalette open={palette} onClose={() => setPalette(false)} />
    </div>
  );
}
