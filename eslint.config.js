import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  // This repo root is cluttered with unrelated AI-tool config directories
  // (.claude, .cursor, etc.) and generated output — only the app's own
  // frontend source under src/ is meant to be linted.
  { ignores: ["**/*", "!src/**"] },
  {
    files: ["src/**/*.{ts,tsx}"],
    extends: [js.configs.recommended, ...tseslint.configs.recommended, reactHooks.configs.flat["recommended-latest"]],
    languageOptions: {
      ecmaVersion: 2022,
      globals: globals.browser,
    },
  },
);
