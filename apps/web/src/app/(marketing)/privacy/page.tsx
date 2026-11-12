import type { Metadata } from "next";
import { StaticPage } from "@/components/marketing/static-page";

export const metadata: Metadata = { title: "Privacy policy" };
export default function Page() {
  return (
    <StaticPage title="Privacy policy" intro="How PERMTPA ONE collects, uses and protects information." draft>
      <p>The full policy will be published here.</p>
    </StaticPage>
  );
}
