export type ShortcutAction =
  | "new"
  | "open"
  | "open-folder"
  | "save"
  | "save-as"
  | "close-tab"
  | "next-tab"
  | "previous-tab";

export interface ShortcutEventLike {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

export function isMacLikePlatform(platform: string | undefined | null): boolean {
  return /mac|iphone|ipad|ipod/i.test(platform ?? "");
}

export function getShortcutAction(event: ShortcutEventLike): ShortcutAction | undefined {
  const key = event.key.toLowerCase();
  const hasPrimaryModifier = event.ctrlKey || event.metaKey;

  if (key === "tab") {
    if (!event.ctrlKey || event.altKey || event.metaKey) {
      return undefined;
    }

    return event.shiftKey ? "previous-tab" : "next-tab";
  }

  if (!hasPrimaryModifier || event.altKey) {
    return undefined;
  }

  if (event.shiftKey) {
    if (key === "o") {
      return "open-folder";
    }

    if (key === "s") {
      return "save-as";
    }

    return undefined;
  }

  switch (key) {
    case "n":
      return "new";
    case "o":
      return "open";
    case "s":
      return "save";
    case "w":
      return "close-tab";
    default:
      return undefined;
  }
}

export function getShortcutLabel(action: ShortcutAction, isMacLike: boolean): string {
  const primaryModifier = isMacLike ? "Cmd" : "Ctrl";

  switch (action) {
    case "new":
      return `${primaryModifier}+N`;
    case "open":
      return `${primaryModifier}+O`;
    case "open-folder":
      return `${primaryModifier}+Shift+O`;
    case "save":
      return `${primaryModifier}+S`;
    case "save-as":
      return `${primaryModifier}+Shift+S`;
    case "close-tab":
      return `${primaryModifier}+W`;
    case "next-tab":
      return "Ctrl+Tab";
    case "previous-tab":
      return "Ctrl+Shift+Tab";
    default:
      return "";
  }
}
