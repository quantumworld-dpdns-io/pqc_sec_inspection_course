"use client";

import { useMemo, useState } from "react";
import { useTranslations } from "next-intl";

import { AlgorithmGrid } from "@/components/algorithm-grid";
import { Card, Field } from "@/components/ui";
import { useRouter } from "@/i18n/navigation";
import { TOOLS, pluginsFor } from "@/lib/tools";
import type { Algorithm, Test } from "@/lib/types";

export function NewTestForm({
  groups,
  sigAlgs,
}: {
  groups: Algorithm[];
  sigAlgs: Algorithm[];
}) {
  const t = useTranslations("newTest");
  const router = useRouter();

  const [host, setHost] = useState("");
  const [port, setPort] = useState("443");
  const [httpPath, setHttpPath] = useState("/");
  const [tool, setTool] = useState(TOOLS[0].tool);
  const [plugin, setPlugin] = useState(TOOLS[0].plugins[0]);
  const [selectedGroups, setSelectedGroups] = useState<Set<string>>(new Set());
  const [selectedSigAlgs, setSelectedSigAlgs] = useState<Set<string>>(new Set());
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const subtaskCount = selectedGroups.size * selectedSigAlgs.size;
  const canSubmit = host.trim() !== "" && subtaskCount > 0 && !submitting;

  const plugins = useMemo(() => pluginsFor(tool), [tool]);

  function toggle(setter: typeof setSelectedGroups) {
    return (name: string) =>
      setter((current) => {
        const next = new Set(current);
        if (!next.delete(name)) {
          next.add(name);
        }
        return next;
      });
  }

  function selectAll(algorithms: Algorithm[], setter: typeof setSelectedGroups) {
    setter(new Set(algorithms.filter((a) => a.enabled).map((a) => a.name)));
  }

  async function submit() {
    setSubmitting(true);
    setError(null);
    try {
      const response = await fetch("/api/v1/tests", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          host: host.trim(),
          port: Number(port),
          httpPath,
          tool,
          plugin,
          groups: [...selectedGroups],
          sigAlgs: [...selectedSigAlgs],
        }),
      });

      if (!response.ok) {
        const body = (await response.json().catch(() => ({}))) as { error?: string };
        throw new Error(body.error ?? response.statusText);
      }

      const test = (await response.json()) as Test;
      router.push(`/tests/${test.id}`);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setSubmitting(false);
    }
  }

  return (
    <form
      className="mt-6 space-y-6"
      onSubmit={(event) => {
        event.preventDefault();
        if (canSubmit) void submit();
      }}
    >
      <div className="grid gap-6 lg:grid-cols-2">
        <Card title={t("connection")}>
          <div className="space-y-4">
            <Field label={t("host")} required>
              <input
                className="w-full rounded border border-slate-300 px-3 py-2 font-mono text-sm"
                placeholder="172.17.0.1"
                value={host}
                onChange={(event) => setHost(event.target.value)}
                required
              />
            </Field>
            <Field label={t("port")} required>
              <input
                className="w-full rounded border border-slate-300 px-3 py-2 font-mono text-sm"
                type="number"
                min={1}
                max={65535}
                value={port}
                onChange={(event) => setPort(event.target.value)}
                required
              />
            </Field>
          </div>
        </Card>

        <Card title={t("connectionOptions")}>
          <p className="mb-4 text-sm text-slate-500">{t("methodNote")}</p>
          <Field label={t("httpPath")} required>
            <input
              className="w-full rounded border border-slate-300 px-3 py-2 font-mono text-sm"
              value={httpPath}
              onChange={(event) => setHttpPath(event.target.value)}
              required
            />
          </Field>
        </Card>

        <Card title={t("toolPlugin")}>
          <div className="space-y-4">
            <Field label={t("tool")} required>
              <select
                className="w-full rounded border border-slate-300 px-3 py-2 text-sm"
                value={tool}
                onChange={(event) => {
                  setTool(event.target.value);
                  setPlugin(pluginsFor(event.target.value)[0] ?? "");
                }}
              >
                {TOOLS.map((entry) => (
                  <option key={entry.tool} value={entry.tool}>
                    {entry.tool}
                  </option>
                ))}
              </select>
            </Field>
            <Field label={t("plugin")} required>
              <select
                className="w-full rounded border border-slate-300 px-3 py-2 text-sm"
                value={plugin}
                onChange={(event) => setPlugin(event.target.value)}
              >
                {plugins.map((name) => (
                  <option key={name} value={name}>
                    {name}
                  </option>
                ))}
              </select>
            </Field>
          </div>
        </Card>
      </div>

      <Card title={t("testOptions")}>
        <div className="space-y-8">
          <fieldset>
            <legend className="mb-3 flex items-center gap-3 text-sm font-medium text-slate-700">
              <span>
                <span className="mr-1 text-rose-500">*</span>
                {t("groups")}
              </span>
              <button
                type="button"
                className="text-xs text-slate-500 underline"
                onClick={() => selectAll(groups, setSelectedGroups)}
              >
                {t("selectAll")}
              </button>
              <button
                type="button"
                className="text-xs text-slate-500 underline"
                onClick={() => setSelectedGroups(new Set())}
              >
                {t("clear")}
              </button>
            </legend>
            <AlgorithmGrid
              algorithms={groups}
              selected={selectedGroups}
              onToggle={toggle(setSelectedGroups)}
            />
          </fieldset>

          <fieldset>
            <legend className="mb-3 flex items-center gap-3 text-sm font-medium text-slate-700">
              <span>
                <span className="mr-1 text-rose-500">*</span>
                {t("sigAlgs")}
              </span>
              <button
                type="button"
                className="text-xs text-slate-500 underline"
                onClick={() => selectAll(sigAlgs, setSelectedSigAlgs)}
              >
                {t("selectAll")}
              </button>
              <button
                type="button"
                className="text-xs text-slate-500 underline"
                onClick={() => setSelectedSigAlgs(new Set())}
              >
                {t("clear")}
              </button>
            </legend>
            <AlgorithmGrid
              algorithms={sigAlgs}
              selected={selectedSigAlgs}
              onToggle={toggle(setSelectedSigAlgs)}
            />
          </fieldset>

          <p className="text-xs text-slate-500">{t("disabledHint")}</p>
        </div>

        <div className="mt-8 flex flex-wrap items-center justify-between gap-4 border-t border-slate-200 pt-5">
          <div>
            <h3 className="text-sm font-semibold text-slate-800">{t("summary")}</h3>
            <dl className="mt-2 flex flex-wrap gap-6 text-sm text-slate-600">
              <div className="flex gap-2">
                <dt>{t("groupCount")}:</dt>
                <dd className="font-mono font-semibold text-slate-900">{selectedGroups.size}</dd>
              </div>
              <div className="flex gap-2">
                <dt>{t("sigAlgCount")}:</dt>
                <dd className="font-mono font-semibold text-slate-900">{selectedSigAlgs.size}</dd>
              </div>
              <div className="flex gap-2">
                <dt>{t("subtaskCount")}:</dt>
                <dd className="font-mono font-semibold text-slate-900">{subtaskCount}</dd>
              </div>
            </dl>
          </div>
          <button
            type="submit"
            disabled={!canSubmit}
            className="rounded bg-brand-navy px-5 py-2.5 text-sm font-medium text-white disabled:cursor-not-allowed disabled:opacity-40"
          >
            {submitting ? t("creating") : t("create")}
          </button>
        </div>

        {error && (
          <p role="alert" className="mt-4 rounded border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
            {error}
          </p>
        )}
      </Card>
    </form>
  );
}
