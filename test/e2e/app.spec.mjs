import { expect, test } from "@playwright/test";

async function harnessState(page) {
  return page.evaluate(() => window.__MDV_E2E__.getState());
}

async function commandNames(page) {
  return page.evaluate(() => window.__MDV_E2E__.getState().commands.map((entry) => entry.command));
}

async function openAppMenu(page) {
  await page.getByRole("button", { name: "Open app menu" }).click();
}

async function newDocument(page) {
  await openAppMenu(page);
  await page.getByRole("button", { name: "New" }).click();
}

async function openMarkdown(page, path = "C:/Users/Test/Documents/opened.md") {
  await page.evaluate((nextPath) => {
    window.__MDV_E2E__.setNextOpenPath(nextPath);
  }, path);
  await openAppMenu(page);
  await page.getByRole("button", { name: "Open..." }).click();
}

async function openFolder(page, path = "C:/Users/Test/Documents/project") {
  await page.evaluate((nextPath) => {
    window.__MDV_E2E__.setNextOpenPath(nextPath);
  }, path);
  await openAppMenu(page);
  await page.getByRole("button", { name: "Open Folder..." }).click();
}

async function fillEditor(page, markdown) {
  const editor = page.getByLabel("Untitled Markdown editor");
  await editor.fill(markdown);
  await expect(editor).toHaveValue(markdown);
}

function tabButton(page, label) {
  return page.getByRole("button", { name: label, exact: true });
}

test("opens and closes the app menu with click and Escape", async ({ page }) => {
  await page.goto("/");

  const menu = page.locator("#titlebar-menu");
  await expect(menu).toBeHidden();

  await openAppMenu(page);
  await expect(menu).toBeVisible();
  await expect(page.getByRole("button", { name: "Open app menu" })).toHaveAttribute(
    "aria-expanded",
    "true"
  );

  await page.keyboard.press("Escape");
  await expect(menu).toBeHidden();
  await expect(page.getByRole("button", { name: "Open app menu" })).toHaveAttribute(
    "aria-expanded",
    "false"
  );
});

test("creates a new untitled document from the menu", async ({ page }) => {
  await page.goto("/");

  await newDocument(page);

  await expect(page.getByLabel("Untitled Markdown editor")).toBeVisible();
  await expect(tabButton(page, "Untitled")).toBeVisible();
  await expect.poll(() => commandNames(page)).toEqual(["current_workspace", "new_document"]);
});

test("creates a new untitled document with the keyboard shortcut", async ({ page }) => {
  await page.goto("/");

  await page.keyboard.press("Control+N");

  await expect(page.getByLabel("Untitled Markdown editor")).toBeVisible();
  await expect(tabButton(page, "Untitled")).toBeVisible();
  await expect.poll(() => commandNames(page)).toEqual(["current_workspace", "new_document"]);
});

test("edits an untitled document and pushes the content update", async ({ page }) => {
  await page.goto("/");
  await newDocument(page);

  await fillEditor(page, "# Draft\n\nBody copy");

  await expect.poll(() => commandNames(page)).toContain("update_document_content");
  await expect
    .poll(async () => (await harnessState(page)).documents[0]?.markdown)
    .toBe("# Draft\n\nBody copy");
});

test("saves an untitled document with Ctrl+S", async ({ page }) => {
  await page.goto("/");
  await newDocument(page);
  await fillEditor(page, "# Draft\n\nBody copy");

  await page.keyboard.press("Control+S");

  await expect(tabButton(page, "draft.md")).toBeVisible();
  await expect.poll(() => commandNames(page)).toEqual([
    "current_workspace",
    "new_document",
    "update_document_content",
    "save_active_document_to_path"
  ]);
});

test("saves an untitled document from the menu Save action", async ({ page }) => {
  await page.goto("/");
  await newDocument(page);
  await fillEditor(page, "# Menu Save");

  await openAppMenu(page);
  await page.getByRole("button", { name: "Save", exact: true }).click();

  await expect(tabButton(page, "draft.md")).toBeVisible();
  await expect.poll(() => commandNames(page)).toContain("save_active_document_to_path");
});

