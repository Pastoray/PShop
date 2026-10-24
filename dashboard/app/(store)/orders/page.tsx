import type { Metadata } from "next";
import { Suspense } from "react";
import { OrderList } from "@/components/store/order-list";

export const metadata: Metadata = { title: "Orders" };

export default function OrdersPage() {
  return (
    <Suspense fallback={null}>
      <OrderList />
    </Suspense>
  );
}
