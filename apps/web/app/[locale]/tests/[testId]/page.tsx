import { notFound } from "next/navigation";
import { getTranslations, setRequestLocale } from "next-intl/server";

import { Nav } from "@/components/nav";
import { StatusChip } from "@/components/status-chip";
import { TestMatrix } from "@/components/test-matrix";
import { Card } from "@/components/ui";
import { Link } from "@/i18n/navigation";
import { apiFetchOptional } from "@/lib/api";
import type { Matrix, TestDetail } from "@/lib/types";

// Lab data is live: every render asks the API rather than baking a snapshot at build time
// (which would also make the image build depend on a running backend).
export const dynamic = "force-dynamic";


export default async function TestPage({
  params,
}: {
  params: Promise<{ locale: string; testId: string }>;
}) {
  const { locale, testId } = await params;
  setRequestLocale(locale);
  const t = await getTranslations("test");
  const tasks = await getTranslations("tasks");

  const [test, matrix] = await Promise.all([
    apiFetchOptional<TestDetail>(`/tests/${testId}`),
    apiFetchOptional<Matrix>(`/tests/${testId}/matrix`),
  ]);

  if (!test || !matrix) {
    notFound();
  }

  return (
    <>
      <Nav active="tasks" />
      <main id="main" className="mx-auto max-w-7xl px-6 py-8">
        <nav className="text-sm text-slate-500">
          <Link href="/tests" className="hover:underline">
            {tasks("title")}
          </Link>
          <span className="px-2">/</span>
          <span className="text-slate-900">
            {test.host}:{test.port}
          </span>
        </nav>

        <h1 className="mt-3 font-mono text-3xl font-semibold tracking-tight text-slate-900">
          {test.host}:{test.port}
        </h1>
        <div className="mt-2 flex items-center gap-3 text-sm">
          <StatusChip status={test.status} />
          <span className="font-mono text-slate-500">
            {test.tool} / {test.plugin}
          </span>
        </div>

        <Card title={t("matrix")} className="mt-6 overflow-hidden">
          <TestMatrix testId={test.id} initialMatrix={matrix} />
        </Card>
      </main>
    </>
  );
}
