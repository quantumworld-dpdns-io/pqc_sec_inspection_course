import { useTranslations } from "next-intl";

import type { ExecStatus, ReportStatus, ResultStatus, TaskStatus } from "@/lib/types";

type AnyStatus = TaskStatus | ExecStatus | ResultStatus | ReportStatus;

const TONE: Record<string, string> = {
  passed: "bg-emerald-50 text-emerald-700 border-emerald-200",
  finished: "bg-slate-100 text-slate-700 border-slate-300",
  complete: "bg-slate-50 text-slate-600 border-slate-200",
  running: "bg-sky-50 text-sky-700 border-sky-200",
  queued: "bg-slate-50 text-slate-500 border-slate-200",
  pending: "bg-slate-50 text-slate-500 border-slate-200",
  partial: "bg-amber-50 text-amber-700 border-amber-200",
  unsupported: "bg-amber-50 text-amber-700 border-amber-200",
  disabled: "bg-slate-100 text-slate-500 border-slate-300",
  failed: "bg-rose-50 text-rose-700 border-rose-200",
  error: "bg-rose-50 text-rose-700 border-rose-200",
  missing: "bg-slate-50 text-slate-500 border-slate-200",
  cancelled: "bg-slate-100 text-slate-600 border-slate-300",
};

export function StatusChip({ status, label }: { status: AnyStatus; label?: string }) {
  const t = useTranslations("status");
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded border px-2 py-1 text-xs font-medium ${
        TONE[status] ?? TONE.pending
      }`}
    >
      {label ?? t(status)}
    </span>
  );
}
