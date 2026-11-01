const SUPPORTED_EXTENSIONS = ["pdf"] as const;

export function isSupportedDocument(path: string): boolean {
  const extension = path.toLowerCase().split(".").pop() ?? "";
  return (SUPPORTED_EXTENSIONS as readonly string[]).includes(extension);
}
