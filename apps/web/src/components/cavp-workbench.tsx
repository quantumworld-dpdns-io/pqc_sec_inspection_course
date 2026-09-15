"use client";

import { useMemo, useState } from "react";
import { useTranslations } from "next-intl";

import { countLinesAndChars } from "@/lib/format";
import { computeCase, type ComputeInput } from "@/lib/mldsa";
import type { CavpPack, CavpValidationReport, Capability } from "@/lib/types";

import { StepHeading } from "./step-heading";

const FIELDS: Record<Capability, string[]> = {
  key_gen: ["seed"],
  sig_gen: ["sk", "message", "context"],
  sig_ver: ["pk", "message", "signature", "context"],
};

const OPTIONAL_FIELDS = new Set(["context"]);

/**
 * Steps 02–05 of the CAVP station.
 *
 * Step 03 runs entirely in the browser with @noble/post-quantum — the same library the
 * answer key was generated with — and the result is deliberately *not* wired straight into
 * step 04: copying it across is part of the exercise, exactly as the dev system had it.
 */
export function CavpWorkbench({ pack }: { pack: CavpPack }) {
  const t = useTranslations("cavp");
  const capability = pack.capability;
  const mode = pack.prompt.mode;
  const tests = pack.prompt.testGroups[0]?.tests ?? [];

  const promptJson = useMemo(() => JSON.stringify(pack.prompt, null, 2), [pack.prompt]);
  const [copied, setCopied] = useState(false);

  const [inputs, setInputs] = useState<Record<string, string>>({});
  const [computed, setComputed] = useState<string | null>(null);
  const [computeError, setComputeError] = useState<string | null>(null);

  const [results, setResults] = useState(() => skeleton(pack));
  const [report, setReport] = useState<CavpValidationReport | null>(null);
  const [validating, setValidating] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);

  const { lines, chars } = countLinesAndChars(results);
  const parses = useMemo(() => {
    try {
      JSON.parse(results);
      return true;
    } catch {
      return false;
    }
  }, [results]);

  async function copyPrompt() {
    await navigator.clipboard.writeText(promptJson);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 2000);
  }

  async function runCompute() {
    setComputeError(null);
    setComputed(null);
    try {
      const input: ComputeInput = {
        capability,
        parameterSet: pack.parameterSet,
        values: inputs,
      };
      setComputed(JSON.stringify(await computeCase(input), null, 2));
    } catch (cause) {
      setComputeError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function validate() {
    setValidating(true);
    setSubmitError(null);
    try {
      const sessionResponse = await fetch("/api/v1/cavp/sessions", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ slug: pack.slug }),
      });
      if (!sessionResponse.ok) {
        throw new Error(await errorText(sessionResponse));
      }
      const session = (await sessionResponse.json()) as { id: string };

      const response = await fetch(`/api/v1/cavp/sessions/${session.id}/validate`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ results }),
      });
      if (!response.ok) {
        throw new Error(await errorText(response));
      }
      setReport((await response.json()) as CavpValidationReport);
    } catch (cause) {
      setSubmitError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setValidating(false);
    }
  }

  return (
    <>
      <section className="mb-12">
        <StepHeading index="02" eyebrow={t("steps.pack")} title={t("steps.packTitle")}>
          {t("steps.packBody", { count: tests.length })}
        </StepHeading>

        <div className="mt-6 overflow-hidden rounded-lg bg-white shadow-sm">
          <header className="flex items-center justify-between px-5 py-4">
            <div>
              <h3 className="font-semibold text-slate-900">{t("promptFile", { mode })}</h3>
              <p className="text-xs text-slate-500">{t("exampleNote")}</p>
            </div>
            <button
              type="button"
              onClick={() => void copyPrompt()}
              className="text-sm font-medium accent-text hover:underline"
            >
              {copied ? t("copied") : t("copyPrompt")}
            </button>
          </header>
          <pre className="hex-wrap max-h-96 overflow-auto bg-slate-900 p-5 font-mono text-xs leading-relaxed text-slate-100">
            {promptJson}
          </pre>
        </div>
      </section>

      <section className="mb-12">
        <StepHeading index="03" eyebrow={t("steps.compute")} title={t("steps.computeTitle")}>
          {t("steps.computeBody")}
        </StepHeading>

        <div className="mt-6 rounded-lg bg-white p-5 shadow-sm">
          <header className="mb-4 flex items-center justify-between">
            <div>
              <h3 className="font-semibold text-slate-900">{t("browserCompute")}</h3>
              <p className="text-xs text-slate-500">{t("browserComputeBody")}</p>
            </div>
            <span className="rounded bg-slate-100 px-3 py-1 text-xs font-semibold accent-text">
              Noble 0.6.1
            </span>
          </header>

          <div className="space-y-4">
            {FIELDS[capability].map((field) => (
              <label key={field} className="block">
                <span className="flex items-center justify-between text-sm font-semibold text-slate-800">
                  {field}
                  <span className="text-xs font-normal text-slate-400">
                    {OPTIONAL_FIELDS.has(field) ? "optional" : t("testCase")}
                  </span>
                </span>
                <textarea
                  rows={3}
                  spellCheck={false}
                  value={inputs[field] ?? ""}
                  onChange={(event) =>
                    setInputs((current) => ({ ...current, [field]: event.target.value }))
                  }
                  className="hex-wrap mt-1 w-full rounded bg-slate-900 p-3 font-mono text-xs text-slate-100 placeholder:text-slate-500"
                  placeholder={`${field}`}
                />
              </label>
            ))}
          </div>

          <div className="mt-4 flex gap-3">
            <button
              type="button"
              onClick={() => void runCompute()}
              className="rounded accent-bg px-5 py-2.5 text-sm font-semibold text-white"
            >
              {t("run")}
            </button>
            <button
              type="button"
              onClick={() => {
                setInputs({});
                setComputed(null);
                setComputeError(null);
              }}
              className="rounded border border-slate-300 px-5 py-2.5 text-sm font-medium text-slate-700"
            >
              {t("clear")}
            </button>
          </div>

          <div className="mt-5 rounded border border-slate-200 bg-slate-50 p-4">
            {computeError ? (
              <p role="alert" className="text-sm text-rose-700">
                {computeError}
              </p>
            ) : computed ? (
              <pre className="hex-wrap max-h-64 overflow-auto font-mono text-xs text-slate-800">
                {computed}
              </pre>
            ) : (
              <p className="text-center text-sm text-slate-500">{t("computeEmpty")}</p>
            )}
          </div>
        </div>
      </section>

      <section className="mb-12">
        <StepHeading index="04" eyebrow={t("steps.results")} title={t("steps.resultsTitle")}>
          {t("steps.resultsBody", { count: Math.max(tests.length - 1, 0) })}
        </StepHeading>

        <div className="mt-6 overflow-hidden rounded-lg bg-white shadow-sm">
          <header className="flex items-center justify-between px-5 py-4">
            <div>
              <h3 className="font-semibold text-slate-900">{t("resultsTitle", { mode })}</h3>
              <p className="text-xs text-slate-500">{t("lines", { lines, chars })}</p>
            </div>
            <span
              className={`rounded px-3 py-1 text-xs font-semibold ${
                parses ? "bg-emerald-50 text-emerald-700" : "bg-rose-50 text-rose-700"
              }`}
            >
              {parses ? t("parseOk") : t("parseError")}
            </span>
          </header>
          <textarea
            value={results}
            spellCheck={false}
            onChange={(event) => setResults(event.target.value)}
            rows={18}
            className="hex-wrap w-full bg-slate-50 p-5 font-mono text-xs leading-relaxed text-slate-800 outline-none"
          />
          <footer className="flex items-center justify-end gap-4 border-t border-slate-200 px-5 py-4">
            {submitError && (
              <p role="alert" className="mr-auto text-sm text-rose-700">
                {submitError}
              </p>
            )}
            <button
              type="button"
              disabled={!parses || validating}
              onClick={() => void validate()}
              className="rounded accent-bg px-6 py-3 text-sm font-semibold text-white disabled:cursor-not-allowed disabled:opacity-40"
            >
              {validating ? t("validating") : t("validate", { mode, count: tests.length })}
            </button>
          </footer>
        </div>
      </section>

      <section className="mb-12">
        <StepHeading index="05" eyebrow={t("steps.report")} title={t("steps.reportTitle")}>
          {t("steps.reportBody")}
        </StepHeading>

        <div className="mt-6 rounded-lg bg-white p-6 shadow-sm">
          {report ? (
            <>
              <p className="text-lg font-semibold text-slate-900">
                {report.correct === report.total
                  ? t("allCorrect", { total: report.total })
                  : t("someWrong", { correct: report.correct, total: report.total })}
              </p>
              <ul className="mt-4 space-y-2">
                {report.cases.map((entry) => (
                  <li
                    key={entry.tcId}
                    className="flex items-start justify-between gap-4 rounded border border-slate-200 px-4 py-3 text-sm"
                  >
                    <span className="font-mono text-slate-700">
                      {t("caseStatus", { tcId: entry.tcId })}
                    </span>
                    <span className="flex flex-1 flex-col items-end">
                      <span
                        className={`rounded px-2 py-0.5 text-xs font-semibold ${
                          entry.status === "correct"
                            ? "bg-emerald-50 text-emerald-700"
                            : entry.status === "incorrect"
                              ? "bg-rose-50 text-rose-700"
                              : "bg-amber-50 text-amber-700"
                        }`}
                      >
                        {t(entry.status)}
                      </span>
                      {entry.reason && (
                        <span className="mt-1 text-xs text-slate-500">
                          {entry.field ? `${entry.field}: ` : ""}
                          {entry.reason}
                        </span>
                      )}
                    </span>
                  </li>
                ))}
              </ul>
            </>
          ) : (
            <div className="py-8 text-center">
              <p className="font-semibold text-slate-900">{t("notSubmittedTitle", { mode })}</p>
              <p className="mt-1 text-slate-500">{t("notSubmittedBody")}</p>
            </div>
          )}
        </div>
      </section>
    </>
  );
}

