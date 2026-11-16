"use client";
/**
 * PERMTPA ONE icon set — original duotone icons (soft fill + stroke), each with its own hover animation.
 * An icon animates when the nearest interactive ancestor (a, button, menuitem, or [data-hover-scope]) is hovered or focused.
 * Motion is gentle and honours prefers-reduced-motion (see <MotionConfig reducedMotion="user"> in the root layout).
 */
import { motion, type Variant, type Variants } from "motion/react";
import { useEffect, useRef, useState } from "react";

export type IconProps = { className?: string };
export type Icon = React.ComponentType<IconProps>;

const fc = { transformBox: "fill-box", transformOrigin: "center" } as const;          // pivot on own centre
const fb = { transformBox: "fill-box", transformOrigin: "50% 100%" } as const;         // pivot on bottom edge
const ft = { transformBox: "fill-box", transformOrigin: "50% 0%" } as const;           // pivot on top edge
const vb = (x: number, y: number) => ({ transformBox: "view-box", transformOrigin: `${x}px ${y}px` }) as const;
const duo = { fill: "currentColor", fillOpacity: 0.14 } as const;                     // the duotone body
const spring = { type: "spring", stiffness: 320, damping: 15 } as const;
const H = (hover: Variant): Variants => ({ hover });                                   // variants helper

function Base({ className, children }: { className?: string; children: React.ReactNode }) {
  const ref = useRef<HTMLSpanElement>(null);
  const [hot, setHot] = useState(false);
  useEffect(() => {
    const host = ref.current?.closest("a,button,[data-hover-scope],[role=menuitem]") ?? ref.current;
    if (!host) return;
    const on = () => setHot(true);
    const off = () => setHot(false);
    const evs: [string, () => void][] = [["pointerenter", on], ["pointerleave", off], ["focusin", on], ["focusout", off]];
    evs.forEach(([e, f]) => host.addEventListener(e, f));
    return () => evs.forEach(([e, f]) => host.removeEventListener(e, f));
  }, []);
  return (
    <motion.span ref={ref} initial="rest" animate={hot ? "hover" : "rest"} className="inline-flex shrink-0">
      <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false" className={className}>
        {children}
      </svg>
    </motion.span>
  );
}
const mk = (children: React.ReactNode): Icon => function AnimatedIcon({ className }) { return <Base className={className}>{children}</Base>; };

/* ---------- navigation & modules ---------- */
export const LayoutDashboard = mk(<>{[[3, 3, 8, 8], [13, 3, 8, 5], [13, 10, 8, 11], [3, 13, 8, 8]].map(([x, y, w, h], i) => (
  <motion.rect key={i} x={x} y={y} width={w} height={h} rx={2} {...duo} style={fc} variants={H({ scale: [1, 0.78, 1.08, 1], transition: { duration: 0.5, delay: i * 0.07 } })} />))}</>);

export const ShoppingCart = mk(<>
  <motion.g variants={H({ x: [0, 2.5, 0], transition: { duration: 0.6 } })}>
    <path d="M6 7.5h14l-1.7 6.8a1.5 1.5 0 0 1-1.5 1.1H8.3a1.5 1.5 0 0 1-1.5-1.2z" {...duo} stroke="none" />
    <path d="M2.5 3.5h2.2l2.1 11.2a1.5 1.5 0 0 0 1.5 1.2h8.3a1.5 1.5 0 0 0 1.5-1.1L20 7.5H6" />
    {[9, 17].map((cx) => <motion.circle key={cx} cx={cx} cy={19.5} r={1.5} strokeDasharray="2.2 1.4" style={fc} variants={H({ rotate: 360, transition: { duration: 0.6 } })} />)}
  </motion.g></>);

export const Box = mk(<>
  <motion.g style={fc} variants={H({ y: [0, -3, 0], rotate: [0, -5, 0], transition: { duration: 0.55 } })}><path d="M3 7.5 12 3l9 4.5-9 4.5z" {...duo} /></motion.g>
  <path d="M3 7.5v9L12 21l9-4.5v-9M12 12v9" /></>);

export const Users = mk(<>
  <motion.g variants={H({ y: [0, -2, 0], transition: { duration: 0.45 } })}><circle cx="9" cy="8" r="3.2" {...duo} /></motion.g>
  <path d="M3.5 20c0-3.3 2.5-5.5 5.5-5.5s5.5 2.2 5.5 5.5" />
  <motion.g variants={H({ y: [0, -2, 0], transition: { duration: 0.45, delay: 0.1 } })}><circle cx="17" cy="9" r="2.4" {...duo} /></motion.g>
  <path d="M15.8 14.6c2.7-.3 4.7 1.3 4.7 4.2" /></>);

