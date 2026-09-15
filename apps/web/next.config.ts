import type { NextConfig } from "next";
import createNextIntlPlugin from "next-intl/plugin";

const withNextIntl = createNextIntlPlugin("./src/i18n/request.ts");

const nextConfig: NextConfig = {
  // Standalone output keeps the runtime image to the server bundle plus its traced deps.
  output: "standalone",
  reactStrictMode: true,
  // In compose, Caddy routes /api/* straight to the Rust API (so SSE never touches Next).
  // This rewrite is the dev-server equivalent, which keeps `pnpm dev` a single origin.
  async rewrites() {
    const api = process.env.API_INTERNAL_URL ?? "http://localhost:8080";
    return [{ source: "/api/:path*", destination: `${api}/api/:path*` }];
  },
};

export default withNextIntl(nextConfig);