async function errorText(response: Response): Promise<string> {
  const body = (await response.json().catch(() => ({}))) as { error?: string };
  return body.error ?? response.statusText;
}

/**
 * Pre-fill step 04 with the ACVP results envelope: tcId 1 carries the worked example the
 * API hands out, the rest are blanks for the student to fill in.
 */
function skeleton(pack: CavpPack): string {
  const tests = pack.prompt.testGroups[0]?.tests ?? [];
  const example = (pack.example ?? {}) as Record<string, unknown>;

  const blank = (tcId: number): Record<string, unknown> => {
    if (tcId === 1 && Object.keys(example).length > 0) {
      const { tcId: _ignored, ...answer } = example;
      return { tcId, ...answer };
    }
    switch (pack.capability) {
      case "key_gen":
        return { tcId, pk: "", sk: "" };
      case "sig_gen":
        return { tcId, signature: "" };
      case "sig_ver":
        return { tcId, testPassed: null };
    }
  };

  return JSON.stringify(
    {
      vsId: pack.vsId,
      algorithm: pack.prompt.algorithm,
      mode: pack.prompt.mode,
      revision: pack.prompt.revision,
      testGroups: [
        {
          tgId: pack.prompt.testGroups[0]?.tgId ?? 1,
          tests: tests.map((test) => blank(Number(test.tcId))),
        },
      ],
    },
    null,
    2,
  );
}