export const Truck = mk(<>
  <motion.g variants={H({ x: [0, 2, 0], transition: { duration: 0.6 } })}>
    <path d="M2.5 6.5h11v9.5h-11z" {...duo} /><path d="M13.5 9.5h4l3 3.5V16h-7z" {...duo} />
    {[7, 17].map((cx) => <motion.circle key={cx} cx={cx} cy={17.5} r={2} fill="var(--surface)" strokeDasharray="2.6 1.4" style={fc} variants={H({ rotate: 360, transition: { duration: 0.6 } })} />)}
  </motion.g>
  <motion.path d="M0.5 9h-0M1 11.5h0" strokeWidth={2} variants={{ rest: { opacity: 0 }, hover: { opacity: [0, 1, 0], x: [4, -2, -5], transition: { duration: 0.6 } } }} /></>);

export const Wallet = mk(<>
  <rect x="3" y="7" width="18" height="12.5" rx="3" {...duo} /><path d="M3 10.5h18" /><circle cx="16.5" cy="14.5" r="1.2" fill="currentColor" stroke="none" />
  <motion.circle cx="9" cy="3.8" r="2.4" fill="var(--surface)" variants={{ rest: { y: 0 }, hover: { y: [0, 5, 0], transition: { duration: 0.7 } } }} /></>);

export const BarChart3 = mk(<>
  <path d="M3 21h18" />
  {[[4, 11], [10, 6], [16, 2.5]].map(([x, y], i) => <motion.rect key={x} x={x} y={y! + 3} width="4" height={18 - y! - 3} rx="1" {...duo} style={fb} variants={H({ scaleY: [1, 0.25, 1], transition: { duration: 0.55, delay: i * 0.09 } })} />)}</>);

export const Sparkles = mk(<>
  <motion.path d="M10 3l1.9 5.1L17 10l-5.1 1.9L10 17l-1.9-5.1L3 10l5.1-1.9z" {...duo} style={fc} variants={H({ rotate: 90, scale: 1.15, transition: spring })} />
  <motion.path d="M18.5 3.5l.7 1.8 1.8.7-1.8.7-.7 1.8-.7-1.8-1.8-.7 1.8-.7z" fill="currentColor" style={fc} animate={{ opacity: [0.35, 1, 0.35], scale: [0.85, 1.1, 0.85] }} transition={{ duration: 2.4, repeat: Infinity }} />
  <motion.path d="M17.5 15.5l.55 1.45 1.45.55-1.45.55-.55 1.45-.55-1.45-1.45-.55 1.45-.55z" fill="currentColor" style={fc} animate={{ opacity: [1, 0.3, 1] }} transition={{ duration: 2.4, repeat: Infinity, delay: 0.8 }} /></>);

export const Bell = mk(<>
  <motion.g style={ft} variants={H({ rotate: [0, 14, -12, 8, -4, 0], transition: { duration: 0.7 } })}>
    <path d="M6 9a6 6 0 0 1 12 0c0 6 2.5 7.5 2.5 7.5h-17S6 15 6 9z" {...duo} /></motion.g>
  <motion.path d="M10.3 20a2 2 0 0 0 3.4 0" variants={H({ x: [0, -1, 1, 0], transition: { duration: 0.7 } })} /></>);

export const UserCog = mk(<>
  <circle cx="10" cy="8" r="3.4" {...duo} /><path d="M3.5 20c0-3.6 2.8-6 6.5-6 1.1 0 2.2.2 3.1.6" />
  <circle cx="18" cy="17.5" r="1.7" />
  <motion.circle cx="18" cy="17.5" r="3.5" strokeDasharray="1.7 1.7" style={fc} variants={H({ rotate: 180, transition: { duration: 0.8 } })} /></>);

export const History = mk(<>
  <circle cx="12" cy="12" r="8.5" {...duo} /><motion.path d="M12 7.5V12" style={vb(12, 12)} variants={H({ rotate: 360, transition: { duration: 0.8 } })} />
  <motion.path d="M12 12l3 2" style={vb(12, 12)} variants={H({ rotate: 30, transition: spring })} /></>);

export const Building2 = mk(<>
  <rect x="5" y="3" width="10" height="18" rx="1.5" {...duo} /><path d="M15 9h3.5A1.5 1.5 0 0 1 20 10.5V21M3 21h18" />
  {[6.5, 10.5, 14.5].map((y, i) => <motion.path key={y} d={`M8.5 ${y}h3`} variants={H({ opacity: [1, 0.15, 1], transition: { duration: 0.6, delay: i * 0.12 } })} />)}</>);

