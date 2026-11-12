"use client";
import Link from "next/link";
import { useState } from "react";
import { useForm, useWatch } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { Eye, EyeOff, Info, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { FieldError, Input, Label } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { forgotSchema, loginSchema, passwordStrength, registerSchema, type ForgotInput, type LoginInput, type RegisterInput } from "@/lib/validation";

/** Milestone 1 has no auth API yet. After client-side validation we say so — we never pretend to sign in. */
function NotConnected() {
  return (
    <div role="status" className="mt-4 flex gap-2.5 rounded-lg border border-border-strong bg-bg-subtle p-3.5 text-sm text-muted">
      <Info className="mt-0.5 size-4 shrink-0 text-accent" />
      <p>Your details are valid, but the PERMTPA ONE authentication service is not connected yet. It ships in Milestone 2 — nothing was sent or stored.</p>
    </div>
  );
}

function PasswordField({ id, label, error, autoComplete, registerProps, showMeter, value }: { id: string; label: string; error?: string; autoComplete: string; registerProps: object; showMeter?: boolean; value?: string }) {
  const [show, setShow] = useState(false);
  const strength = passwordStrength(value ?? "");
  return (
    <div>
      <Label htmlFor={id}>{label}</Label>
      <div className="relative">
        <Input id={id} type={show ? "text" : "password"} autoComplete={autoComplete} aria-invalid={!!error} aria-describedby={error ? `${id}-err` : undefined} className="pr-11" {...registerProps} />
        <button type="button" aria-label={show ? "Hide password" : "Show password"} onClick={() => setShow(!show)} className="absolute right-2 top-1/2 -translate-y-1/2 cursor-pointer rounded-md p-1.5 text-subtle hover:text-fg">
          {show ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
        </button>
      </div>
      {showMeter && (
        <div className="mt-2 flex gap-1" aria-hidden>
          {[1, 2, 3, 4].map((i) => <span key={i} className={cn("h-1 flex-1 rounded-full bg-border-strong transition-colors", strength >= i && (strength <= 1 ? "bg-danger" : strength === 2 ? "bg-warning" : "bg-accent"))} />)}
        </div>
      )}
      <FieldError id={`${id}-err`}>{error}</FieldError>
    </div>
  );
}

function Submit({ loading, children }: { loading: boolean; children: React.ReactNode }) {
  return <Button type="submit" variant="navy" size="lg" className="w-full" disabled={loading}>{loading && <Loader2 className="animate-spin" />}{children}</Button>;
}

export function LoginForm() {
  const [done, setDone] = useState(false);
  const f = useForm<LoginInput>({ resolver: zodResolver(loginSchema), defaultValues: { email: "", password: "" } });
  const { errors, isSubmitting } = f.formState;
  return (
    <form noValidate onSubmit={f.handleSubmit(async () => setDone(true))} className="space-y-4">
      <div><Label htmlFor="email">Email</Label><Input id="email" type="email" autoComplete="email" placeholder="you@business.com" aria-invalid={!!errors.email} aria-describedby={errors.email ? "email-err" : undefined} {...f.register("email")} /><FieldError id="email-err">{errors.email?.message}</FieldError></div>
      <PasswordField id="password" label="Password" autoComplete="current-password" error={errors.password?.message} registerProps={f.register("password")} />
      <div className="flex justify-end"><Link href="/forgot-password" className="text-sm text-accent hover:underline">Forgot password?</Link></div>
      <Submit loading={isSubmitting}>Log in</Submit>
      {done && <NotConnected />}
    </form>
  );
}

export function RegisterForm() {
  const [done, setDone] = useState(false);
  const f = useForm<RegisterInput>({ resolver: zodResolver(registerSchema), defaultValues: { fullName: "", businessName: "", email: "", password: "", acceptTerms: false } });
  const { errors, isSubmitting } = f.formState;
  const pw = useWatch({ control: f.control, name: "password" });
  return (
    <form noValidate onSubmit={f.handleSubmit(async () => setDone(true))} className="space-y-4">
      <div><Label htmlFor="fullName">Full name</Label><Input id="fullName" autoComplete="name" aria-invalid={!!errors.fullName} {...f.register("fullName")} /><FieldError>{errors.fullName?.message}</FieldError></div>
      <div><Label htmlFor="businessName">Business name</Label><Input id="businessName" autoComplete="organization" aria-invalid={!!errors.businessName} {...f.register("businessName")} /><FieldError>{errors.businessName?.message}</FieldError></div>
      <div><Label htmlFor="email">Work email</Label><Input id="email" type="email" autoComplete="email" aria-invalid={!!errors.email} {...f.register("email")} /><FieldError>{errors.email?.message}</FieldError></div>
      <PasswordField id="password" label="Password" autoComplete="new-password" showMeter value={pw} error={errors.password?.message} registerProps={f.register("password")} />
      <div>
        <label className="flex cursor-pointer items-start gap-2.5 text-sm text-muted"><input type="checkbox" className="mt-0.5 size-4 accent-[var(--accent)]" {...f.register("acceptTerms")} /><span>I agree to the <Link href="/terms" className="text-accent hover:underline">Terms</Link> and <Link href="/privacy" className="text-accent hover:underline">Privacy Policy</Link>.</span></label>
        <FieldError>{errors.acceptTerms?.message}</FieldError>
      </div>
      <Submit loading={isSubmitting}>Create account</Submit>
      {done && <NotConnected />}
    </form>
  );
}

export function ForgotForm() {
  const [done, setDone] = useState(false);
  const f = useForm<ForgotInput>({ resolver: zodResolver(forgotSchema), defaultValues: { email: "" } });
  const { errors, isSubmitting } = f.formState;
  return (
    <form noValidate onSubmit={f.handleSubmit(async () => setDone(true))} className="space-y-4">
      <div><Label htmlFor="email">Email</Label><Input id="email" type="email" autoComplete="email" aria-invalid={!!errors.email} {...f.register("email")} /><FieldError>{errors.email?.message}</FieldError></div>
      <Submit loading={isSubmitting}>Send reset link</Submit>
      {done && <NotConnected />}
    </form>
  );
}
