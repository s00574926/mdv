import { defineConfig } from "vite";
import { fileURLToPath, URL } from "node:url";

function normalizeModuleId(id: string): string {
  return id.replaceAll("\\", "/");
}

function mermaidChunkName(id: string): string | undefined {
  const normalizedId = normalizeModuleId(id);

  if (!normalizedId.includes("/node_modules/")) {
    return undefined;
  }

  if (
    normalizedId.includes("/node_modules/cytoscape/") ||
    normalizedId.includes("/node_modules/cytoscape-cose-bilkent/") ||
    normalizedId.includes("/node_modules/cytoscape-fcose/")
  ) {
    return "mermaid-cytoscape";
  }

  if (normalizedId.includes("/node_modules/katex/")) {
    return "mermaid-katex";
  }

  if (
    normalizedId.includes("/node_modules/@upsetjs/venn.js/")
  ) {
    return "mermaid-venn";
  }

  if (normalizedId.includes("/node_modules/dagre-d3-es/")) {
    return "mermaid-dagre";
  }

  if (
    normalizedId.includes("/node_modules/d3/") ||
    normalizedId.includes("/node_modules/d3-")
  ) {
    return "mermaid-d3";
  }

  return undefined;
}

export default defineConfig(({ mode }) => ({
  clearScreen: false,
  resolve: {
    alias:
      mode === "e2e"
        ? {
            "@tauri-apps/api/core": fileURLToPath(
              new URL("./test/e2e/mocks/tauri-core.ts", import.meta.url)
            ),
            "@tauri-apps/api/event": fileURLToPath(
              new URL("./test/e2e/mocks/tauri-event.ts", import.meta.url)
            ),
            "@tauri-apps/api/path": fileURLToPath(
              new URL("./test/e2e/mocks/tauri-path.ts", import.meta.url)
            ),
            "@tauri-apps/api/window": fileURLToPath(
              new URL("./test/e2e/mocks/tauri-window.ts", import.meta.url)
            ),
            "@tauri-apps/plugin-dialog": fileURLToPath(
              new URL("./test/e2e/mocks/tauri-dialog.ts", import.meta.url)
            )
          }
        : undefined
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          return mermaidChunkName(id);
        }
      }
    }
  },
  server: {
    port: 1420,
    strictPort: true
  },
  preview: {
    port: 1420,
    strictPort: true
  }
}));