export const Settings = mk(<>
  <motion.g style={fc} variants={H({ rotate: 90, transition: spring })}>
    <circle cx="12" cy="12" r="8.6" strokeWidth="3.2" strokeDasharray="2.6 3.84" strokeLinecap="butt" />
    <circle cx="12" cy="12" r="6" {...duo} /><circle cx="12" cy="12" r="2.2" /></motion.g></>);

export const Landmark = mk(<>
  <motion.path d="M3 9.5 12 4l9 5.5z" {...duo} style={fc} variants={H({ y: [0, -2, 0], transition: { duration: 0.5 } })} />
  {[6, 10, 14, 18].map((x, i) => <motion.path key={x} d={`M${x} 12v5`} style={fb} variants={H({ scaleY: [1, 0.4, 1], transition: { duration: 0.45, delay: i * 0.07 } })} />)}
  <path d="M3.5 20h17" /></>);

/* ---------- security & trust ---------- */
export const ShieldCheck = mk(<>
  <path d="M12 3 4.5 6v5.5c0 4.7 3.1 8 7.5 9.5 4.4-1.5 7.5-4.8 7.5-9.5V6z" {...duo} />
  <motion.path d="M8.8 12.2l2.3 2.3 4.2-4.6" initial={{ pathLength: 0 }} whileInView={{ pathLength: 1 }} viewport={{ once: true }} transition={{ duration: 0.6, delay: 0.2 }} variants={H({ pathLength: [0, 1], transition: { duration: 0.5 } })} /></>);

export const ShieldOff = mk(<>
  <path d="M12 3 4.5 6v5.5c0 4.7 3.1 8 7.5 9.5 4.4-1.5 7.5-4.8 7.5-9.5V6z" {...duo} />
  <motion.path d="M4 4l16 16" variants={H({ pathLength: [0, 1], transition: { duration: 0.5 } })} /></>);

export const KeyRound = mk(<>
  <motion.g style={vb(7.5, 15.5)} variants={H({ rotate: [0, -22, 8, 0], transition: { duration: 0.6 } })}>
    <circle cx="7.5" cy="15.5" r="3.8" {...duo} /><path d="M10.2 12.8 19 4M16 7l2.5 2.5M13.5 9.5l2 2" /></motion.g></>);

export const LockKeyhole = mk(<>
  <rect x="4.5" y="10.5" width="15" height="10" rx="2.5" {...duo} /><circle cx="12" cy="15" r="1.3" fill="currentColor" stroke="none" /><path d="M12 16v1.8" />
  <motion.path d="M8 10.5V7.5a4 4 0 0 1 8 0v3" variants={H({ y: [0, -3, 0], transition: { duration: 0.6 } })} /></>);

export const ScrollText = mk(<>
  <rect x="5" y="3" width="14" height="18" rx="2.5" {...duo} />
  {[[8.5, 8, 15.5], [8.5, 12, 15.5], [8.5, 16, 12.5]].map(([x1, y, x2], i) => <motion.path key={y} d={`M${x1} ${y}H${x2}`} variants={H({ pathLength: [0, 1], transition: { duration: 0.4, delay: i * 0.12 } })} />)}</>);

export const Fingerprint = mk(<>
  {["M6 10a6 6 0 0 1 12 0v1.5", "M9 21c.5-2 .8-4 .8-6.5a2.2 2.2 0 0 1 4.4 0c0 2.5-.4 4.8-1.2 6.5", "M3.5 12a8.5 8.5 0 0 1 17 0v1", "M18 14.5c0 2.2-.3 4-1 5.5"].map((d, i) => (
    <motion.path key={d} d={d} variants={H({ pathLength: [0, 1], transition: { duration: 0.5, delay: i * 0.1 } })} />))}</>);

export const ClipboardList = mk(<>
  <rect x="5" y="4.5" width="14" height="16.5" rx="2.5" {...duo} />
  <motion.rect x="9" y="2.5" width="6" height="4" rx="1.5" fill="var(--surface)" variants={H({ y: [0, -1.5, 0], transition: { duration: 0.4 } })} />
  <path d="M8.5 11h7M8.5 15h4.5" /></>);

