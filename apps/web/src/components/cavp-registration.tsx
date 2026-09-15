import { getTranslations } from "next-intl/server";

import { Link } from "@/i18n/navigation";
import type { Capability, CavpPack, CavpPackSummary } from "@/lib/types";

import { StepHeading } from "./step-heading";

/**
 * Step 01 — Capability registration.
 *
 * The controls are read-only on purpose: the station runs a fixed capability set, and the
 * panel exists to show students what an ACVP registration actually declares.
 */
export async function CavpRegistration({
  pack,
  active,
  packs,
}: {
  pack: CavpPack;
  active: Capability;
  packs: CavpPackSummary[];
}) {
  const t = await getTranslations("cavp");
  const registration = pack.registration as Record<string, unknown>;

  const parameterSets = (registration.parameterSets as string[] | undefined) ?? [];
  const vectorTypes = (registration.vectorTypes as string[] | undefined) ?? [];
  const limits = registration.capabilityLimits as
    | {
        messageLength?: { min: number; max: number; increment: number; unit: string };
        contextLength?: { min: number; max: number; increment: number; unit: string };
        hashAlgorithms?: string[];
      }
    | undefined;

  return (
    <section className="mb-12">
      <StepHeading index="01" eyebrow={t("steps.registration")} title={t("steps.registrationTitle")}>
        {t("steps.registrationBody")}
      </StepHeading>

      <div className="mt-6 overflow-hidden rounded-lg bg-white shadow-sm">
        <div className="grid grid-cols-3 bg-slate-100">
          {packs.map((entry) => (
            <Link
              key={entry.slug}
              href={`/cavp/${entry.capability}`}
              className={`px-6 py-4 text-left ${
                entry.capability === active ? "bg-white" : "hover:bg-white/60"
              }`}
            >
              <span
                className={`block font-semibold ${
                  entry.capability === active ? "accent-text" : "text-slate-500"
                }`}
              >
                {t(`capabilities.${entry.capability}`)}
              </span>
              <span className="block text-xs text-slate-500">
                {t(`capabilityHints.${entry.capability}`)}
              </span>
            </Link>
          ))}
        </div>

        <div className="space-y-6 p-6">
          <div>
            <h3 className="mb-3 font-semibold text-slate-900">{t("algorithmMode")}</h3>
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
              <ReadOnlyField label={t("algorithm")} value={String(registration.algorithm ?? "")} />
              <ReadOnlyField
                label={t("mode")}
                value={`${String(registration.mode ?? "")} (${t(`capabilityHints.${active}`)})`}
              />
              <ReadOnlyField label={t("revision")} value={String(registration.revision ?? "")} />
              <ReadOnlyField
                label={t("vsId")}
                value={String(registration.capabilityVsId ?? pack.vsId)}
              />
            </div>
          </div>

          <div>
            <h3 className="mb-3 font-semibold text-slate-900">{t("testParameters")}</h3>
            <div className="grid gap-6 sm:grid-cols-3">
              <ChoiceList
                label={t("parameterSets")}
                options={parameterSets}
                selected={[pack.parameterSet]}
              />
              <ChoiceList
                label={t("vectorType")}
                options={vectorTypes}
                selected={[String(registration.selectedVectorType ?? "AFT")]}
              />
              <ChoiceList
                label={t("sample")}
                options={["true", "false"]}
                selected={[String(registration.sample ?? true)]}
              />
            </div>
          </div>

          {registration.signatureInterfaces != null && (
            <div>
              <h3 className="mb-3 font-semibold text-slate-900">{t("signatureOptions")}</h3>
              <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-4">
                <ChoiceList
                  label={t("signatureInterfaces")}
                  options={(registration.signatureInterfaces as string[]) ?? []}
                  selected={[String(registration.selectedSignatureInterface ?? "external")]}
                />
                <ChoiceList
                  label={t("preHash")}
                  options={(registration.preHash as string[]) ?? []}
                  selected={[String(registration.selectedPreHash ?? "pure")]}
                />
                <ChoiceList
                  label={t("externalMu")}
                  options={["false", "true"]}
                  selected={[String(registration.externalMu ?? false)]}
                />
                <ChoiceList
                  label={t("deterministic")}
                  options={["true", "false"]}
                  selected={[String(registration.deterministic ?? true)]}
                />
              </div>
            </div>
          )}

          {limits && (
            <div>
              <h3 className="mb-3 font-semibold text-slate-900">{t("capabilityLimits")}</h3>
              <div className="grid gap-4 lg:grid-cols-3">
                {limits.messageLength && (
                  <ReadOnlyField
                    label={t("messageLength")}
                    value={`${limits.messageLength.min}-${limits.messageLength.max} ${limits.messageLength.unit} / step ${limits.messageLength.increment}`}
                  />
                )}
                {limits.contextLength && (
                  <ReadOnlyField
                    label={t("contextLength")}
                    value={`${limits.contextLength.min}-${limits.contextLength.max} ${limits.contextLength.unit} / step ${limits.contextLength.increment}`}
                  />
                )}
                {limits.hashAlgorithms && (
                  <ReadOnlyField
                    label={t("hashAlgorithms")}
                    value={limits.hashAlgorithms.join(", ")}
                  />
                )}
              </div>
            </div>
          )}

          <p className="border-l-4 accent-border bg-slate-50 px-4 py-3 text-sm text-slate-600">
            {t("fixedNote")}
          </p>
        </div>
      </div>
    </section>
  );
}

function ReadOnlyField({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <span className="block text-xs font-medium text-slate-500">{label}</span>
      <p className="mt-1 truncate rounded border border-slate-200 bg-slate-50 px-3 py-2 text-sm font-medium text-slate-900">
        {value}
      </p>
    </div>
  );
}

function ChoiceList({
  label,
  options,
  selected,
}: {
  label: string;
  options: string[];
  selected: string[];
}) {
  return (
    <div>
      <span className="block text-xs font-medium text-slate-500">{label}</span>
      <div className="mt-2 flex flex-wrap gap-2">
        {options.map((option) => {
          const isSelected = selected.includes(option);
          return (
            <span
              key={option}
              className={`flex items-center gap-2 rounded border px-3 py-1.5 text-sm ${
                isSelected
                  ? "accent-border accent-text border-2 font-semibold"
                  : "border-slate-200 text-slate-400"
              }`}
            >
              <span aria-hidden>{isSelected ? "☑" : "☐"}</span>
              {option}
            </span>
          );
        })}
      </div>
    </div>
  );
}
