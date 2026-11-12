import type { Metadata } from "next";
import { StaticPage } from "@/components/marketing/static-page";

export const metadata: Metadata = { title: "Contact" };
export default function Page() {
  return (
    <StaticPage title="Contact" intro="Talk to the PERMTPA team about PERMTPA ONE.">
      <p>A contact form and support channels will be published before launch.</p>
    </StaticPage>
  );
}