test("saves an untitled document from the menu Save As action", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => {
    window.__MDV_E2E__.setNextSavePath("C:/Users/Test/Documents/save-as.md");
  });
  await newDocument(page);
  await fillEditor(page, "# Save As");

  await openAppMenu(page);
  await page.getByRole("button", { name: /^Save As/ }).click();

  await expect(tabButton(page, "save-as.md")).toBeVisible();
  await expect
    .poll(async () => (await harnessState(page)).documents[0]?.path)
    .toBe("C:/Users/Test/Documents/save-as.md");
});

test("opens a markdown document from the menu", async ({ page }) => {
  await page.goto("/");

  await openMarkdown(page, "C:/Users/Test/Documents/opened.md");

  await expect(tabButton(page, "opened.md")).toBeVisible();
  await expect(page.locator("#preview h1")).toHaveText("opened");
  await expect.poll(() => commandNames(page)).toEqual([
    "current_workspace",
    "open_markdown_dialog"
  ]);
});

test("opens a markdown document with the keyboard shortcut", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => {
    window.__MDV_E2E__.setNextOpenPath("C:/Users/Test/Documents/shortcut.md");
  });

  await page.keyboard.press("Control+O");

  await expect(tabButton(page, "shortcut.md")).toBeVisible();
  await expect(page.locator("#preview h1")).toHaveText("shortcut");
  await expect.poll(() => commandNames(page)).toEqual([
    "current_workspace",
    "open_markdown_dialog"
  ]);
});

test("opens a folder from the menu", async ({ page }) => {
  await page.goto("/");

  await openFolder(page);

  await expect(page.getByRole("tree")).toBeVisible();
  await expect(page.getByRole("treeitem", { name: "guide.md" })).toBeVisible();
  await expect.poll(() => commandNames(page)).toEqual(["current_workspace", "open_folder"]);
});

test("opens a folder with the keyboard shortcut", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => {
    window.__MDV_E2E__.setNextOpenPath("C:/Users/Test/Documents/shortcut-project");
  });

  await page.keyboard.press("Control+Shift+O");

  await expect(page.getByRole("tree")).toBeVisible();
  await expect(page.getByRole("treeitem", { name: "guide.md" })).toBeVisible();
  await expect.poll(() => commandNames(page)).toEqual(["current_workspace", "open_folder"]);
});

test("opens a dropped markdown file", async ({ page }) => {
  await page.goto("/");

  await page.evaluate(async () => {
    await window.__MDV_E2E__.emitWindowDragDrop(["C:/Users/Test/Documents/dropped.md"]);
  });

  await expect(tabButton(page, "dropped.md")).toBeVisible();
  await expect(page.locator("#preview h1")).toHaveText("dropped");
  await expect(page.locator("#drop-overlay")).toBeHidden();
  await expect.poll(() => commandNames(page)).toEqual(["current_workspace", "open_dropped_path"]);
});

test("opens a dropped folder", async ({ page }) => {
  await page.goto("/");

  await page.evaluate(async () => {
    await window.__MDV_E2E__.emitWindowDragDrop(["C:/Users/Test/Documents/dropped-project"]);
  });

  await expect(page.getByRole("tree")).toBeVisible();
  await expect(page.getByRole("treeitem", { name: "guide.md" })).toBeVisible();
  await expect(page.locator("#drop-overlay")).toBeHidden();
  await expect.poll(() => commandNames(page)).toEqual(["current_workspace", "open_dropped_path"]);
});

test("shows a drop target while dragging files over the window", async ({ page }) => {
  await page.goto("/");

  await page.evaluate(() => {
    const dataTransfer = new DataTransfer();
    dataTransfer.setData("application/x-mdv-path", "C:/Users/Test/Documents/hover.md");
    document.dispatchEvent(new DragEvent("dragenter", { bubbles: true, cancelable: true, dataTransfer }));
  });

  await expect(page.locator("#drop-overlay")).toBeVisible();
  await expect(page.locator("#drop-overlay-detail")).toHaveText("hover.md");
});

