import { getTranslations } from "next-intl/server";

import { Link } from "@/i18n/navigation";

import { LocaleSwitcher } from "./locale-switcher";

type NavKey = "newTest" | "tasks" | "environment" | "kem" | "cavp";

const ITEMS: Array<{ key: NavKey; href: string }> = [
  { key: "newTest", href: "/tests/new" },
  { key: "tasks", href: "/tests" },
  { key: "environment", href: "/environment" },
  { key: "kem", href: "/kem/library" },
  { key: "cavp", href: "/cavp/key_gen" },
];

/** The dark navy bar from the dev system, extended to reach all three modules. */
export async function Nav({ active }: { active?: NavKey }) {
  const t = await getTranslations("nav");
  const app = await getTranslations("app");

  return (
    <header className="bg-brand-navy text-white">
      <nav className="mx-auto flex max-w-7xl items-stretch gap-0 px-0" aria-label="Main">
        <Link
          href="/"
          className="flex items-center px-6 py-4 text-lg font-semibold tracking-tight hover:bg-white/10"
        >
          {app("name")}
        </Link>
        <ul className="flex flex-1 items-stretch">
          {ITEMS.map((item) => (
            <li key={item.key}>
              <Link
                href={item.href}
                aria-current={active === item.key ? "page" : undefined}
                className={`flex h-full items-center px-5 text-sm transition-colors hover:bg-white/10 ${
                  active === item.key ? "bg-white/15 font-medium text-white" : "text-slate-300"
                }`}
              >
                {t(item.key)}
              </Link>
            </li>
          ))}
        </ul>
        <div className="flex items-center px-4">
          <LocaleSwitcher />
        </div>
      </nav>
    </header>
  );
}
