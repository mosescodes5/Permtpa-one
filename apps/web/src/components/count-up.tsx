"use client";
import { animate, motion, useInView, useMotionValue, useReducedMotion, useTransform } from "motion/react";
import { useEffect, useRef } from "react";
import { formatMoney } from "@/lib/money";

/** Animated number: counts up from 0 when first scrolled into view, then eases between values (e.g. when a filter changes).
 *  Server HTML contains the final formatted value, so it is correct without JavaScript. Values are integers (minor units). */
export function CountUp({ value, format, className }: { value: number; format: (n: number) => string; className?: string }) {
  const ref = useRef<HTMLSpanElement>(null);
  const inView = useInView(ref, { once: true });
  const reduce = useReducedMotion();
  const mv = useMotionValue(value);
  const text = useTransform(mv, (v) => format(Math.round(v)));
  const started = useRef(false);

  useEffect(() => {
    if (reduce) { mv.set(value); return; }
    if (!started.current) {
      if (!inView) return;
      started.current = true;
      mv.set(0);
    }
    const controls = animate(mv, value, { duration: 1.1, ease: [0.22, 1, 0.36, 1] });
    return () => controls.stop();
  }, [value, inView, reduce, mv]);

  return <motion.span ref={ref} className={className} aria-label={format(value)}>{text}</motion.span>;
}

/** Serializable wrapper so Server Components can render animated money (functions cannot cross the server/client boundary). */
export function MoneyCountUp({ value, compact = false, className }: { value: number; compact?: boolean; className?: string }) {
  return <CountUp value={value} className={className} format={(n) => formatMoney(n, "NGN", { compact })} />;
}
