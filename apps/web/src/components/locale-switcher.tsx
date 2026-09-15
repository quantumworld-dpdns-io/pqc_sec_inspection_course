"use client";

import { useLocale, useTranslations } from "next-intl";
import { useParams } from "next/navigation";
import { useTransition } from "react";

import { usePathname, useRouter } from "@/i18n/navigation";
import { routing } from "@/i18n/routing";

const LABELS: Record<string, string> = {
  en: "English",
  "zh-TW": "繁體中文",
};

export function LocaleSwitcher() {
  const locale = useLocale();
  const t = useTranslations("app");
  const router = useRouter();
  const pathname = usePathname();
  const params = useParams();
  const [pending, startTransition] = useTransition();

  return (
    <label className="flex items-center gap-2 rounded bg-white px-3 py-2 text-sm text-slate-900">
      <span className="sr-only">{t("language")}</span>
      <select
        className="bg-transparent outline-none"
        value={locale}
        disabled={pending}
        onChange={(event) => {
          const next = event.target.value;
          startTransition(() => {
            // `params` keeps dynamic segments (test id, capability) pointing at the same
            // record when the locale changes.
            router.replace(
              // @ts-expect-error -- pathname is a literal route type; params fills its slots.
              { pathname, params },
              { locale: next },
            );
          });
        }}
      >
        {routing.locales.map((value) => (
          <option key={value} value={value}>
            {LABELS[value] ?? value}
          </option>
        ))}
      </select>
      <span aria-hidden>🌐</span>
    </label>
  );
}
