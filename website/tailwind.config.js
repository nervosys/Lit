/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        brand: {
          50: "#f0f7ff",
          100: "#e0efff",
          200: "#b9dfff",
          300: "#7ac5ff",
          400: "#36a9ff",
          500: "#0090f0",
          600: "#0070cc",
          700: "#0058a6",
          800: "#004b89",
          900: "#003f71",
          950: "#00284a",
        },
      },
    },
  },
  plugins: [],
};
