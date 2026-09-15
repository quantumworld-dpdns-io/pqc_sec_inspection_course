import { getTranslations, setRequestLocale } from "next-intl/server";

import { Nav } from "@/components/nav";
import { Card } from "@/components/ui";
import { apiFetch } from "@/lib/api";
import type { Endpoint } from "@/lib/types";

// Lab data is live: every render asks the API rather than baking a snapshot at build time
// (which would also make the image build depend on a running backend).
export const dynamic = "force-dynamic";


export default async function EnvironmentPage({
  params,
}: {
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  setRequestLocale(locale);
  const t = await getTranslations("environment");
  const endpoints = await apiFetch<Endpoint[]>("/endpoints");

  return (
    <>
      <Nav active="environment" />
      <main id="main" className="mx-auto max-w-7xl px-6 py-10">
        <h1 className="text-2xl font-semibold text-slate-900">{t("title")}</h1>
        <p className="mt-1 text-slate-600">{t("subtitle")}</p>

        <Card className="mt-6">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-200 text-left text-slate-500">
                <th className="px-3 py-2 font-medium">{t("label")}</th>
                <th className="px-3 py-2 font-medium">{t("host")}</th>
                <th className="px-3 py-2 font-medium">{t("port")}</th>
                <th className="px-3 py-2 font-medium">{t("notes")}</th>
                <th className="px-3 py-2 font-medium">{t("state")}</th>
              </tr>
            </thead>
            <tbody>
              {endpoints.map((endpoint) => (
                <tr key={endpoint.id} className="border-b border-slate-100 last:border-b-0">
                  <td className="px-3 py-3 font-medium text-slate-900">
                    {locale === "zh-TW" ? endpoint.labelZh : endpoint.label}
                  </td>
                  <td className="px-3 py-3 font-mono text-slate-700">{endpoint.host}</td>
                  <td className="px-3 py-3 font-mono text-slate-700">{endpoint.port}</td>
                  <td className="px-3 py-3 text-slate-600">{endpoint.notes ?? "—"}</td>
                  <td className="px-3 py-3">
                    {endpoint.enabled ? t("enabled") : t("disabled")}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </Card>

        <Card title={t("adapters")} className="mt-6">
          <p className="text-sm text-slate-600">{t("adaptersBody")}</p>
          <ul className="mt-3 grid gap-2 text-sm font-mono text-slate-700 sm:grid-cols-2 lg:grid-cols-4">
            {["openssl", "boringssl", "wolfssl", "go"].map((adapter) => (
              <li key={adapter} className="rounded border border-slate-200 px-3 py-2">
                adapter-{adapter}
              </li>
            ))}
          </ul>
        </Card>
      </main>
    </>
  );
}
