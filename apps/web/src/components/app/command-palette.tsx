"use client";
import { useRouter } from "next/navigation";
import { useMemo, useState } from "react";
import { CornerDownLeft, Moon, Search, Sun } from "@/components/icons";
import { useTheme } from "next-themes";
import { NAV } from "@/lib/nav";
import { Kbd } from "@/components/ui/kbd";
import { cn } from "@/lib/utils";

type Cmd = { id: string; label: string; hint?: string; icon: React.ComponentType<{ className?: string }>; run: () => void };

export function CommandPalette({ open, onClose }: { open: boolean; onClose: () => void }) {
  return open ? <PaletteDialog onClose={onClose} /> : null;
}

function PaletteDialog({ onClose }: { onClose: () => void }) {
  const router = useRouter();
  const { resolvedTheme, setTheme } = useTheme();
  const [q, setQ] = useState("");
  const [active, setActive] = useState(0);

  const commands: Cmd[] = useMemo(() => [
    ...NAV.map((n) => ({ id: n.slug, label: `Go to ${n.label}`, hint: n.shortcut, icon: n.icon, run: () => router.push(n.href) })),
    { id: "theme", label: resolvedTheme === "dark" ? "Switch to light theme" : "Switch to dark theme", icon: resolvedTheme === "dark" ? Sun : Moon, run: () => setTheme(resolvedTheme === "dark" ? "light" : "dark") },
  ], [router, resolvedTheme, setTheme]);

  const results = useMemo(() => commands.filter((c) => c.label.toLowerCase().includes(q.trim().toLowerCase())), [commands, q]);


  const run = (c?: Cmd) => { if (!c) return; onClose(); c.run(); };

  return (
    <div className="animate-fade-in fixed inset-0 z-[60] flex items-start justify-center bg-navy/50 px-4 pt-[14vh] backdrop-blur-sm" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div role="dialog" aria-modal="true" aria-label="Command palette" className="animate-pop-in w-full max-w-xl origin-top overflow-hidden rounded-2xl border border-border-strong bg-raised shadow-lg"
        onKeyDown={(e) => {
          if (e.key === "Escape") onClose();
          if (e.key === "ArrowDown") { e.preventDefault(); setActive((a) => Math.min(a + 1, results.length - 1)); }
          if (e.key === "ArrowUp") { e.preventDefault(); setActive((a) => Math.max(a - 1, 0)); }
          if (e.key === "Enter") run(results[active]);
        }}>
        <div className="flex items-center gap-3 border-b border-border px-4">
          <Search className="size-4 text-subtle" />
          <input autoFocus value={q} onChange={(e) => { setQ(e.target.value); setActive(0); }} placeholder="Search pages and actions…" aria-label="Search commands" className="h-14 flex-1 bg-transparent text-sm outline-none placeholder:text-subtle" />
          <Kbd>esc</Kbd>
        </div>
        <ul className="scroll-thin max-h-80 overflow-y-auto p-2" role="listbox">
          {results.length === 0 && <li className="px-3 py-10 text-center text-sm text-muted">No results for &ldquo;{q}&rdquo;</li>}
          {results.map((c, i) => (
            <li key={c.id} role="option" aria-selected={i === active}>
              <button type="button" onMouseEnter={() => setActive(i)} onClick={() => run(c)} className={cn("flex w-full cursor-pointer items-center gap-3 rounded-lg px-3 py-2.5 text-left text-sm", i === active ? "bg-bg-subtle text-fg" : "text-muted")}>
                <c.icon className="size-4" /><span className="flex-1">{c.label}</span>
                {c.hint && <span className="flex gap-1">{c.hint.split(" ").map((k) => <Kbd key={k}>{k}</Kbd>)}</span>}
                {i === active && <CornerDownLeft className="size-3.5 text-subtle" />}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
