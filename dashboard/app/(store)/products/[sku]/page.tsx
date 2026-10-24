import { ProductView } from "@/components/store/product-view";

export default async function ProductPage({ params }: { params: Promise<{ sku: string }> }) {
  const { sku } = await params;
  return <ProductView sku={sku} />;
}
