"use client";

import type { Algorithm } from "@/lib/types";

/**
 * The three-column checkbox grid from the dev system. Disabled entries stay visible and
 * greyed out on purpose: students should see that an algorithm exists but is not enabled
 * here, rather than wonder why it vanished.
 */
export function AlgorithmGrid({
  algorithms,
  selected,
  onToggle,
}: {
  algorithms: Algorithm[];
  selected: Set<string>;
  onToggle: (name: string) => void;
}) {
  return (
    <div className="grid gap-x-6 gap-y-2 sm:grid-cols-2 lg:grid-cols-3">
      {algorithms.map((algorithm) => (
        <label
          key={algorithm.id}
          className={`flex items-center gap-2 text-sm ${
            algorithm.enabled ? "text-slate-800" : "cursor-not-allowed text-slate-400"
          }`}
          title={algorithm.family ?? undefined}
        >
          <input
            type="checkbox"
            className="size-4 rounded border-slate-300"
            checked={selected.has(algorithm.name)}
            disabled={!algorithm.enabled}
            onChange={() => onToggle(algorithm.name)}
          />
          <span className="font-mono">{algorithm.displayName}</span>
        </label>
      ))}
    </div>
  );
}
