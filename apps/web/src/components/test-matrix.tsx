"use client";

import { useEffect, useMemo, useState } from "react";
import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import { resultChipClass, resultGlyph } from "@/lib/format";
import type { Matrix, ResultStatus, TestEvent } from "@/lib/types";

/**
 * Groups down the side, signature algorithms across the top, one chip per subtask.
 *
 * The grid subscribes to the test's SSE channel so a running task fills in live instead of
 * needing a refresh; the initial snapshot comes from the server render, so the first paint
 * is already correct.
 */
export function TestMatrix({
  testId,
  initialMatrix,
}: {
  testId: string;
  initialMatrix: Matrix;
}) {
  const t = useTranslations("test");
  const statusT = useTranslations("status");
  const [matrix, setMatrix] = useState(initialMatrix);
  const [live, setLive] = useState(false);

  useEffect(() => {
    const source = new EventSource(`/api/v1/tests/${testId}/events`);
    source.onopen = () => setLive(true);
    source.onerror = () => setLive(false);
    source.onmessage = (event) => {
      const parsed = JSON.parse(event.data) as TestEvent;
      if (parsed.type !== "subtaskUpdated") return;
      setMatrix((current) => ({
        ...current,
        cells: current.cells.map((cell) =>
          cell.subtaskId === parsed.subtaskId
            ? { ...cell, resultStatus: parsed.resultStatus, execStatus: parsed.execStatus }
            : cell,
        ),
      }));
    };
    return () => source.close();
  }, [testId]);

  const byKey = useMemo(() => {
    const map = new Map<string, { subtaskId: string; resultStatus: ResultStatus }>();
    for (const cell of matrix.cells) {
      map.set(`${cell.groupName}|${cell.sigAlg}`, cell);
    }
    return map;
  }, [matrix]);

  const done = matrix.cells.filter(
    (cell) => cell.execStatus === "finished" || cell.execStatus === "error",
  ).length;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between text-sm text-slate-600">
        <span>{t("progress", { done, total: matrix.cells.length })}</span>
        {live && (
          <span className="flex items-center gap-1.5 text-emerald-700">
            <span className="size-2 animate-pulse rounded-full bg-emerald-500" aria-hidden />
            {t("live")}
          </span>
        )}
      </div>

      <div className="overflow-x-auto">
        <table className="border-separate border-spacing-1 text-sm">
          <thead>
            <tr>
              <th className="sticky left-0 z-10 bg-white" />
              {matrix.sigAlgs.map((sigAlg) => (
                <th
                  key={sigAlg}
                  className="px-2 pb-2 text-center font-mono text-xs font-medium text-slate-700"
                >
                  {sigAlg}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {matrix.groups.map((group) => (
              <tr key={group}>
                <th
                  scope="row"
                  className="sticky left-0 z-10 bg-white pr-3 text-right font-mono text-xs font-medium text-slate-700"
                >
                  {group}
                </th>
                {matrix.sigAlgs.map((sigAlg) => {
                  const cell = byKey.get(`${group}|${sigAlg}`);
                  if (!cell) {
                    return <td key={sigAlg} className="size-9" />;
                  }
                  const label = `${group} x ${sigAlg}: ${statusT(cell.resultStatus)}`;
                  return (
                    <td key={sigAlg} className="p-0">
                      <Link
                        href={`/tests/${testId}/subtasks/${cell.subtaskId}`}
                        aria-label={label}
                        title={label}
                        className={`flex size-9 items-center justify-center rounded border text-sm font-semibold transition hover:scale-105 ${
                          resultChipClass[cell.resultStatus]
                        }`}
                      >
                        <span aria-hidden>{resultGlyph[cell.resultStatus]}</span>
                      </Link>
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="flex flex-wrap items-center gap-4 border-t border-slate-200 pt-3 text-xs text-slate-600">
        <span className="font-medium">{t("legend")}</span>
        {(["passed", "failed", "unsupported", "disabled", "pending"] as ResultStatus[]).map(
          (status) => (
            <span key={status} className="flex items-center gap-1.5">
              <span
                className={`flex size-5 items-center justify-center rounded border text-[10px] ${resultChipClass[status]}`}
                aria-hidden
              >
                {resultGlyph[status]}
              </span>
              {statusT(status)}
            </span>
          ),
        )}
      </div>
    </div>
  );
}
