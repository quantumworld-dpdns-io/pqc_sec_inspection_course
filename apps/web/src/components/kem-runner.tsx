"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslations } from "next-intl";

import { formatClock } from "@/lib/format";
import { resultChipClass } from "@/lib/format";
import type {
  Algorithm,
  Endpoint,
  KemEvent,
  KemMatrixCell,
  KemRun,
  KemRunLog,
  KemRunMode,
  ResultStatus,
} from "@/lib/types";

const MATRIX_ADAPTERS = [
  "openssl",
  "boringssl",
  "wolfssl",
  "go",
  "bouncycastle",
  "aws-lc",
  "nss",
  "circl",
];

const MATRIX_KEMS = [
  "MLKEM1024",
  "MLKEM768",
  "SecP256r1MLKEM768",
  "SecP384r1MLKEM1024",
  "X25519Kyber768Draft00",
  "X25519MLKEM768",
];

/**
 * Drives one KEM DEMO run and streams its console.
 *
 * Logs arrive over SSE and are also persisted, so reopening a finished run replays the same
 * lines the student saw live.
 */
export function KemRunner({
  mode,
  endpoints,
  groups,
  locale,
}: {
  mode: KemRunMode;
  endpoints: Endpoint[];
  groups: Algorithm[];
  locale: string;
}) {
  const t = useTranslations("kem");
  const statusT = useTranslations("status");

  const [endpoint, setEndpoint] = useState(endpoints[0]?.slug ?? "");
  const [selected, setSelected] = useState<Set<string>>(
    () => new Set(groups.filter((group) => group.enabled).slice(0, 8).map((group) => group.name)),
  );
  const [logs, setLogs] = useState<KemRunLog[]>([]);
  const [cells, setCells] = useState<KemMatrixCell[]>([]);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const sourceRef = useRef<EventSource | null>(null);

  useEffect(() => () => sourceRef.current?.close(), []);

  const toggle = useCallback((name: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (!next.delete(name)) next.add(name);
      return next;
    });
  }, []);

  async function run() {
    setRunning(true);
    setError(null);
    setLogs([]);
    setCells([]);
    sourceRef.current?.close();

    try {
      const response = await fetch("/api/v1/kem/runs", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          mode,
          endpoint,
          kemGroups: mode === "group_compat" ? [...selected] : [],
        }),
      });
      if (!response.ok) {
        const body = (await response.json().catch(() => ({}))) as { error?: string };
        throw new Error(body.error ?? response.statusText);
      }
      const created = (await response.json()) as KemRun;

      const source = new EventSource(`/api/v1/kem/runs/${created.id}/events`);
      sourceRef.current = source;
      source.onmessage = (event) => {
        const parsed = JSON.parse(event.data) as KemEvent;
        if (parsed.type === "log") {
          setLogs((current) => [
            ...current,
            {
              runId: created.id,
              seq: parsed.seq,
              at: parsed.at,
              level: parsed.level,
              message: parsed.message,
            },
          ]);
        } else if (parsed.type === "cell") {
          setCells((current) => [
            ...current.filter(
              (cell) => !(cell.kem === parsed.kem && cell.adapter === parsed.adapter),
            ),
            {
              runId: created.id,
              kem: parsed.kem,
              adapter: parsed.adapter,
              verdict: parsed.verdict,
              detail: parsed.detail,
            },
          ]);
        } else {
          setRunning(false);
          source.close();
        }
      };
      source.onerror = () => {
        setRunning(false);
        source.close();
      };
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setRunning(false);
    }
  }

  const runLabel =
    mode === "library_compat"
      ? t("runCompatibility")
      : mode === "group_compat"
        ? t("runHandshake")
        : t("runPriority");

  return (
    <div className="space-y-6">
      <section className="rounded border border-slate-300 bg-white">
        <header className="flex items-center gap-3 border-b border-slate-300 px-5 py-3 font-mono text-sm">
          <span className="rounded border border-slate-400 px-2 py-0.5 text-xs">01</span>
          <h2 className="font-semibold tracking-wide">{t("title")}</h2>
        </header>

        <div className="space-y-5 p-5">
          <label className="block">
            <span className="font-mono text-xs uppercase tracking-widest text-slate-500">
              {t("targetEndpoint")}
            </span>
            <select
              className="mt-2 w-full rounded border-2 border-slate-800 bg-white px-4 py-3 font-medium"
              value={endpoint}
              onChange={(event) => setEndpoint(event.target.value)}
            >
              {endpoints.map((option) => (
                <option key={option.slug} value={option.slug}>
                  {locale === "zh-TW" ? option.labelZh : option.label}
                </option>
              ))}
            </select>
          </label>

          {mode === "library_compat" && (
            <p className="font-mono text-sm text-slate-600">{t("libraryHint")}</p>
          )}
          {mode === "priority" && (
            <p className="font-mono text-sm text-slate-600">{t("priorityHint")}</p>
          )}

          {mode === "group_compat" && (
            <fieldset>
              <legend className="font-mono text-xs uppercase tracking-widest text-slate-500">
                {t("groupsLabel")}
              </legend>
              <div className="mt-3 flex flex-wrap gap-2 rounded border border-slate-200 bg-slate-50 p-3">
                {groups.map((group) => {
                  const active = selected.has(group.name);
                  return (
                    <button
                      key={group.id}
                      type="button"
                      disabled={!group.enabled}
                      onClick={() => toggle(group.name)}
                      className={`rounded border px-3 py-1.5 font-mono text-xs tracking-wide transition ${
                        !group.enabled
                          ? "cursor-not-allowed border-slate-200 text-slate-400"
                          : active
                            ? "accent-border accent-text border-2 font-semibold"
                            : "border-slate-300 text-slate-600 hover:border-slate-500"
                      }`}
                    >
                      {group.displayName.toUpperCase()}
                    </button>
                  );
                })}
              </div>
              <div className="mt-2 flex gap-4 text-xs text-slate-500">
                <button
                  type="button"
                  className="underline"
                  onClick={() =>
                    setSelected(
                      new Set(groups.filter((group) => group.enabled).map((group) => group.name)),
                    )
                  }
                >
                  {t("selectAll")}
                </button>
                <button type="button" className="underline" onClick={() => setSelected(new Set())}>
                  {t("clear")}
                </button>
              </div>
            </fieldset>
          )}

          <button
            type="button"
            onClick={() => void run()}
            disabled={running || endpoint === "" || (mode === "group_compat" && selected.size === 0)}
            className="w-full rounded border-2 border-slate-800 px-4 py-4 text-center font-mono text-base font-semibold tracking-[0.15em] accent-text hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {running ? t("running") : runLabel}
          </button>
        </div>

        <footer className="flex items-center gap-4 border-t border-slate-300 bg-slate-50 px-5 py-2 font-mono text-xs text-slate-500">
          <span className="flex items-center gap-1.5">
            <span
              className={`size-2 rounded-full ${running ? "animate-pulse bg-amber-500" : "bg-emerald-500"}`}
              aria-hidden
            />
            {running ? t("running") : t("ready")}
          </span>
          <span>TLS KEM DEMO</span>
        </footer>
      </section>

      {error && (
        <p role="alert" className="rounded border border-rose-300 bg-rose-50 px-4 py-2 text-sm text-rose-700">
          {error}
        </p>
      )}

      <section className="rounded border border-slate-300 bg-white">
        <header className="flex items-center justify-between border-b border-slate-300 px-5 py-3 font-mono text-sm">
          <span className="flex items-center gap-3">
            <span className="rounded border border-slate-400 px-2 py-0.5 text-xs">02</span>
            <h2 className="font-semibold tracking-wide">{t("output")}</h2>
          </span>
          {!running && logs.length > 0 && <span className="text-xs text-slate-500">{t("done")}</span>}
        </header>

        <div className="p-5">
          {mode === "library_compat" && cells.length > 0 && (
            <CompatibilityMatrix cells={cells} statusLabel={statusT} matrixLabel={t("matrix")} />
          )}

          <div className="mt-4 overflow-hidden rounded-lg border border-slate-300">
            <div className="flex items-center gap-2 border-b border-slate-300 bg-slate-100 px-4 py-2">
              <span className="size-3 rounded-full bg-rose-400" aria-hidden />
              <span className="size-3 rounded-full bg-amber-400" aria-hidden />
              <span className="size-3 rounded-full bg-emerald-400" aria-hidden />
              <span className="ml-2 font-mono text-xs text-slate-500">tlsender — output</span>
            </div>
            <div className="max-h-96 overflow-auto bg-slate-50 p-4 font-mono text-xs leading-relaxed">
              {logs.length === 0 ? (
                <p className="text-slate-400">{t("waiting")}</p>
              ) : (
                <ul className="space-y-1">
                  {logs.map((log) => (
                    <li key={log.seq} className={logToneClass(log.level)}>
                      <span aria-hidden className="mr-2 text-slate-400">
                        {log.level === "result" ? "✓" : "▸"}
                      </span>
                      [{formatClock(log.at)}] {log.message}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}

function logToneClass(level: string): string {
  switch (level) {
    case "result":
      return "text-emerald-700";
    case "finding_warn":
      return "text-amber-700";
    case "finding_ok":
      return "text-emerald-700";
    default:
      return "text-slate-700";
  }
}

function CompatibilityMatrix({
  cells,
  statusLabel,
  matrixLabel,
}: {
  cells: KemMatrixCell[];
  statusLabel: (key: string) => string;
  matrixLabel: string;
}) {
  const lookup = new Map(cells.map((cell) => [`${cell.kem}|${cell.adapter}`, cell.verdict]));

  return (
    <div className="overflow-x-auto rounded-lg border border-slate-300">
      <table className="w-full border-collapse text-xs">
        <thead>
          <tr className="bg-slate-100 font-mono text-slate-600">
            <th className="px-4 py-3 text-right font-medium">{matrixLabel}</th>
            {MATRIX_ADAPTERS.map((adapter) => (
              <th key={adapter} className="px-3 py-3 text-center font-medium uppercase">
                {adapter}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {MATRIX_KEMS.map((kem) => (
            <tr key={kem} className="border-t border-slate-200">
              <th scope="row" className="px-4 py-2 text-right font-mono font-medium text-slate-700">
                {kem}
              </th>
              {MATRIX_ADAPTERS.map((adapter) => {
                const verdict = (lookup.get(`${kem}|${adapter}`) ?? "pending") as ResultStatus;
                return (
                  <td key={adapter} className="px-3 py-2 text-center">
                    <span
                      className={`inline-block rounded border px-2 py-1 font-mono text-[10px] font-semibold uppercase ${resultChipClass[verdict]}`}
                    >
                      {statusLabel(verdict)}
                    </span>
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
