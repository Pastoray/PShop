import type { Metadata } from "next";
import { CheckoutView } from "@/components/store/checkout-view";

export const metadata: Metadata = { title: "Check out" };
export default function CheckoutPage() {
  return <CheckoutView />;
}
