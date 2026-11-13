"use client";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useState } from "react";
import { useForm, useWatch } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { CheckCircle2, Eye, EyeOff, Loader2, MailCheck, TriangleAlert } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { FieldError, Input, Label } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { ApiProblem, BUSINESS_TYPES, api, problemMessage, setCsrfToken } from "@/lib/api";
import { forgotSchema, loginSchema, onboardingSchema, passwordStrength, registerSchema, resetSchema, type ForgotInput, type LoginInput, type OnboardingInput, type RegisterInput, type ResetInput } from "@/lib/validation";

/** Only same-site relative paths are honoured — prevents open-redirect abuse of `?next=`. */
export function safeNext(raw: string | null): string | null {
  if (!raw || !raw.startsWith("/") || raw.startsWith("//") || raw.startsWith("/\\")) return null;
  return raw;
}

function Banner({ tone, children }: { tone: "error" | "success"; children: React.ReactNode }) {
  const Icon = tone === "error" ? TriangleAlert : CheckCircle2;
  return (
    <div role={tone === "error" ? "alert" : "status"} className={cn("flex gap-2.5 rounded-lg border p-3.5 text-sm", tone === "error" ? "border-danger/30 bg-danger-soft text-danger" : "border-accent/30 bg-accent-soft text-accent")}>
      <Icon className="mt-0.5 size-4 shrink-0" /><div>{children}</div>
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
  const router = useRouter();
  const params = useSearchParams();
  const [error, setError] = useState<string | null>(null);
  const [needMfa, setNeedMfa] = useState(false);
  const [unverified, setUnverified] = useState(false);
  const [resent, setResent] = useState(false);
  const f = useForm<LoginInput & { mfaCode?: string }>({ resolver: zodResolver(loginSchema), defaultValues: { email: "", password: "", mfaCode: "" } });
  const { errors, isSubmitting } = f.formState;

  async function onSubmit(v: LoginInput & { mfaCode?: string }) {
    setError(null); setUnverified(false);
    try {
      const r = await api<{ csrfToken: string }>("/v1/auth/login", { method: "POST", body: { email: v.email, password: v.password, ...(needMfa && v.mfaCode ? { mfaCode: v.mfaCode } : {}) } });
      setCsrfToken(r.csrfToken);
      router.replace(safeNext(params.get("next")) ?? "/dashboard");
    } catch (e) {
      if (e instanceof ApiProblem && e.code === "mfa_required") { setNeedMfa(true); return; }
      if (e instanceof ApiProblem && e.code === "email_not_verified") { setUnverified(true); return; }
      setError(problemMessage(e));
    }
  }

  async function resend() {
    try { await api("/v1/auth/resend-verification", { method: "POST", body: { email: f.getValues("email") } }); setResent(true); } catch (e) { setError(problemMessage(e)); }
  }

  return (
    <form noValidate onSubmit={f.handleSubmit(onSubmit)} className="space-y-4">
      {error && <Banner tone="error">{error}</Banner>}
      {unverified && (
        <Banner tone="error">
          Please verify your email address before logging in.{" "}
          {resent ? <span className="font-medium">A new link is on its way.</span> : <button type="button" onClick={resend} className="cursor-pointer font-medium underline">Resend verification email</button>}
        </Banner>
      )}
      <div><Label htmlFor="email">Email</Label><Input id="email" type="email" autoComplete="email" placeholder="you@business.com" aria-invalid={!!errors.email} {...f.register("email")} /><FieldError>{errors.email?.message}</FieldError></div>
      <PasswordField id="password" label="Password" autoComplete="current-password" error={errors.password?.message} registerProps={f.register("password")} />
      {needMfa && (
        <div>
          <Label htmlFor="mfaCode">Verification code</Label>
          <Input id="mfaCode" inputMode="numeric" autoComplete="one-time-code" autoFocus placeholder="6-digit code or recovery code" {...f.register("mfaCode")} />
          <p className="mt-1.5 text-xs text-subtle">Enter the code from your authenticator app, or one of your recovery codes.</p>
        </div>
      )}
      <div className="flex justify-end"><Link href="/forgot-password" className="text-sm text-accent hover:underline">Forgot password?</Link></div>
      <Submit loading={isSubmitting}>{needMfa ? "Verify and log in" : "Log in"}</Submit>
    </form>
  );
}

export function RegisterForm() {
  const [error, setError] = useState<string | null>(null);
  const [sentTo, setSentTo] = useState<string | null>(null);
  const f = useForm<RegisterInput>({ resolver: zodResolver(registerSchema), defaultValues: { fullName: "", email: "", password: "", acceptTerms: false } });
  const { errors, isSubmitting } = f.formState;
  const pw = useWatch({ control: f.control, name: "password" });

  async function onSubmit(v: RegisterInput) {
    setError(null);
    try {
      await api("/v1/auth/register", { method: "POST", body: { fullName: v.fullName, email: v.email, password: v.password } });
      setSentTo(v.email);
    } catch (e) { setError(problemMessage(e)); }
  }

  if (sentTo) {
    return (
      <div role="status" className="rounded-2xl border border-border-strong bg-surface p-6 text-center shadow-sm">
        <span className="mx-auto flex size-12 items-center justify-center rounded-full bg-accent-soft text-accent"><MailCheck className="size-6" /></span>
        <h2 className="mt-4 text-lg font-semibold">Check your email</h2>
        <p className="mt-2 text-sm text-muted">If <strong className="font-medium text-fg">{sentTo}</strong> can be registered, we have sent a verification link. It expires in 24 hours.</p>
        <Link href="/login" className="mt-5 inline-block text-sm font-medium text-accent hover:underline">Continue to log in</Link>
      </div>
    );
  }
  return (
    <form noValidate onSubmit={f.handleSubmit(onSubmit)} className="space-y-4">
      {error && <Banner tone="error">{error}</Banner>}
      <div><Label htmlFor="fullName">Full name</Label><Input id="fullName" autoComplete="name" aria-invalid={!!errors.fullName} {...f.register("fullName")} /><FieldError>{errors.fullName?.message}</FieldError></div>
      <div><Label htmlFor="email">Work email</Label><Input id="email" type="email" autoComplete="email" aria-invalid={!!errors.email} {...f.register("email")} /><FieldError>{errors.email?.message}</FieldError></div>
      <PasswordField id="password" label="Password" autoComplete="new-password" showMeter value={pw} error={errors.password?.message} registerProps={f.register("password")} />
      <div>
        <label className="flex cursor-pointer items-start gap-2.5 text-sm text-muted"><input type="checkbox" className="mt-0.5 size-4 accent-[var(--accent)]" {...f.register("acceptTerms")} /><span>I agree to the <Link href="/terms" className="text-accent hover:underline">Terms</Link> and <Link href="/privacy" className="text-accent hover:underline">Privacy Policy</Link>.</span></label>
        <FieldError>{errors.acceptTerms?.message}</FieldError>
      </div>
      <Submit loading={isSubmitting}>Create account</Submit>
    </form>
  );
}

export function ForgotForm() {
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const f = useForm<ForgotInput>({ resolver: zodResolver(forgotSchema), defaultValues: { email: "" } });
  const { errors, isSubmitting } = f.formState;
  if (sent) return <Banner tone="success">If an account exists for that address, we have sent a password reset link. It expires in 24 hours.</Banner>;
  return (
    <form noValidate onSubmit={f.handleSubmit(async (v) => { setError(null); try { await api("/v1/auth/forgot-password", { method: "POST", body: v }); setSent(true); } catch (e) { setError(problemMessage(e)); } })} className="space-y-4">
      {error && <Banner tone="error">{error}</Banner>}
      <div><Label htmlFor="email">Email</Label><Input id="email" type="email" autoComplete="email" aria-invalid={!!errors.email} {...f.register("email")} /><FieldError>{errors.email?.message}</FieldError></div>
      <Submit loading={isSubmitting}>Send reset link</Submit>
    </form>
  );
}

export function ResetForm() {
  const token = useSearchParams().get("token") ?? "";
  const router = useRouter();
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState(false);
  const f = useForm<ResetInput>({ resolver: zodResolver(resetSchema), defaultValues: { newPassword: "", confirm: "" } });
  const { errors, isSubmitting } = f.formState;
  const pw = useWatch({ control: f.control, name: "newPassword" });
  if (!token) return <Banner tone="error">This reset link is incomplete. Request a new one from the <Link href="/forgot-password" className="underline">forgot password</Link> page.</Banner>;
  if (done) return <div className="space-y-4"><Banner tone="success">Your password has been changed and all other sessions were signed out.</Banner><Button variant="navy" size="lg" className="w-full" onClick={() => router.replace("/login")}>Continue to log in</Button></div>;
  return (
    <form noValidate onSubmit={f.handleSubmit(async (v) => { setError(null); try { await api("/v1/auth/reset-password", { method: "POST", body: { token, newPassword: v.newPassword } }); setCsrfToken(null); setDone(true); } catch (e) { setError(problemMessage(e)); } })} className="space-y-4">
      {error && <Banner tone="error">{error}</Banner>}
      <PasswordField id="newPassword" label="New password" autoComplete="new-password" showMeter value={pw} error={errors.newPassword?.message} registerProps={f.register("newPassword")} />
      <PasswordField id="confirm" label="Confirm new password" autoComplete="new-password" error={errors.confirm?.message} registerProps={f.register("confirm")} />
      <Submit loading={isSubmitting}>Change password</Submit>
    </form>
  );
}

export function OnboardingForm({ onCreated }: { onCreated: () => Promise<void> | void }) {
  const [error, setError] = useState<string | null>(null);
  const f = useForm<OnboardingInput>({ resolver: zodResolver(onboardingSchema), defaultValues: { name: "", businessType: "" } });
  const { errors, isSubmitting } = f.formState;
  return (
    <form noValidate onSubmit={f.handleSubmit(async (v) => { setError(null); try { await api("/v1/organizations", { method: "POST", body: v }); await onCreated(); } catch (e) { setError(problemMessage(e)); } })} className="space-y-4">
      {error && <Banner tone="error">{error}</Banner>}
      <div><Label htmlFor="name">Business name</Label><Input id="name" autoComplete="organization" aria-invalid={!!errors.name} {...f.register("name")} /><FieldError>{errors.name?.message}</FieldError></div>
      <div>
        <Label htmlFor="businessType">What kind of business is it?</Label>
        <select id="businessType" aria-invalid={!!errors.businessType} className="h-11 w-full cursor-pointer rounded-lg border border-border-strong bg-surface px-3 text-sm shadow-sm focus-visible:border-accent focus-visible:outline-none" {...f.register("businessType")}>
          <option value="">Select…</option>
          {BUSINESS_TYPES.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
        </select>
        <FieldError>{errors.businessType?.message}</FieldError>
      </div>
      <Submit loading={isSubmitting}>Create workspace</Submit>
    </form>
  );
}

export { Banner };
