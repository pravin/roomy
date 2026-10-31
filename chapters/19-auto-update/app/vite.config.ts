import { defineConfig } from "vite";

// Tauri expects the development server on a fixed port, and its own
// messages to stay on the screen.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