test("selects a markdown file from the explorer", async ({ page }) => {
  await page.goto("/");
  await openFolder(page);

  await page.getByRole("treeitem", { name: "guide.md" }).click();

  await expect(page.locator("#preview")).toContainText("guide");
  await expect(page.locator("#preview h1")).toHaveText("guide");
  await expect.poll(() => commandNames(page)).toEqual([
    "current_workspace",
    "open_folder",
    "select_explorer_file"
  ]);
});

test("switches document tabs by clicking a tab", async ({ page }) => {
  await page.goto("/");
  await openMarkdown(page, "C:/Users/Test/Documents/first.md");
  await openMarkdown(page, "C:/Users/Test/Documents/second.md");

  await tabButton(page, "first.md").click();

  await expect(tabButton(page, "first.md")).toHaveAttribute("aria-current", "page");
  await expect(page.locator("#preview h1")).toHaveText("first");
  await expect.poll(() => commandNames(page)).toEqual([
    "current_workspace",
    "open_markdown_dialog",
    "open_markdown_dialog",
    "select_document"
  ]);
});

test("switches document tabs with next and previous shortcuts", async ({ page }) => {
  await page.goto("/");
  await openMarkdown(page, "C:/Users/Test/Documents/first.md");
  await openMarkdown(page, "C:/Users/Test/Documents/second.md");

  await page.keyboard.press("Control+Shift+Tab");
  await expect(tabButton(page, "first.md")).toHaveAttribute("aria-current", "page");

  await page.keyboard.press("Control+Tab");
  await expect(tabButton(page, "second.md")).toHaveAttribute("aria-current", "page");
  await expect.poll(() => commandNames(page)).toEqual([
    "current_workspace",
    "open_markdown_dialog",
    "open_markdown_dialog",
    "select_document",
    "select_document"
  ]);
});

test("closes a clean tab with the tab close button", async ({ page }) => {
  await page.goto("/");
  await openMarkdown(page, "C:/Users/Test/Documents/closable.md");

  await page.getByRole("button", { name: "Close closable.md tab" }).click();

  await expect(tabButton(page, "closable.md")).toHaveCount(0);
  await expect.poll(() => commandNames(page)).toEqual([
    "current_workspace",
    "open_markdown_dialog",
    "close_document"
  ]);
});

test("keeps an unsaved tab open when the close prompt is canceled", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => {
    window.__MDV_E2E__.setNextMessageResult("Cancel");
  });
  await newDocument(page);
  await fillEditor(page, "# Keep Me");

  await page.getByRole("button", { name: "Close Untitled tab" }).click();

  await expect(tabButton(page, "Untitled")).toBeVisible();
  await expect.poll(() => commandNames(page)).toEqual([
    "current_workspace",
    "new_document",
    "update_document_content"
  ]);
});

test("discards an unsaved tab when the close prompt is discarded", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => {
    window.__MDV_E2E__.setNextMessageResult("Discard");
  });
  await newDocument(page);
  await fillEditor(page, "# Discard Me");

  await page.getByRole("button", { name: "Close Untitled tab" }).click();

  await expect(tabButton(page, "Untitled")).toHaveCount(0);
  await expect.poll(() => commandNames(page)).toEqual([
    "current_workspace",
    "new_document",
    "update_document_content",
    "close_document"
  ]);
});

test("opens a document from the recent files submenu", async ({ page }) => {
  await page.goto("/");
  await openMarkdown(page, "C:/Users/Test/Documents/recent-a.md");

  await openAppMenu(page);
  await page.getByRole("button", { name: "Open Recent", exact: true }).click();
  await page.getByRole("button", { name: "1. recent-a.md" }).click();

  await expect(page.getByRole("button", { name: "recent-a.md", exact: true })).toHaveCount(2);
  await expect.poll(() => commandNames(page)).toEqual([
    "current_workspace",
    "open_markdown_dialog",
    "open_recent_index"
  ]);
});

