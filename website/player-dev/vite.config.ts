import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig(({ mode }) => ({
  // Base path for deployment
  base: './',
  // Use root as publicDir since static assets are there
  publicDir: false,
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
    },
    extensions: ['.mjs', '.js', '.mts', '.ts', '.jsx', '.tsx', '.json'],
  },
  build: {
    target: 'ES2022',
    minify: 'terser',
    terserOptions: {
      mangle: {
        toplevel: false, // Don't mangle top-level names (breaks WASM bindings)
        properties: {
          regex: /^_[^_]/, // Only mangle properties starting with single _
        },
      },
      compress: {
        drop_console: true,
        drop_debugger: true,
        passes: 2,
      },
    },
    rollupOptions: {
      input: resolve(__dirname, 'index.dev.html'),
      external: [/\/pkg\/ym2149_wasm\.js$/],
      output: {
        entryFileNames: '[name].[hash].js',
        chunkFileNames: '[name].[hash].js',
        assetFileNames: '[name].[hash].[ext]',
        paths: {
          // Rewrite external import to local path
          '../pkg/ym2149_wasm.js': './ym2149_wasm.js'
        },
      },
    },
    outDir: '../player',
    emptyOutDir: false, // Don't delete catalog files
  },
  plugins: [],
  assetsInclude: ['**/*.wasm'],
  optimizeDeps: {
    exclude: ['../pkg/ym2149_wasm.js'],
  },
  server: {
    port: 5173,
    strictPort: false,
    fs: {
      // Allow serving files from pkg directory
      allow: ['..'],
    },
    open: '/index.dev.html',
  },
  appType: 'mpa', // Multi-page app mode
  preview: {
    port: 4173,
  },
}));
