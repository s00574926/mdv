export function buildDefaultSavePath(directory: string | undefined, fileName: string): string {
  if (!directory) {
    return fileName;
  }

  return directory.endsWith("\\") || directory.endsWith("/")
    ? `${directory}${fileName}`
    : `${directory}${defaultPathSeparator(directory)}${fileName}`;
}

function defaultPathSeparator(directory: string): "\\" | "/" {
  return directory.lastIndexOf("/") > directory.lastIndexOf("\\") ? "/" : "\\";
}