test("hydrates relative local assets and opens local markdown links", async ({ page }) => {
  await page.goto("/");

  await page.evaluate(async () => {
    await window.__MDV_E2E__.emitWorkspaceUpdated({
      document: {
        title: "Local refs",
        html: `
          <p><img alt="Diagram" src="mdv-local-asset:C%3A%5CUsers%5CTest%5CDocuments%5Cassets%5Cdiagram.png"></p>
          <p><a href="mdv-local-markdown:C%3A%5CUsers%5CTest%5CDocuments%5Cnext.md">Next</a></p>
        `,
        sourceName: "local-refs.md",
        sourcePath: "C:/Users/Test/Documents/local-refs.md",
        watching: true,
        trustModel: "trusted-local-markdown-preview"
      },
      editorText: null,
      currentFilePath: "C:/Users/Test/Documents/local-refs.md",
      explorer: null,
      explorerUpdated: false,
      recentPaths: [],
      documentTabs: [{ label: "local-refs.md", isUntitled: false, hasUnsavedContent: false, isActive: true }],
      activeDocumentIndex: 0
    });
  });

  await expect(page.getByAltText("Diagram")).toHaveAttribute(
    "src",
    "asset://C%3A%5CUsers%5CTest%5CDocuments%5Cassets%5Cdiagram.png"
  );

  await page.getByRole("link", { name: "Next" }).click();

  await expect(tabButton(page, "next.md")).toBeVisible();
  await expect.poll(() => commandNames(page)).toContain("open_markdown");
});

test("finds preview text and jumps to document headings", async ({ page }) => {
  await page.goto("/");

  await page.evaluate(async () => {
    const filler = Array.from({ length: 40 }, (_, index) => `<p>Filler line ${index + 1}</p>`).join("");
    await window.__MDV_E2E__.emitWorkspaceUpdated({
      document: {
        title: "Navigation",
        html: `
          <h1>Overview</h1>
          <p>Alpha appears near the top.</p>
          ${filler}
          <h2>Details</h2>
          <p>Alpha appears again near the details.</p>
        `,
        sourceName: "navigation.md",
        sourcePath: "C:/Users/Test/Documents/navigation.md",
        watching: true,
        trustModel: "trusted-local-markdown-preview"
      },
      editorText: null,
      currentFilePath: "C:/Users/Test/Documents/navigation.md",
      explorer: null,
      explorerUpdated: false,
      recentPaths: [],
      documentTabs: [{ label: "navigation.md", isUntitled: false, hasUnsavedContent: false, isActive: true }],
      activeDocumentIndex: 0
    });
  });

  await page.keyboard.press("Control+F");

  await expect(page.locator("#document-nav-panel")).toBeVisible();
  await expect(page.getByRole("button", { name: "Overview" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Details" })).toBeVisible();

  await page.getByLabel("Find in document").fill("Alpha");
  await expect(page.locator("mark.document-find-match")).toHaveCount(2);
  await expect(page.locator("#document-find-count")).toHaveText("1/2");

  await page.keyboard.press("Enter");
  await expect(page.locator("#document-find-count")).toHaveText("2/2");

  await page.getByRole("button", { name: "Details" }).click();
  await expect(page.locator("#preview h2")).toHaveAttribute("id", "details");
  await expect(page.getByRole("button", { name: "Details" })).toHaveAttribute("aria-current", "location");
});

test("toggles the application theme from the titlebar", async ({ page }) => {
  await page.goto("/");

  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.getByRole("button", { name: "Switch to dark theme" }).click();

  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(page.getByRole("button", { name: "Switch to light theme" })).toBeVisible();
});

test("toggles maximize and restore from the titlebar", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Maximize window" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-window-maximized", "true");
  await expect(page.getByRole("button", { name: "Restore window" })).toBeVisible();

  await page.getByRole("button", { name: "Restore window" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-window-maximized", "false");
  await expect(page.getByRole("button", { name: "Maximize window" })).toBeVisible();
});

test("minimizes the window from the titlebar", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Minimize window" }).click();

  await expect.poll(async () => (await harnessState(page)).windowMinimizeCount).toBe(1);
});

test("requests app exit from the close button when there are no unsaved changes", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Close window" }).click();

  await expect.poll(async () => (await harnessState(page)).exitRequested).toBe(true);
  await expect.poll(() => commandNames(page)).toEqual(["current_workspace", "exit_app"]);
});

test("does not exit from the close button when the unsaved prompt is canceled", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => {
    window.__MDV_E2E__.setNextMessageResult("Cancel");
  });
  await newDocument(page);
  await fillEditor(page, "# Not Closing");

  await page.getByRole("button", { name: "Close window" }).click();

  await expect.poll(async () => (await harnessState(page)).exitRequested).toBe(false);
  await expect(tabButton(page, "Untitled")).toBeVisible();
});

