import type { ReactNode } from "react";

export function Card({
  title,
  action,
  children,
  className = "",
}: {
  title?: ReactNode;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`rounded-lg border border-slate-200 bg-white shadow-sm ${className}`}>
      {(title || action) && (
        <header className="flex items-center justify-between gap-4 border-b border-slate-200 px-5 py-4">
          {typeof title === "string" ? <h2 className="font-semibold text-slate-900">{title}</h2> : title}
          {action}
        </header>
      )}
      <div className="p-5">{children}</div>
    </section>
  );
}

export function Field({
  label,
  required = false,
  hint,
  children,
}: {
  label: string;
  required?: boolean;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <label className="block space-y-1.5">
      <span className="text-sm font-medium text-slate-700">
        {required && <span className="mr-1 text-rose-500">*</span>}
        {label}
      </span>
      {children}
      {hint && <span className="block text-xs text-slate-500">{hint}</span>}
    </label>
  );
}

/** Two-column definition table used by Outcome and TLS Summary. */
export function DefinitionTable({ rows }: { rows: Array<{ label: string; value: ReactNode }> }) {
  return (
    <table className="w-full border-collapse text-sm">
      <tbody>
        {rows.map((row) => (
          <tr key={row.label} className="border-b border-slate-200 last:border-b-0">
            <th scope="row" className="w-1/2 bg-slate-50 px-4 py-3 text-left font-normal text-slate-600">
              {row.label}
            </th>
            <td className="px-4 py-3 font-mono text-slate-900">{row.value}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export function Collapsible({ title, children }: { title: string; children: ReactNode }) {
  return (
    <details className="group rounded-lg border border-slate-200 bg-white">
      <summary className="cursor-pointer list-none px-4 py-3 text-sm font-medium text-slate-800 marker:hidden">
        <span className="mr-2 inline-block transition-transform group-open:rotate-90" aria-hidden>
          ›
        </span>
        {title}
      </summary>
      <div className="border-t border-slate-200 p-4">{children}</div>
    </details>
  );
}

export function CodeBlock({ children, className = "" }: { children: string; className?: string }) {
  return (
    <pre
      className={`hex-wrap max-h-96 overflow-auto rounded-lg bg-slate-900 p-4 font-mono text-xs leading-relaxed text-slate-100 ${className}`}
    >
      {children}
    </pre>
  );
}
