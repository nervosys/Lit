/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        brand: {
          50: "#ecfdf5",
          100: "#d1fae5",
          200: "#a7f3d0",
          300: "#6ee7b7",
          400: "#00ffcc",
          500: "#00e6b8",
          600: "#00cc99",
          700: "#009973",
          800: "#007a5e",
          900: "#005c47",
          950: "#003d2e",
        },
        cyber: {
          50: "#f0fdfa",
          100: "#ccfbf1",
          200: "#99f6e4",
          300: "#5eead4",
          400: "#00ffcc",
          500: "#00e6b8",
          600: "#00cc99",
          700: "#0f766e",
          800: "#115e59",
          900: "#134e4a",
          950: "#042f2e",
        },
        navy: {
          800: "#0f1623",
          900: "#0a0e17",
          950: "#060a12",
        },
      },
      fontFamily: {
        mono: ['"JetBrains Mono"', '"Fira Code"', 'ui-monospace', 'SFMono-Regular', 'monospace'],
      },
      boxShadow: {
        'glow': '0 0 20px rgba(0, 255, 204, 0.15)',
        'glow-sm': '0 0 10px rgba(0, 255, 204, 0.1)',
        'glow-lg': '0 0 40px rgba(0, 255, 204, 0.2)',
      },
      animation: {
        'pulse-slow': 'pulse 4s cubic-bezier(0.4, 0, 0.6, 1) infinite',
        'scan': 'scan 8s linear infinite',
      },
      keyframes: {
        scan: {
          '0%': { transform: 'translateY(-100%)' },
          '100%': { transform: 'translateY(100%)' },
        },
      },
    },
  },
  plugins: [],
};
