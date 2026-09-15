import { getTranslations, setRequestLocale } from "next-intl/server";

import { Nav } from "@/components/nav";
import { Link } from "@/i18n/navigation";

export default async function HomePage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  setRequestLocale(locale);
  const t = await getTranslations("home");

  const modules = [
    { key: "pqcas", href: "/tests/new", body: "pqcasBody" },
    { key: "kem", href: "/kem/library", body: "kemBody" },
    { key: "cavp", href: "/cavp/key_gen", body: "cavpBody" },
  ] as const;

  return (
    <>
      <Nav />
      <main id="main" className="mx-auto max-w-7xl px-6 py-12">
        <h1 className="text-3xl font-semibold tracking-tight text-slate-900">{t("title")}</h1>
        <p className="mt-2 max-w-2xl text-slate-600">{t("subtitle")}</p>

        <div className="mt-10 grid gap-6 md:grid-cols-3">
          {modules.map((module) => (
            <Link
              key={module.key}
              href={module.href}
              className="group rounded-lg border border-slate-200 bg-white p-6 shadow-sm transition hover:border-slate-400 hover:shadow"
            >
              <h2 className="text-lg font-semibold text-slate-900">{t(module.key)}</h2>
              <p className="mt-2 text-sm leading-relaxed text-slate-600">{t(module.body)}</p>
              <span className="mt-4 inline-block text-sm font-medium text-slate-900 group-hover:underline">
                {t("open")} →
              </span>
            </Link>
          ))}
        </div>
      </main>
    </>
  );
}
