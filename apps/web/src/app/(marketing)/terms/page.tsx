import type { Metadata } from "next";
import { StaticPage } from "@/components/marketing/static-page";

export const metadata: Metadata = { title: "Terms of service" };
export default function Page() {
  return (
    <StaticPage title="Terms of service" intro="The terms that govern use of PERMTPA ONE." draft>
      <p>The full terms will be published here.</p>
    </StaticPage>
  );
}