test("closes the titlebar menu by clicking outside it", async ({ page }) => {
  await page.goto("/");

  await openAppMenu(page);
  await expect(page.locator("#titlebar-menu")).toBeVisible();
  await page.locator("#preview").click();

  await expect(page.locator("#titlebar-menu")).toBeHidden();
});

test("requests app exit from the Quit menu item", async ({ page }) => {
  await page.goto("/");

  await openAppMenu(page);
  await page.getByRole("button", { name: "Quit" }).click();

  await expect.poll(async () => (await harnessState(page)).exitRequested).toBe(true);
  await expect.poll(() => commandNames(page)).toEqual(["current_workspace", "exit_app"]);
});

test("closes a clean tab with the close-tab keyboard shortcut", async ({ page }) => {
  await page.goto("/");
  await openMarkdown(page, "C:/Users/Test/Documents/shortcut-close.md");

  await page.keyboard.press("Control+W");

  await expect(tabButton(page, "shortcut-close.md")).toHaveCount(0);
  await expect.poll(() => commandNames(page)).toEqual([
    "current_workspace",
    "open_markdown_dialog",
    "close_document"
  ]);
});

test("shows the empty recent-files state from the Open Recent submenu", async ({ page }) => {
  await page.goto("/");

  await openAppMenu(page);
  await page.getByRole("button", { name: "Open Recent", exact: true }).click();

  const emptyRecentButton = page.getByRole("button", { name: "No Recent Files" });
  await expect(emptyRecentButton).toBeVisible();
  await expect(emptyRecentButton).toBeDisabled();
});

test("zooms the preview with the platform zoom gesture", async ({ page }) => {
  await page.goto("/");
  await openMarkdown(page, "C:/Users/Test/Documents/zoom.md");

  const preview = page.locator("#preview");
  await preview.hover();
  await page.keyboard.down("Control");
  await page.mouse.wheel(0, -120);
  await page.keyboard.up("Control");

  await expect(preview).toHaveCSS("--preview-scale", "1.10");
});

test("toggles maximize by double-clicking the titlebar", async ({ page }) => {
  await page.goto("/");

  const titlebar = page.locator(".titlebar");
  const box = await titlebar.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.dblclick(box.x + box.width / 2, box.y + box.height / 2);

  await expect(page.locator("html")).toHaveAttribute("data-window-maximized", "true");
  await expect(page.getByRole("button", { name: "Restore window" })).toBeVisible();
});

test("starts window dragging after dragging the titlebar", async ({ page }) => {
  await page.goto("/");

  const titlebarCaption = page.locator(".titlebar-caption");
  const box = await titlebarCaption.boundingBox();
  expect(box).not.toBeNull();

  await page.mouse.move(box.x + 10, box.y + 10);
  await page.mouse.down();
  await page.mouse.move(box.x + 40, box.y + 10);
  await page.mouse.up();

  await expect.poll(async () => (await harnessState(page)).windowDragCount).toBe(1);
});
