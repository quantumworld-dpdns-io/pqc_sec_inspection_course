import { notFound } from "next/navigation";
import { getTranslations, setRequestLocale } from "next-intl/server";

import { CavpRegistration } from "@/components/cavp-registration";
import { CavpWorkbench } from "@/components/cavp-workbench";
import { Nav } from "@/components/nav";
import { apiFetch } from "@/lib/api";
import type { CavpPack, CavpPackSummary, Capability } from "@/lib/types";

// Lab data is live: every render asks the API rather than baking a snapshot at build time
// (which would also make the image build depend on a running backend).
export const dynamic = "force-dynamic";


const CAPABILITIES: Capability[] = ["key_gen", "sig_gen", "sig_ver"];

export function generateStaticParams() {
  return CAPABILITIES.map((capability) => ({ capability }));
}

export default async function CavpPage({
  params,
}: {
  params: Promise<{ locale: string; capability: string }>;
}) {
  const { locale, capability } = await params;
  setRequestLocale(locale);

  if (!CAPABILITIES.includes(capability as Capability)) {
    notFound();
  }
  const active = capability as Capability;
  const t = await getTranslations("cavp");

  const packs = await apiFetch<CavpPackSummary[]>("/cavp/packs", { revalidate: 60 });
  const summary = packs.find((entry) => entry.capability === active);
  if (!summary) {
    notFound();
  }

  const pack = await apiFetch<CavpPack>(`/cavp/packs/${summary.slug}`, { revalidate: 60 });
  const caseCount = pack.prompt.testGroups[0]?.tests.length ?? 0;

  return (
    <div data-module="cavp" data-capability={active} className="min-h-screen">
      <Nav active="cavp" />
      <main id="main" className="mx-auto max-w-5xl px-6 py-10">
        <header className="mb-10 flex items-center justify-between gap-6 rounded-lg bg-white/70 px-6 py-4 shadow-sm">
          <div className="flex items-center gap-4">
            <span aria-hidden className="text-3xl accent-text">
              &#127963;
            </span>
            <div>
              <h1 className="text-lg font-semibold text-slate-900">{t("title")}</h1>
              <p className="text-xs text-slate-500">{t("subtitle")}</p>
            </div>
          </div>
          <span className="rounded-full border accent-border px-4 py-2 text-sm font-semibold accent-text">
            {pack.slug.toUpperCase()} · {caseCount}
          </span>
        </header>

        <CavpRegistration pack={pack} active={active} packs={packs} />
        <CavpWorkbench pack={pack} />
      </main>
    </div>
  );
}
