import type { Metadata } from "next";
import { Inventory } from "@/components/app/inventory";
export const metadata: Metadata = { title: "Inventory" };
export default function Page() { return <Inventory />; }
