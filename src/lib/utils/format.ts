export function pathSummary(path: string[] | string, maxDepth: number): string {
  const arr = Array.isArray(path)
    ? [...path]
    : String(path).split(/[\\/]/).filter(Boolean);
  arr.pop();
  if (!arr.length) return "";
  if (!maxDepth || maxDepth <= 0) return arr.join(" / ");
  if (arr.length <= maxDepth) return arr.join(" / ");
  return `… / ${arr.slice(-maxDepth).join(" / ")}`;
}

export function displayTitle(title: string, showExt: boolean): string {
  if (showExt) return title;
  return title.replace(/\.[a-zA-Z0-9]{1,6}$/, "");
}

export function formatBytes(bytes: number | null): string {
  if (bytes == null) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

export function sectionTitle(section: number): string {
  const titles = [
    "Portada", "Resumen", "Introducción", "Estado del arte",
    "Marco conceptual", "Metodología", "Implementación", "Experimentos",
    "Resultados", "Discusión", "Conclusiones", "Referencias",
  ];
  return titles[(section - 1) % titles.length] ?? "Sección";
}
