import next from "eslint-config-next";

const config = [
  ...next,
  {
    ignores: [".next/**", "node_modules/**", "e2e/**", "playwright-report/**", "test-results/**"],
  },
];

export default config;
