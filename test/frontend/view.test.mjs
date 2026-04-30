import assert from "node:assert/strict";

import {
  clampContextMenuPosition,
  DEFAULT_PREVIEW_SCALE,
  MAX_PREVIEW_SCALE,
  MIN_PREVIEW_SCALE,
  applyPreviewScale,
  clampPreviewScale,
  getNextPreviewScale,
  isPreviewZoomShortcut,
  TRUSTED_PREVIEW_TRUST_MODEL,
  clearPreview,
  renderDocumentTabs,
  renderEditor,
  sameDocumentTabs,
  sameExplorer,
  sameRecentPaths,
  hasUnsavedUntitledContent,
  getUnsavedUntitledDocumentIndexes,
  renderExplorer,
  renderWorkspaceFrame,
  shouldShowEditorPreview,
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

class FakeStyle {
  #properties = new Map();

  setProperty(name, value) {
    this.#properties.set(name, value);
  }

  getPropertyValue(name) {
    return this.#properties.get(name) ?? "";
  }
}

function createElements() {
  return {
    appRoot: {
      classList: new FakeClassList()
    },
    documentTabsPanel: {
      hidden: true
    },
    documentTabs: {
      innerHTML: ""
    },
    editorPanel: {
      hidden: true
    },
    editor: {
      value: "",
      readOnly: false
    },
    explorerPanel: {
      hidden: true
    },
    explorerTree: {
      innerHTML: ""
    },
    preview: {
      innerHTML: "",
      hidden: false,
      style: new FakeStyle()
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
    editorText: null,
    currentFilePath: "C:/docs/doc.md",
    explorerUpdated: true,
    explorer: null,
    recentPaths: [],
    documentTabs: [],
    activeDocumentIndex: null,
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
  assert.match(elements.explorerTree.innerHTML, /<details class="tree-directory">/);
  assert.doesNotMatch(elements.explorerTree.innerHTML, /<details class="tree-directory" open>/);
  assert.match(elements.explorerTree.innerHTML, /tree-file-button-active/);
  assert.match(elements.explorerTree.innerHTML, /aria-current="page"/);
  assert.match(elements.explorerTree.innerHTML, /data-file-path="C:\/docs\/&lt;draft&gt;&amp;&quot;quote&quot;\.md"/);
});

runTest("renderExplorer escapes file and directory names before injecting HTML", () => {
  const elements = createElements();
  const explorer = {
    name: "docs",
    path: "C:/docs",
    children: [
      {
        name: "<img src=x onerror=alert(1)>",
        path: "C:/docs/malicious-dir",
        kind: "directory",
        children: [
          {
            name: "<script>alert(1)</script>.md",
            path: "C:/docs/malicious-dir/script.md",
            kind: "file",
            children: []
          }
        ]
      }
    ]
  };

  renderExplorer(elements, explorer, null);

  assert.doesNotMatch(elements.explorerTree.innerHTML, /<img src=x/);
  assert.doesNotMatch(elements.explorerTree.innerHTML, /<script>/);
  assert.match(elements.explorerTree.innerHTML, /&lt;img src=x onerror=alert\(1\)&gt;/);
  assert.match(elements.explorerTree.innerHTML, /&lt;script&gt;alert\(1\)&lt;\/script&gt;\.md/);
});

runTest("sameExplorer only reports changes when the tree content changes", () => {
  const left = {
    name: "docs",
    path: "C:/docs",
    children: [
      {
        name: "guide.md",
        path: "C:/docs/guide.md",
        kind: "file",
        children: []
      }
    ]
  };
  const right = {
    name: "docs",
    path: "C:/docs",
    children: [
      {
        name: "guide.md",
        path: "C:/docs/guide.md",
        kind: "file",
        children: []
      }
    ]
  };
  const changed = {
    ...right,
    children: [
      {
        name: "notes.md",
        path: "C:/docs/notes.md",
        kind: "file",
        children: []
      }
    ]
  };

  assert.equal(sameExplorer(left, right), true);
  assert.equal(sameExplorer(left, changed), false);
  assert.equal(sameExplorer(left, null), false);
});

runTest("renderDocumentTabs hides the strip when there are no documents and marks the active tab", () => {
  const elements = createElements();

  renderDocumentTabs(elements, []);
  assert.equal(elements.documentTabsPanel.hidden, true);
  assert.equal(elements.documentTabs.innerHTML, "");

  renderDocumentTabs(elements, [
    { label: "Untitled", isUntitled: true, hasUnsavedContent: true, isActive: true },
    { label: 'Roadmap <draft> & "quotes"', isUntitled: false, hasUnsavedContent: false, isActive: false }
  ]);

  assert.equal(elements.documentTabsPanel.hidden, false);
  assert.match(elements.documentTabs.innerHTML, /document-tab-active/);
  assert.match(elements.documentTabs.innerHTML, /data-document-index="0"/);
  assert.match(elements.documentTabs.innerHTML, /data-close-document-index="0"/);
  assert.match(elements.documentTabs.innerHTML, /aria-label="Close Untitled tab"/);
  assert.match(
    elements.documentTabs.innerHTML,
    /Roadmap &lt;draft&gt; &amp; &quot;quotes&quot;/
  );
  assert.match(
    elements.documentTabs.innerHTML,
    /aria-label="Close Roadmap &lt;draft&gt; &amp; &quot;quotes&quot; tab"/
  );
});

runTest("sameDocumentTabs checks label and active state", () => {
  const left = [
    { label: "Untitled", isUntitled: true, hasUnsavedContent: false, isActive: true },
    { label: "Guide", isUntitled: false, hasUnsavedContent: false, isActive: false }
  ];

  assert.equal(sameDocumentTabs(left, [...left]), true);
  assert.equal(
    sameDocumentTabs(left, [
      { label: "Untitled", isUntitled: true, hasUnsavedContent: false, isActive: false },
      { label: "Guide", isUntitled: false, hasUnsavedContent: false, isActive: true }
    ]),
    false
  );
  assert.equal(
    sameDocumentTabs(left, [
      { label: "Untitled", isUntitled: true, hasUnsavedContent: true, isActive: true },
      { label: "Guide", isUntitled: false, hasUnsavedContent: false, isActive: false }
    ]),
    false
  );
});

runTest("unsaved untitled helpers only flag non-empty untitled tabs", () => {
  const tabs = [
    { label: "Untitled", isUntitled: true, hasUnsavedContent: true, isActive: true },
    { label: "Guide", isUntitled: false, hasUnsavedContent: true, isActive: false },
    { label: "Untitled 2", isUntitled: true, hasUnsavedContent: false, isActive: false }
  ];

  assert.equal(hasUnsavedUntitledContent(tabs[0]), true);
  assert.equal(hasUnsavedUntitledContent(tabs[1]), false);
  assert.deepEqual(getUnsavedUntitledDocumentIndexes(tabs), [0]);
});

runTest("sameRecentPaths checks the recent file order", () => {
  assert.equal(sameRecentPaths(["a.md", "b.md"], ["a.md", "b.md"]), true);
  assert.equal(sameRecentPaths(["a.md", "b.md"], ["b.md", "a.md"]), false);
  assert.equal(sameRecentPaths(["a.md"], undefined), false);
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

runTest("renderWorkspaceFrame injects trusted HTML and shows the untitled editor", () => {
  const elements = createElements();

  renderWorkspaceFrame(
    elements,
    createWorkspace({
      documentTabs: [{ label: "Doc", isUntitled: false, hasUnsavedContent: false, isActive: true }],
      activeDocumentIndex: 0,
      explorer: {
        name: "docs",
        path: "C:/docs",
        children: []
      }
    })
  );

  assert.equal(elements.preview.innerHTML, "<h1>Doc</h1>");
  assert.equal(elements.preview.hidden, false);
  assert.equal(elements.editorPanel.hidden, true);
  assert.equal(elements.documentTabsPanel.hidden, false);
  assert.match(elements.documentTabs.innerHTML, /Doc/);

  renderWorkspaceFrame(
    elements,
    createWorkspace({
      documentTabs: [{ label: "Untitled", isUntitled: true, hasUnsavedContent: true, isActive: true }],
      activeDocumentIndex: 0,
      document: {
        title: "",
        html: "",
        sourceName: "",
        sourcePath: "",
        watching: false,
        trustModel: TRUSTED_PREVIEW_TRUST_MODEL
      },
      editorText: "# Draft"
    })
  );

  assert.equal(elements.editorPanel.hidden, false);
  assert.equal(elements.editor.value, "# Draft");
  assert.equal(elements.preview.hidden, true);
  assert.equal(elements.preview.innerHTML, "");
});

runTest("renderEditor only updates the text area when needed", () => {
  const elements = createElements();
  elements.editor.value = "# Existing";

  renderEditor(elements, "# Existing");
  assert.equal(elements.editorPanel.hidden, false);
  assert.equal(elements.editor.value, "# Existing");

  renderEditor(elements, null);
  assert.equal(elements.editorPanel.hidden, true);
  assert.equal(elements.editor.value, "");
});

runTest("shouldShowEditorPreview only enables untitled Mermaid previews", () => {
  assert.equal(
    shouldShowEditorPreview(
      createWorkspace({
        editorText: "flowchart TD\n  A --> B",
        document: {
          title: "Untitled",
          html: "<pre class=\"mermaid\">flowchart TD\n  A --&gt; B</pre>",
          sourceName: "",
          sourcePath: "",
          watching: false,
          trustModel: TRUSTED_PREVIEW_TRUST_MODEL
        }
      })
    ),
    true
  );

  assert.equal(
    shouldShowEditorPreview(
      createWorkspace({
        editorText: "# Draft",
        document: {
          title: "Untitled",
          html: "<h1>Draft</h1>",
          sourceName: "",
          sourcePath: "",
          watching: false,
          trustModel: TRUSTED_PREVIEW_TRUST_MODEL
        }
      })
    ),
    false
  );

  assert.equal(
    shouldShowEditorPreview(
      createWorkspace({
        editorText: null,
        document: {
          title: "Doc",
          html: "<pre class=\"mermaid\">flowchart TD\n  A --&gt; B</pre>",
          sourceName: "doc.md",
          sourcePath: "C:/docs/doc.md",
          watching: true,
          trustModel: TRUSTED_PREVIEW_TRUST_MODEL
        }
      })
    ),
    false
  );
});

runTest("clearPreview empties the preview region", () => {
  const preview = { innerHTML: "<p>stale</p>" };
  clearPreview(preview);
  assert.equal(preview.innerHTML, "");
});

runTest("preview zoom uses ctrl or cmd modified wheel gestures", () => {
  assert.equal(isPreviewZoomShortcut({ ctrlKey: true, metaKey: false }), true);
  assert.equal(isPreviewZoomShortcut({ ctrlKey: false, metaKey: true }), true);
  assert.equal(isPreviewZoomShortcut({ ctrlKey: false, metaKey: false }), false);
});

runTest("preview zoom steps and clamps the scale range", () => {
  assert.equal(DEFAULT_PREVIEW_SCALE, 1);
  assert.equal(getNextPreviewScale(1, -120), 1.1);
  assert.equal(getNextPreviewScale(1, 120), 0.9);
  assert.equal(getNextPreviewScale(MIN_PREVIEW_SCALE, 120), MIN_PREVIEW_SCALE);
  assert.equal(getNextPreviewScale(MAX_PREVIEW_SCALE, -120), MAX_PREVIEW_SCALE);
  assert.equal(clampPreviewScale(99), MAX_PREVIEW_SCALE);
  assert.equal(clampPreviewScale(0.01), MIN_PREVIEW_SCALE);
  assert.equal(clampPreviewScale(Number.NaN), DEFAULT_PREVIEW_SCALE);
});

runTest("applyPreviewScale writes the preview scale css variable", () => {
  const preview = {
    innerHTML: "",
    style: new FakeStyle()
  };

  const nextScale = applyPreviewScale(preview, 1.3);

  assert.equal(nextScale, 1.3);
  assert.equal(preview.style.getPropertyValue("--preview-scale"), "1.30");
});

runTest("clampContextMenuPosition keeps menus inside the viewport", () => {
  assert.deepEqual(
    clampContextMenuPosition({
      left: 120,
      top: 140,
      menuWidth: 160,
      menuHeight: 90,
      viewportWidth: 800,
      viewportHeight: 600
    }),
    { left: 120, top: 140 }
  );

  assert.deepEqual(
    clampContextMenuPosition({
      left: 780,
      top: 590,
      menuWidth: 160,
      menuHeight: 90,
      viewportWidth: 800,
      viewportHeight: 600
    }),
    { left: 632, top: 502 }
  );
});
