import type { Metadata } from "next";
import { Organization } from "@/components/app/organization";
export const metadata: Metadata = { title: "Organization" };
export default function Page() { return <Organization />; }
