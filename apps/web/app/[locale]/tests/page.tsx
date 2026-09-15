import { getTranslations, setRequestLocale } from "next-intl/server";

import { Nav } from "@/components/nav";
import { StatusChip } from "@/components/status-chip";
import { Card } from "@/components/ui";
import { Link } from "@/i18n/navigation";
import { apiFetch } from "@/lib/api";
import { formatDateTime } from "@/lib/format";
import type { Test } from "@/lib/types";

// Lab data is live: every render asks the API rather than baking a snapshot at build time
// (which would also make the image build depend on a running backend).
export const dynamic = "force-dynamic";


export default async function TasksPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  setRequestLocale(locale);
  const t = await getTranslations("tasks");
  const tests = await apiFetch<Test[]>("/tests");

  return (
    <>
      <Nav active="tasks" />
      <main id="main" className="mx-auto max-w-7xl px-6 py-10">
        <h1 className="text-2xl font-semibold text-slate-900">{t("title")}</h1>

        <Card className="mt-6">
          {tests.length === 0 ? (
            <p className="py-6 text-center text-sm text-slate-500">{t("empty")}</p>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-200 text-left text-slate-500">
                  <th className="px-3 py-2 font-medium">{t("target")}</th>
                  <th className="px-3 py-2 font-medium">{t("toolPlugin")}</th>
                  <th className="px-3 py-2 font-medium">{t("status")}</th>
                  <th className="px-3 py-2 font-medium">{t("subtasks")}</th>
                  <th className="px-3 py-2 font-medium">{t("created")}</th>
                </tr>
              </thead>
              <tbody>
                {tests.map((test) => (
                  <tr key={test.id} className="border-b border-slate-100 last:border-b-0">
                    <td className="px-3 py-3">
                      <Link
                        href={`/tests/${test.id}`}
                        className="font-mono font-medium text-slate-900 hover:underline"
                      >
                        {test.host}:{test.port}
                      </Link>
                    </td>
                    <td className="px-3 py-3 font-mono text-slate-600">
                      {test.tool} / {test.plugin}
                    </td>
                    <td className="px-3 py-3">
                      <StatusChip status={test.status} />
                    </td>
                    <td className="px-3 py-3 font-mono text-slate-700">{test.subtaskCount}</td>
                    <td className="px-3 py-3 text-slate-600">
                      {formatDateTime(test.createdAt, locale)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Card>
      </main>
    </>
  );
}
