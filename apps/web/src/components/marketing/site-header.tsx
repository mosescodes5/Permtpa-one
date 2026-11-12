"use client";
import Link from "next/link";
import { useState } from "react";
import { Menu, X } from "lucide-react";
import { Logo } from "@/components/ui/logo";
import { Button } from "@/components/ui/button";
import { ThemeToggle } from "@/components/theme-toggle";

const links = [
  { href: "/#platform", label: "Platform" },
  { href: "/#intelligence", label: "Intelligence" },
  { href: "/#security", label: "Security" },
  { href: "/#pricing", label: "Pricing" },
];

export function SiteHeader() {
  const [open, setOpen] = useState(false);
  return (
    <header className="sticky top-0 z-40 border-b border-border/70 bg-bg/80 backdrop-blur-md">
      <div className="mx-auto flex h-16 max-w-7xl items-center justify-between px-5 sm:px-8">
        <Link href="/" aria-label="PERMTPA ONE home"><Logo /></Link>
        <nav aria-label="Primary" className="hidden items-center gap-8 md:flex">
          {links.map((l) => <Link key={l.href} href={l.href} className="text-sm text-muted transition-colors hover:text-fg">{l.label}</Link>)}
        </nav>
        <div className="hidden items-center gap-2 md:flex">
          <ThemeToggle />
          <Link href="/login" className="px-3 text-sm font-medium text-muted hover:text-fg">Log in</Link>
          <Link href="/register"><Button variant="navy" size="sm">Get started</Button></Link>
        </div>
        <div className="flex items-center gap-1 md:hidden">
          <ThemeToggle />
          <Button variant="ghost" size="icon" aria-label={open ? "Close menu" : "Open menu"} aria-expanded={open} onClick={() => setOpen(!open)}>{open ? <X /> : <Menu />}</Button>
        </div>
      </div>
      {open && (
        <div className="border-t border-border bg-bg px-5 pb-5 md:hidden">
          <nav aria-label="Mobile" className="flex flex-col py-2">
            {links.map((l) => <Link key={l.href} href={l.href} onClick={() => setOpen(false)} className="py-3 text-base text-fg">{l.label}</Link>)}
          </nav>
          <div className="flex gap-2"><Link href="/login" className="flex-1"><Button variant="secondary" className="w-full">Log in</Button></Link><Link href="/register" className="flex-1"><Button variant="navy" className="w-full">Get started</Button></Link></div>
        </div>
      )}
    </header>
  );
}
