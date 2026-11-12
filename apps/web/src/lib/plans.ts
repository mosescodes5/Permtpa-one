import { naira } from "./money";

/** Initial, configurable proposed prices. Server-side billing (Milestone 6) is the source of truth. */
export type Plan = {
  id: "free" | "starter" | "professional" | "business";
  name: string;
  tagline: string;
  monthly: bigint;
  /** Annual = 10 × monthly (two months free) — a proposed default, configurable. */
  annual: bigint;
  highlight?: boolean;
  features: string[];
};

const annual = (monthlyNaira: number) => naira(monthlyNaira * 10);

export const PLANS: Plan[] = [
  { id: "free", name: "Free", tagline: "For getting started", monthly: 0n, annual: 0n,
    features: ["1 user", "Up to 50 products", "Basic point of sale", "Core dashboard"] },
  { id: "starter", name: "Starter", tagline: "For growing shops", monthly: naira(3000), annual: annual(3000),
    features: ["Up to 3 users", "Unlimited products", "Customer records", "Expense tracking", "Email receipts"] },
  { id: "professional", name: "Professional", tagline: "For serious operators", monthly: naira(8000), annual: annual(8000), highlight: true,
    features: ["Up to 10 users", "Roles & permissions", "Purchase orders", "Analytics & exports", "PERMTPA ONE Intelligence", "Automations"] },
  { id: "business", name: "Business", tagline: "For multi-team businesses", monthly: naira(20000), annual: annual(20000),
    features: ["Unlimited users", "Audit logs", "Advanced reporting", "Priority support", "WhatsApp & email integrations"] },
];
