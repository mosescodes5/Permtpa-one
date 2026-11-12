import { cn } from "@/lib/utils";
export function Kbd({ className, ...props }: React.HTMLAttributes<HTMLElement>) {
  return <kbd className={cn("inline-flex h-5 min-w-5 items-center justify-center rounded border border-border-strong bg-bg-subtle px-1 font-mono text-[10px] font-medium text-muted", className)} {...props} />;
}
