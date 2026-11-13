"use client";
import { useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";

/** Minimal accessible popover: click-outside + Escape to close, focus returns to trigger. */
export function Popover({ trigger, children, align = "right", className }: { trigger: (p: { open: boolean; toggle: () => void; props: { "aria-haspopup": "true"; "aria-expanded": boolean } }) => React.ReactNode; children: (close: () => void) => React.ReactNode; align?: "left" | "right"; className?: string }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) setOpen(false); };
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    document.addEventListener("mousedown", onDown); document.addEventListener("keydown", onKey);
    return () => { document.removeEventListener("mousedown", onDown); document.removeEventListener("keydown", onKey); };
  }, [open]);
  return (
    <div ref={ref} className="relative">
      {trigger({ open, toggle: () => setOpen((o) => !o), props: { "aria-haspopup": "true", "aria-expanded": open } })}
      {open && <div role="menu" className={cn("animate-pop-in absolute z-50 mt-2 min-w-64 rounded-xl border border-border-strong bg-raised p-1.5 shadow-lg", align === "right" ? "right-0" : "left-0", className)}>{children(() => setOpen(false))}</div>}
    </div>
  );
}
