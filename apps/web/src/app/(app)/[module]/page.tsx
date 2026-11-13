import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { Hourglass } from "@/components/icons";
import { NAV, findNav } from "@/lib/nav";

export function generateStaticParams() { return NAV.filter((n) => n.slug !== "dashboard").map((n) => ({ module: n.slug })); }
export const dynamicParams = false;

export async function generateMetadata({ params }: { params: Promise<{ module: string }> }): Promise<Metadata> {
  const n = findNav((await params).module);
  return { title: n?.label ?? "Not found" };
}

export default async function ModulePage({ params }: { params: Promise<{ module: string }> }) {
  const n = findNav((await params).module);
  if (!n || n.slug === "dashboard") notFound();
  return (
    <div>
      <h1 className="font-display text-4xl">{n.label}</h1>
      <p className="mt-1 text-sm text-muted">{n.blurb}</p>
      <div className="mt-8 flex flex-col items-center rounded-2xl border border-dashed border-border-strong bg-surface px-6 py-20 text-center">
        <span className="flex size-12 items-center justify-center rounded-xl bg-accent-soft text-accent"><n.icon className="size-6" /></span>
        <h2 className="mt-5 text-lg font-semibold">{n.label} is on the roadmap</h2>
        <p className="mt-2 flex max-w-md items-center justify-center gap-1.5 text-sm text-muted"><Hourglass className="size-3.5" />Planned for Milestone {n.milestone}. This area is intentionally empty until the backend for it exists.</p>
      </div>
    </div>
  );
}
