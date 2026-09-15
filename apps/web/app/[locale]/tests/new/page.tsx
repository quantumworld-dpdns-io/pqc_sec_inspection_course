import { getTranslations, setRequestLocale } from "next-intl/server";

import { NewTestForm } from "@/components/new-test-form";
import { Nav } from "@/components/nav";
import { apiFetch } from "@/lib/api";
import type { Algorithm } from "@/lib/types";

// Lab data is live: every render asks the API rather than baking a snapshot at build time
// (which would also make the image build depend on a running backend).
export const dynamic = "force-dynamic";


export default async function NewTestPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  setRequestLocale(locale);
  const t = await getTranslations("newTest");

  // The catalog changes only when the lab reseeds, so a short revalidate keeps the form
  // snappy without pinning stale algorithm lists for long.
  const algorithms = await apiFetch<Algorithm[]>("/algorithms", { revalidate: 60 });
  const groups = algorithms.filter((algorithm) => algorithm.kind === "kem_group");
  const sigAlgs = algorithms.filter((algorithm) => algorithm.kind === "sig_alg");

  return (
    <>
      <Nav active="newTest" />
      <main id="main" className="mx-auto max-w-7xl px-6 py-8">
        <h1 className="text-2xl font-semibold text-slate-900">{t("title")}</h1>
        <NewTestForm groups={groups} sigAlgs={sigAlgs} />
      </main>
    </>
  );
}
