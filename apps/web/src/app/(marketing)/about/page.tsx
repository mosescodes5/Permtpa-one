import type { Metadata } from "next";
import { StaticPage } from "@/components/marketing/static-page";

export const metadata: Metadata = { title: "About PERMTPA" };
export default function Page() {
  return (
    <StaticPage title="About PERMTPA" intro="PERMTPA builds PERMTPA ONE, the business operating system that helps small and medium-sized businesses run with clarity and control.">
      <p>PERMTPA ONE is a technology product by PERMTPA, designed for retail, agriculture, restaurants, distribution, services and agencies.</p>
    </StaticPage>
  );
}
