import type { NextConfig } from "next";

const apiOrigin = process.env.API_ORIGIN; // e.g. http://localhost:8080 for `npm run dev`

const config: NextConfig = {
  // Emits a minimal self-contained server for the Docker image (see docker/Dockerfile.dashboard).
  output: "standalone",
  reactStrictMode: true,
  poweredByHeader: false,
  // In compose/Kubernetes the ingress routes /v1/* and /ws before the request reaches Next.
  // Only for local development (`next dev` on :3000) do we proxy the API ourselves.
  async rewrites() {
    return apiOrigin ? [{ source: "/v1/:path*", destination: `${apiOrigin}/v1/:path*` }] : [];
  },
};

export default config;
