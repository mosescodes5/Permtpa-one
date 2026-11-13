"use client";
import { MotionConfig } from "motion/react";

/** Honour the OS "reduce motion" setting: transform/layout animations are disabled; gentle fades remain. */
export function MotionProvider({ children }: { children: React.ReactNode }) {
  return <MotionConfig reducedMotion="user">{children}</MotionConfig>;
}
