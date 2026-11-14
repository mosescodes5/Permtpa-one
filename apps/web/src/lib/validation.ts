import { z } from "zod";

export const emailSchema = z.string().trim().min(1, "Enter your email address").email("Enter a valid email address").max(254);

/** Length-first policy (NIST 800-63B style): long passphrases, no arbitrary composition rules. */
export const passwordSchema = z.string().min(12, "Use at least 12 characters").max(128, "Use at most 128 characters");

export const loginSchema = z.object({ email: emailSchema, password: z.string().min(1, "Enter your password"), mfaCode: z.string().trim().max(32).optional() });

export const registerSchema = z
  .object({
    fullName: z.string().trim().min(2, "Enter your full name").max(100),
    email: emailSchema,
    password: passwordSchema,
    acceptTerms: z.boolean().refine((v) => v, "You must accept the terms to continue"),
  })
  .strict();

export const forgotSchema = z.object({ email: emailSchema });

export type LoginInput = z.infer<typeof loginSchema>;
export type RegisterInput = z.infer<typeof registerSchema>;
export type ForgotInput = z.infer<typeof forgotSchema>;

/** 0–4 heuristic strength meter (UI hint only; the server enforces policy). */
export function passwordStrength(pw: string): 0 | 1 | 2 | 3 | 4 {
  if (!pw) return 0;
  let score = 0;
  if (pw.length >= 12) score++;
  if (pw.length >= 16) score++;
  if (/[a-z]/.test(pw) && /[A-Z]/.test(pw)) score++;
  if (/\d/.test(pw) && /[^A-Za-z0-9]/.test(pw)) score++;
  return Math.min(score, 4) as 0 | 1 | 2 | 3 | 4;
}

export const resetSchema = z.object({ newPassword: passwordSchema, confirm: z.string() }).refine((v) => v.newPassword === v.confirm, { message: "Passwords do not match", path: ["confirm"] });
export type ResetInput = z.infer<typeof resetSchema>;

export const inviteSchema = z.object({ email: emailSchema, role: z.string().min(1, "Choose a role") });
export type InviteInput = z.infer<typeof inviteSchema>;

export const onboardingSchema = z.object({ name: z.string().trim().min(2, "Enter your business name").max(120), businessType: z.string().min(1, "Choose a business type") });
export type OnboardingInput = z.infer<typeof onboardingSchema>;
