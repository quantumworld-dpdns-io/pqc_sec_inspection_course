import { notFound } from "next/navigation";
import { getTranslations, setRequestLocale } from "next-intl/server";

import { HandshakeTrace } from "@/components/handshake-trace";
import { StatusChip } from "@/components/status-chip";
import { Collapsible, DefinitionTable } from "@/components/ui";
import { Link } from "@/i18n/navigation";
import { apiFetchOptional } from "@/lib/api";
import type { SubtaskDetail } from "@/lib/types";

// Lab data is live: every render asks the API rather than baking a snapshot at build time
// (which would also make the image build depend on a running backend).
export const dynamic = "force-dynamic";


/**
 * The subtask report. The dev system showed this as a drawer over the matrix; here it is a
 * real route so a specific failing combination can be linked to and reloaded.
 */
export default async function SubtaskPage({
  params,
}: {
  params: Promise<{ locale: string; testId: string; subtaskId: string }>;
}) {
  const { locale, testId, subtaskId } = await params;
  setRequestLocale(locale);
  const t = await getTranslations("subtask");
  const statusT = await getTranslations("status");

  const detail = await apiFetchOptional<SubtaskDetail>(`/subtasks/${subtaskId}`);
  if (!detail) {
    notFound();
  }

  const summary = detail.report?.tlsSummary;
  const na = t("notAvailable");

  return (
    <main id="main" className="min-h-screen bg-white">
      <header className="flex items-center justify-between gap-4 border-b border-slate-200 px-6 py-4">
        <div className="flex items-center gap-4">
          <Link
            href={`/tests/${testId}`}
            aria-label={t("close")}
            className="rounded p-1 text-2xl leading-none text-slate-500 hover:bg-slate-100"
          >
            &times;
          </Link>
          <h1 className="font-mono text-xl font-semibold text-slate-900">{detail.title}</h1>
          <span className="text-sm text-slate-400">
            {detail.position} / {detail.total}
          </span>
        </div>
        <nav className="flex gap-2">
          <NavButton
            href={detail.previousId ? `/tests/${testId}/subtasks/${detail.previousId}` : null}
            label={t("previous")}
            glyph={"\u2039"}
          />
          <NavButton
            href={detail.nextId ? `/tests/${testId}/subtasks/${detail.nextId}` : null}
            label={t("next")}
            glyph={"\u203A"}
            trailing
          />
        </nav>
      </header>

      <div className="mx-auto max-w-5xl space-y-10 px-6 py-8">
        <section>
          <h2 className="mb-3 text-lg font-semibold text-slate-900">{t("outcome")}</h2>
          <DefinitionTable
            rows={[
              {
                label: t("resultStatus"),
                value: (
                  <StatusChip
                    status={detail.resultStatus}
                    label={
                      detail.resultStatus === "failed"
                        ? statusT("testFailed")
                        : detail.resultStatus === "passed"
                          ? statusT("testPassed")
                          : undefined
                    }
                  />
                ),
              },
              { label: t("executionStatus"), value: <StatusChip status={detail.execStatus} /> },
              { label: t("reportStatus"), value: <StatusChip status={detail.reportStatus} /> },
              { label: t("failedStage"), value: detail.failedStage ?? na },
              { label: t("errorSummary"), value: detail.errorSummary ?? "" },
            ]}
          />
        </section>

        <section>
          <h2 className="mb-3 text-lg font-semibold text-slate-900">{t("tlsSummary")}</h2>
          <DefinitionTable
            rows={[
              {
                label: t("handshakeSuccess"),
                value: (
                  <span className="rounded border border-slate-300 px-2 py-0.5 text-xs">
                    {summary?.handshakeSuccess ? t("yes") : t("no")}
                  </span>
                ),
              },
              { label: t("tlsVersion"), value: summary?.tlsVersion ?? na },
              { label: t("cipherSuite"), value: summary?.cipherSuite ?? na },
              { label: t("keyExchangeGroup"), value: summary?.keyExchangeGroup ?? na },
              { label: t("certVerifySigAlg"), value: summary?.certVerifySigAlg ?? na },
            ]}
          />
        </section>

        {detail.report ? (
          <>
            <HandshakeTrace
              messages={detail.report.messages}
              outboundLabel={t("clientToServer")}
              inboundLabel={t("serverToClient")}
            />

            <section>
              <h2 className="mb-3 text-lg font-semibold text-slate-900">{t("certificateChain")}</h2>
              {detail.report.certChain.length === 0 ? (
                <p className="text-sm text-slate-500">{t("noCertificates")}</p>
              ) : (
                <ul className="space-y-3">
                  {detail.report.certChain.map((cert, index) => (
                    <li key={`${cert.subject}-${index}`} className="rounded border border-slate-200 p-4 text-sm">
                      <p className="font-mono text-slate-900">{cert.subject}</p>
                      <p className="mt-1 font-mono text-xs text-slate-500">{cert.issuer}</p>
                      {cert.signature_algorithm && (
                        <p className="mt-2 font-mono text-xs text-slate-600">
                          {cert.signature_algorithm}
                          {cert.public_key_algorithm ? ` / ${cert.public_key_algorithm}` : ""}
                        </p>
                      )}
                    </li>
                  ))}
                </ul>
              )}
            </section>

            <section>
              <h2 className="mb-3 text-lg font-semibold text-slate-900">{t("http")}</h2>
              {detail.report.http ? (
                <div className="rounded border border-slate-200 p-4 font-mono text-sm">
                  <p className="text-slate-900">HTTP {detail.report.http.status}</p>
                  <ul className="mt-2 space-y-1 text-xs text-slate-600">
                    {detail.report.http.headers.map((header) => (
                      <li key={header.label}>
                        {header.label}: {header.values.join(", ")}
                      </li>
                    ))}
                  </ul>
                </div>
              ) : (
                <p className="text-sm text-slate-500">{t("httpNotAttempted")}</p>
              )}
            </section>

            <div className="space-y-3">
              <Collapsible title={t("diagnostics")}>
                <pre className="hex-wrap overflow-auto text-xs text-slate-700">
                  {JSON.stringify(detail.report.diagnostics, null, 2)}
                </pre>
              </Collapsible>
              <Collapsible title={t("rawJson")}>
                <pre className="hex-wrap max-h-96 overflow-auto text-xs text-slate-700">
                  {JSON.stringify(detail.report.raw, null, 2)}
                </pre>
              </Collapsible>
              <Collapsible title={t("evidence")}>
                {detail.report.evidence ? (
                  <pre className="hex-wrap max-h-96 overflow-auto text-xs text-slate-700">
                    {decodeEvidence(detail.report.evidence)}
                  </pre>
                ) : (
                  <p className="text-sm text-slate-500">{t("noEvidence")}</p>
                )}
              </Collapsible>
            </div>
          </>
        ) : (
          <p className="text-sm text-slate-500">{t("noReport")}</p>
        )}
      </div>
    </main>
  );
}

/** Adapters send the capture base64-encoded; render it as text when it is text. */
function decodeEvidence(encoded: string): string {
  try {
    return Buffer.from(encoded, "base64").toString("utf8");
  } catch {
    return encoded;
  }
}

function NavButton({
  href,
  label,
  glyph,
  trailing = false,
}: {
  href: string | null;
  label: string;
  glyph: string;
  trailing?: boolean;
}) {
  const className =
    "flex items-center gap-2 rounded border border-slate-300 px-4 py-2 text-sm text-slate-800";
  const content = (
    <>
      {!trailing && <span aria-hidden>{glyph}</span>}
      {label}
      {trailing && <span aria-hidden>{glyph}</span>}
    </>
  );

  // A missing neighbour keeps its place in the header rather than shifting the buttons.
  if (!href) {
    return (
      <span className={`${className} cursor-not-allowed opacity-40`} aria-disabled="true">
        {content}
      </span>
    );
  }
  return (
    <Link href={href} className={`${className} hover:bg-slate-50`}>
      {content}
    </Link>
  );
}
