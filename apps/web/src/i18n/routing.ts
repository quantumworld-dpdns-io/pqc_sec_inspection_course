import { defineRouting } from "next-intl/routing";

/** The dev system shipped English and 繁體中文; the language switcher toggles between them. */
export const routing = defineRouting({
  locales: ["en", "zh-TW"],
  defaultLocale: "en",
});

export type Locale = (typeof routing.locales)[number];
