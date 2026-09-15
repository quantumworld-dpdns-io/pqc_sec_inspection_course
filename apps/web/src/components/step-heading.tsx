import type { ReactNode } from "react";

/** The numbered step header from the CAVP station: circled index, eyebrow, title, blurb. */
export function StepHeading({
  index,
  eyebrow,
  title,
  children,
}: {
  index: string;
  eyebrow: string;
  title: string;
  children?: ReactNode;
}) {
  return (
    <div className="flex gap-5">
      <span className="flex size-10 shrink-0 items-center justify-center rounded-full accent-bg text-sm font-semibold text-white">
        {index}
      </span>
      <div>
        <p className="text-xs font-semibold uppercase tracking-[0.18em] accent-text">{eyebrow}</p>
        <h2 className="mt-1 text-2xl font-bold text-slate-900">{title}</h2>
        {children && <p className="mt-2 text-slate-600">{children}</p>}
      </div>
    </div>
  );
}
