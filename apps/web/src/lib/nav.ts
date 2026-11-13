import { BarChart3, Bell, Box, Building2, History, LayoutDashboard, Landmark, Settings, ShoppingCart, Sparkles, Truck, UserCog, Users, type Icon } from "@/components/icons";

export type NavItem = { slug: string; label: string; href: string; icon: Icon; section: "Operate" | "Grow" | "Admin"; milestone: number; blurb: string; shortcut?: string };

export const NAV: NavItem[] = [
  { slug: "dashboard", label: "Dashboard", href: "/dashboard", icon: LayoutDashboard, section: "Operate", milestone: 1, blurb: "Your business at a glance.", shortcut: "G D" },
  { slug: "sales", label: "Sales & POS", href: "/sales", icon: ShoppingCart, section: "Operate", milestone: 4, blurb: "Checkout, receipts, returns and refunds.", shortcut: "G S" },
  { slug: "inventory", label: "Inventory", href: "/inventory", icon: Box, section: "Operate", milestone: 3, blurb: "Products, variants, stock movements and purchase orders.", shortcut: "G I" },
  { slug: "customers", label: "Customers", href: "/customers", icon: Users, section: "Operate", milestone: 4, blurb: "Profiles, balances, follow-ups and lifetime value.", shortcut: "G C" },
  { slug: "suppliers", label: "Suppliers", href: "/suppliers", icon: Truck, section: "Operate", milestone: 4, blurb: "Suppliers and purchasing." },
  { slug: "finance", label: "Finance", href: "/finance", icon: Landmark, section: "Grow", milestone: 5, blurb: "Income, expenses, receivables, payables and reports.", shortcut: "G F" },
  { slug: "analytics", label: "Analytics", href: "/analytics", icon: BarChart3, section: "Grow", milestone: 5, blurb: "Margins, turnover, retention and forecasts." },
  { slug: "intelligence", label: "Intelligence", href: "/intelligence", icon: Sparkles, section: "Grow", milestone: 7, blurb: "Ask questions about your business in plain language." },
  { slug: "automations", label: "Automations", href: "/automations", icon: Bell, section: "Grow", milestone: 7, blurb: "Reminders, alerts and scheduled reports." },
  { slug: "team", label: "Team", href: "/team", icon: UserCog, section: "Admin", milestone: 3, blurb: "Employees, roles, shifts and attendance." },
  { slug: "audit-log", label: "Audit log", href: "/audit-log", icon: History, section: "Admin", milestone: 5, blurb: "A tamper-resistant record of sensitive actions." },
  { slug: "organization", label: "Organization", href: "/organization", icon: Building2, section: "Admin", milestone: 3, blurb: "Business profile, locations and billing." },
  { slug: "settings", label: "Settings", href: "/settings", icon: Settings, section: "Admin", milestone: 2, blurb: "Profile, security, sessions and two-factor authentication." },
];

export const NAV_SECTIONS = ["Operate", "Grow", "Admin"] as const;
export const findNav = (slug: string) => NAV.find((n) => n.slug === slug);
