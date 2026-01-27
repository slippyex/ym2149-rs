/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './index.dev.html',
    './src/**/*.{ts,js}',
  ],
  theme: {
    extend: {
      colors: {
        chip: {
          dark: '#0a0a0f',
          darker: '#05050a',
          purple: '#8b5cf6',
          cyan: '#06b6d4',
          pink: '#ec4899',
          green: '#10b981',
        },
      },
      fontFamily: {
        mono: ['JetBrains Mono', 'Fira Code', 'monospace'],
        display: ['Space Grotesk', 'system-ui', 'sans-serif'],
      },
    },
  },
  plugins: [],
};
