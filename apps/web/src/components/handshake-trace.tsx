import type { HandshakeMessage } from "@/lib/types";

/**
 * The indexed message trace: one row per handshake record, direction on the left, the
 * record's fields on the right. Failure cases end in an Alert row, which is usually the
 * single most useful line in the whole report.
 */
export function HandshakeTrace({
  messages,
  outboundLabel,
  inboundLabel,
}: {
  messages: HandshakeMessage[];
  outboundLabel: string;
  inboundLabel: string;
}) {
  if (messages.length === 0) {
    return null;
  }

  return (
    <section>
      <ul className="divide-y divide-slate-200 border-y border-slate-200">
        {messages.map((message) => (
          <li key={message.index} className="grid grid-cols-[3rem_10rem_1fr] gap-4 py-6">
            <span className="font-mono text-sm text-slate-400">{message.index}</span>
            <span className="self-center font-mono text-sm text-slate-600">
              {message.direction === "outbound" ? outboundLabel : inboundLabel}
            </span>
            <div>
              <span
                className={`inline-block rounded px-2 py-1 text-xs font-medium ${
                  message.kind === "Alert"
                    ? "bg-rose-500 text-white"
                    : "border border-slate-300 bg-slate-50 text-slate-800"
                }`}
              >
                {message.kind}
              </span>
              <dl className="mt-3 grid grid-cols-[minmax(8rem,auto)_1fr] gap-x-4 gap-y-1 text-sm">
                {message.fields
                  .filter((field) => field.values.length > 0)
                  .map((field) => (
                    <div key={field.label} className="contents">
                      <dt className="text-slate-500">{field.label}</dt>
                      <dd className="hex-wrap font-mono text-slate-900">
                        {field.values.join(", ")}
                      </dd>
                    </div>
                  ))}
              </dl>
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}