/* ---------- arrows, marks & feedback ---------- */
export const ArrowRight = mk(<motion.g variants={H({ x: 3, transition: spring })}><path d="M4 12h15M13 6l6 6-6 6" /></motion.g>);
export const ArrowUpRight = mk(<motion.g variants={H({ x: 1.5, y: -1.5, transition: spring })}><path d="M7 17 17 7M8 7h9v9" /></motion.g>);
export const ArrowDownRight = mk(<motion.g variants={H({ x: 1.5, y: 1.5, transition: spring })}><path d="M7 7l10 10M17 8v9H8" /></motion.g>);
export const CornerDownLeft = mk(<motion.g variants={H({ x: -2, transition: spring })}><path d="M19 5v8a3 3 0 0 1-3 3H5M9 12l-4 4 4 4" /></motion.g>);
export const Check = mk(<motion.path d="M4.5 12.5l4.5 4.5L19.5 7" initial={{ pathLength: 0 }} whileInView={{ pathLength: 1 }} viewport={{ once: true }} transition={{ duration: 0.5 }} />);

export const CheckCircle2 = mk(<>
  <motion.circle cx="12" cy="12" r="9" {...duo} initial={{ pathLength: 0 }} animate={{ pathLength: 1 }} transition={{ duration: 0.5 }} />
  <motion.path d="M8 12.3l2.8 2.8L16.3 9.5" initial={{ pathLength: 0 }} animate={{ pathLength: 1 }} transition={{ duration: 0.4, delay: 0.3 }} /></>);
export const TriangleAlert = mk(<>
  <motion.g style={fb} initial={{ rotate: 0 }} animate={{ rotate: [0, -7, 7, -4, 0] }} transition={{ duration: 0.6, delay: 0.1 }}>
    <path d="M12 3.5 2.8 19.5h18.4z" {...duo} /><path d="M12 9.5v4.5" /><circle cx="12" cy="17" r="0.6" fill="currentColor" /></motion.g></>);
