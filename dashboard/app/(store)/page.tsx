import { Suspense } from "react";
import { CatalogueView } from "@/components/store/catalogue-view";

export default function HomePage() {
  return (
    <Suspense fallback={null}>
      <CatalogueView />
    </Suspense>
  );
}
