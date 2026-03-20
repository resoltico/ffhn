export default [
  {
    ignores: ["eslint.config.js"],
  },
  {
    files: ["bin/**/*.js", "lib/**/*.js"],
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "module",
      globals: {
        console: "readonly",
        process: "readonly",
        performance: "readonly",
        Buffer: "readonly",
        fetch: "readonly",
        TextDecoder: "readonly",
        AbortSignal: "readonly",
        AbortController: "readonly",
        URL: "readonly",
        structuredClone: "readonly",
        setTimeout: "readonly",
        clearTimeout: "readonly"
      },
    },
    rules: {
      indent: ["error", 4],
      "no-undef": "error",
      "no-unused-vars": ["error", { varsIgnorePattern: "^_" }],
      eqeqeq: ["error", "always"],
      "no-var": "error",
      "prefer-const": "error",
      curly: ["error", "all"],
      "require-await": "error",
      "no-shadow": "error",
      "no-throw-literal": "error",
      "prefer-template": "error",
      semi: ["error", "always"],
      "no-console": "off",
      "no-magic-numbers": [
        "error",
        {
          ignore: [-1, 0, 1, 2, 3, 4, 5, 8, 16, 50, 200, 255, 400, 404, 408, 429, 500, 750, 1000, 30000, 1024],
          ignoreArrayIndexes: true,
          enforceConst: true,
          detectObjects: true
        }
      ]
    }
  },
  {
    files: ["test/**/*.js"],
    rules: {
      "no-magic-numbers": "off",
    },
  }
];