export const AlertTriangle = TriangleAlert;
export const Info = mk(<>
  <motion.g style={fc} initial={{ scale: 0.6, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={spring}>
    <circle cx="12" cy="12" r="9" {...duo} /><path d="M12 11v5" /><circle cx="12" cy="7.8" r="0.7" fill="currentColor" /></motion.g></>);
export const MailCheck = mk(<>
  <rect x="3" y="5.5" width="18" height="13" rx="2.5" {...duo} /><path d="m3.5 7.5 8.5 6 8.5-6" />
  <motion.path d="M9 16.2l2 2 4-4" stroke="var(--accent)" initial={{ pathLength: 0 }} animate={{ pathLength: 1 }} transition={{ duration: 0.5, delay: 0.3 }} /></>);
export const Hourglass = mk(<motion.g style={fc} variants={H({ rotate: 180, transition: { duration: 0.6 } })}>
  <path d="M6.5 3.5h11M6.5 20.5h11M7.5 3.5c0 4 4.5 5.5 4.5 8.5s-4.5 4.5-4.5 8.5M16.5 3.5c0 4-4.5 5.5-4.5 8.5s4.5 4.5 4.5 8.5" {...duo} /></motion.g>);

/* ---------- interface ---------- */
export const Menu = mk(<>{[7, 12, 17].map((y, i) => <motion.path key={y} d={`M4 ${y}h16`} variants={H({ x: [0, 3, 0], transition: { duration: 0.4, delay: i * 0.06 } })} />)}</>);
export const X = mk(<motion.g style={fc} variants={H({ rotate: 90, transition: spring })}><path d="M6 6l12 12M18 6 6 18" /></motion.g>);
export function MenuToggleIcon({ open, className }: { open: boolean; className?: string }) {
  const t = { type: "spring", stiffness: 300, damping: 22 } as const;
  return (
    <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" aria-hidden="true" className={className}>
      <motion.path d="M4 7h16" style={fc} animate={open ? { y: 5, rotate: 45 } : { y: 0, rotate: 0 }} transition={t} />
      <motion.path d="M4 12h16" style={fc} animate={open ? { opacity: 0, scaleX: 0 } : { opacity: 1, scaleX: 1 }} transition={t} />
      <motion.path d="M4 17h16" style={fc} animate={open ? { y: -5, rotate: -45 } : { y: 0, rotate: 0 }} transition={t} />
    </svg>
  );
}
export const Search = mk(<motion.g style={vb(20, 20)} variants={H({ rotate: [0, -14, 8, 0], transition: { duration: 0.55 } })}><circle cx="10.5" cy="10.5" r="6.5" {...duo} /><path d="m15.5 15.5 5 5" /></motion.g>);
export const Sun = mk(<>
  <circle cx="12" cy="12" r="4" {...duo} />
  <motion.g style={fc} variants={H({ rotate: 90, transition: { duration: 0.7 } })}><path d="M12 2.5v2.2M12 19.3v2.2M2.5 12h2.2M19.3 12h2.2M5.3 5.3l1.6 1.6M17.1 17.1l1.6 1.6M5.3 18.7l1.6-1.6M17.1 6.9l1.6-1.6" /></motion.g></>);
export const Moon = mk(<>
  <motion.path d="M20 14.2A8 8 0 1 1 9.8 4a6.5 6.5 0 0 0 10.2 10.2z" {...duo} style={fc} variants={H({ rotate: -18, transition: spring })} />
  <motion.path d="M17 4.5v2M16 5.5h2" animate={{ opacity: [0.3, 1, 0.3] }} transition={{ duration: 2.2, repeat: Infinity }} /></>);
export const LogOut = mk(<><path d="M9.5 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h3.5" {...duo} /><motion.g variants={H({ x: 3, transition: spring })}><path d="M15 8l4 4-4 4M19 12H9.5" /></motion.g></>);
export const Eye = mk(<><path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z" {...duo} /><motion.circle cx="12" cy="12" r="2.8" variants={H({ x: [0, -1.4, 1.4, 0], transition: { duration: 0.7 } })} /></>);
export const EyeOff = mk(<><path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z" {...duo} /><circle cx="12" cy="12" r="2.8" /><motion.path d="M4 4l16 16" variants={H({ pathLength: [0.2, 1], transition: { duration: 0.4 } })} /></>);
export const ChevronsUpDown = mk(<><motion.path d="M7 9.5 12 5l5 4.5" variants={H({ y: -1.5, transition: spring })} /><motion.path d="M7 14.5 12 19l5-4.5" variants={H({ y: 1.5, transition: spring })} /></>);
export const Laptop = mk(<><rect x="4.5" y="5" width="15" height="10.5" rx="2" {...duo} /><path d="M2.5 19h19" /></>);
export const Smartphone = mk(<><rect x="6.5" y="2.5" width="11" height="19" rx="2.5" {...duo} /><path d="M11 18.5h2" /></>);
export const Receipt = mk(<><path d="M5.5 3.5h13V21l-2.2-1.5L14 21l-2-1.5L10 21l-2.3-1.5L5.5 21z" {...duo} />{[8, 12].map((y, i) => <motion.path key={y} d={`M9 ${y}h6`} variants={H({ pathLength: [0, 1], transition: { duration: 0.4, delay: i * 0.1 } })} />)}</>);
export const PanelLeftClose = mk(<><rect x="3.5" y="4.5" width="17" height="15" rx="2.5" {...duo} /><path d="M9.5 4.5v15" /><motion.path d="M16 10l-2 2 2 2" variants={H({ x: -1.5, transition: spring })} /></>);
export const PanelLeftOpen = mk(<><rect x="3.5" y="4.5" width="17" height="15" rx="2.5" {...duo} /><path d="M9.5 4.5v15" /><motion.path d="M14 10l2 2-2 2" variants={H({ x: 1.5, transition: spring })} /></>);
/** Static spinner — callers add `animate-spin`. */
export const Loader2 = function Loader2({ className }: IconProps) {
  return <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" aria-hidden="true" className={className}><circle cx="12" cy="12" r="9" strokeOpacity="0.2" /><path d="M21 12a9 9 0 0 0-9-9" /></svg>;
};

/* ---------- inventory additions ---------- */
export const Plus = mk(<motion.g style={fc} variants={H({ rotate: 90, transition: spring })}><path d="M12 5v14M5 12h14" /></motion.g>);
export const ChevronLeft = mk(<motion.path d="M15 5l-7 7 7 7" variants={H({ x: -2, transition: spring })} />);
export const ChevronRight = mk(<motion.path d="M9 5l7 7-7 7" variants={H({ x: 2, transition: spring })} />);
export const Archive = mk(<>
  <rect x="3" y="3.5" width="18" height="5" rx="1.5" {...duo} /><path d="M5 8.5V19a1.5 1.5 0 0 0 1.5 1.5h11A1.5 1.5 0 0 0 19 19V8.5" />
  <motion.path d="M9.5 13h5" variants={H({ pathLength: [0, 1], transition: { duration: 0.4 } })} /></>);
export const Pencil = mk(<motion.g style={vb(5, 19)} variants={H({ rotate: [0, -10, 6, 0], transition: { duration: 0.5 } })}><path d="M4 20l1-4L16.5 4.5a2 2 0 0 1 3 3L8 19z" {...duo} /><path d="M14.5 6.5l3 3" /></motion.g>);
