import type { Metadata } from "next";
import { Team } from "@/components/app/team";
export const metadata: Metadata = { title: "Team" };
export default function Page() { return <Team />; }
