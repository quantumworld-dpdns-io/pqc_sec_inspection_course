import { notFound } from "next/navigation";
import { getTranslations, setRequestLocale } from "next-intl/server";

import { KemRunner } from "@/components/kem-runner";
import { Nav } from "@/components/nav";
import { Link } from "@/i18n/navigation";
import { apiFetch } from "@/lib/api";
import type { Algorithm, Endpoint, KemRunMode } from "@/lib/types";

// Lab data is live: every render asks the API rather than baking a snapshot at build time
// (which would also make the image build depend on a running backend).
export const dynamic = "force-dynamic";


const MODES = {
  library: { api: "library_compat", tab: "library" },
  group: { api: "group_compat", tab: "group" },
  priority: { api: "priority", tab: "priority" },
} as const;

type ModeKey = keyof typeof MODES;

export function generateStaticParams() {
  return Object.keys(MODES).map((mode) => ({ mode }));
}

export default async function KemPage({
  params,
}: {
  params: Promise<{ locale: string; mode: string }>;
}) {
  const { locale, mode } = await params;
  setRequestLocale(locale);

  if (!(mode in MODES)) {
    notFound();
  }
  const modeKey = mode as ModeKey;
  const t = await getTranslations("kem");

  const [endpoints, algorithms] = await Promise.all([
    apiFetch<Endpoint[]>("/endpoints", { revalidate: 60 }),
    apiFetch<Algorithm[]>("/algorithms?kind=kem_group", { revalidate: 60 }),
  ]);

  return (
    <div data-module="kem" className="min-h-screen">
      <Nav active="kem" />
      <div className="kem-grid min-h-[calc(100vh-64px)]">
        <main id="main" className="mx-auto max-w-5xl px-6 py-10">
          <header className="mb-6 flex items-baseline gap-4">
            <h1 className="font-mono text-2xl font-semibold tracking-[0.2em] accent-text">
              [ PQC KEM DEMO ]
            </h1>
            <p className="font-mono text-xs uppercase tracking-widest text-slate-500">
              {t("title")}
            </p>
          </header>

          <nav className="mb-6 flex gap-6 border-b border-slate-300 font-mono text-sm">
            {(Object.keys(MODES) as ModeKey[]).map((key) => (
              <Link
                key={key}
                href={`/kem/${key}`}
                className={`-mb-px border-b-2 px-1 pb-3 ${
                  key === modeKey
                    ? "accent-border accent-text font-semibold"
                    : "border-transparent text-slate-500 hover:text-slate-800"
                }`}
              >
                {t(`tabs.${MODES[key].tab}`)}
              </Link>
            ))}
          </nav>

          <KemRunner
            mode={MODES[modeKey].api as KemRunMode}
            endpoints={endpoints}
            groups={algorithms}
            locale={locale}
          />
        </main>
      </div>
    </div>
  );
}
