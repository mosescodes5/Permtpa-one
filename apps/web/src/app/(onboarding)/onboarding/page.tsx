"use client";
import { useRouter } from "next/navigation";
import { AuthShell } from "@/components/auth/auth-shell";
import { AuthGate, useAuth } from "@/components/auth/auth-provider";
import { OnboardingForm } from "@/components/auth/forms";

export default function Page() {
  const { me, refresh } = useAuth();
  const router = useRouter();
  return (
    <AuthGate>
      <AuthShell title={`Welcome${me ? `, ${me.user.fullName.split(" ")[0]}` : ""}`} subtitle="Set up your workspace. You will be its Owner." footer={<>You can add teammates and more businesses later.</>}>
        <OnboardingForm onCreated={async () => { await refresh(); router.replace("/dashboard"); }} />
      </AuthShell>
    </AuthGate>
  );
}
