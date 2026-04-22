import assert from "node:assert/strict";

import {
  TRUSTED_PREVIEW_TRUST_MODEL,
  clearPreview,
  renderExplorer,
  renderWorkspaceFrame,
  setTrustedPreviewHtml
} from "../../src/view.ts";

class FakeClassList {
  #tokens = new Set();

  add(token) {
    this.#tokens.add(token);
  }

  remove(token) {
    this.#tokens.delete(token);
  }

  has(token) {
    return this.#tokens.has(token);
  }
}

function createElements() {
  return {
    appRoot: {
      classList: new FakeClassList()
    },
    explorerPanel: {
      hidden: true
    },
    explorerTree: {
      innerHTML: ""
    },
    preview: {
      innerHTML: ""
    }
  };
}

function createWorkspace(overrides = {}) {
  return {
    document: {
      title: "Doc",
      html: "<h1>Doc</h1>",
      sourceName: "doc.md",
      sourcePath: "C:/docs/doc.md",
      watching: true,
      trustModel: TRUSTED_PREVIEW_TRUST_MODEL
    },
    currentFilePath: "C:/docs/doc.md",
    explorer: null,
    recentPaths: [],
    ...overrides
  };
}

function runTest(name, fn) {
  try {
    fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

runTest("renderExplorer hides the panel when no explorer is open", () => {
  const elements = createElements();
  elements.explorerTree.innerHTML = "stale";
  elements.appRoot.classList.add("app-root-with-explorer");

  renderExplorer(elements, null, null);

  assert.equal(elements.explorerPanel.hidden, true);
  assert.equal(elements.explorerTree.innerHTML, "");
  assert.equal(elements.appRoot.classList.has("app-root-with-explorer"), false);
});

runTest("renderExplorer marks the active file and escapes file-path attributes", () => {
  const elements = createElements();
  const activePath = 'C:/docs/<draft>&"quote".md';
  const explorer = {
    name: "docs",
    path: "C:/docs",
    children: [
      {
        name: "docs",
        path: "C:/docs",
        kind: "directory",
        children: [
          {
            name: "draft.md",
            path: activePath,
            kind: "file",
            children: []
          }
        ]
      }
    ]
  };

  renderExplorer(elements, explorer, activePath);

  assert.equal(elements.explorerPanel.hidden, false);
  assert.equal(elements.appRoot.classList.has("app-root-with-explorer"), true);
  assert.match(elements.explorerTree.innerHTML, /tree-file-button-active/);
  assert.match(elements.explorerTree.innerHTML, /aria-current="page"/);
  assert.match(elements.explorerTree.innerHTML, /data-file-path="C:\/docs\/&lt;draft&gt;&amp;&quot;quote&quot;\.md"/);
});

runTest("setTrustedPreviewHtml rejects unexpected trust models", () => {
  const preview = { innerHTML: "" };

  assert.throws(
    () =>
      setTrustedPreviewHtml(preview, {
        title: "Doc",
        html: "<p>unsafe</p>",
        sourceName: "doc.md",
        sourcePath: "C:/docs/doc.md",
        watching: false,
        trustModel: "wrong-model"
      }),
    /Unexpected preview trust model/
  );
});

runTest("renderWorkspaceFrame injects trusted HTML and clears empty documents", () => {
  const elements = createElements();

  renderWorkspaceFrame(
    elements,
    createWorkspace({
      explorer: {
        name: "docs",
        path: "C:/docs",
        children: []
      }
    })
  );

  assert.equal(elements.preview.innerHTML, "<h1>Doc</h1>");

  renderWorkspaceFrame(
    elements,
    createWorkspace({
      document: {
        title: "",
        html: "",
        sourceName: "",
        sourcePath: "",
        watching: false,
        trustModel: TRUSTED_PREVIEW_TRUST_MODEL
      }
    })
  );

  assert.equal(elements.preview.innerHTML, "");
});

runTest("clearPreview empties the preview region", () => {
  const preview = { innerHTML: "<p>stale</p>" };
  clearPreview(preview);
  assert.equal(preview.innerHTML, "");
});
