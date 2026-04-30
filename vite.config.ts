import { defineConfig } from "vite";

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

export default defineConfig({
  clearScreen: false,
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
});
