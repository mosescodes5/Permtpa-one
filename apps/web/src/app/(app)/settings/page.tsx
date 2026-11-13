import type { Metadata } from "next";
import { Settings } from "@/components/app/settings";

export const metadata: Metadata = { title: "Settings" };
export default function Page() { return <Settings />; }
