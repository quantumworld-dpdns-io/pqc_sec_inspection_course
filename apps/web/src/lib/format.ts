import type { ResultStatus } from "./types";

/** Tailwind classes per matrix verdict, shared by the PQ-CAS and KEM matrices. */
export const resultChipClass: Record<ResultStatus, string> = {
  passed: "bg-emerald-500 text-white border-emerald-600",
  failed: "bg-rose-500 text-white border-rose-600",
  unsupported: "bg-amber-100 text-amber-800 border-amber-300",
  disabled: "bg-slate-100 text-slate-500 border-slate-300",
  pending: "bg-slate-50 text-slate-400 border-slate-200",
};

export const resultGlyph: Record<ResultStatus, string> = {
  passed: "✓",
  failed: "✕",
  unsupported: "–",
  disabled: "·",
  pending: "…",
};

export function formatDateTime(value: string | null | undefined, locale: string): string {
  if (!value) return "—";
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "medium",
    timeStyle: "medium",
  }).format(new Date(value));
}

/** `3:12:38 PM` style stamps, matching the dev system's console. */
export function formatClock(value: string): string {
  return new Intl.DateTimeFormat("en-US", {
    hour: "numeric",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(value));
}

export function countLinesAndChars(text: string): { lines: number; chars: number } {
  return { lines: text === "" ? 0 : text.split("\n").length, chars: text.length };
}
