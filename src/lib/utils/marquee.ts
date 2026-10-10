/* Utilidades para el efecto marquee del título */

export interface MarqueeConfig {
  distance: number;
  duration: number;
}

export function computeMarquee(
  textEl: HTMLElement,
  containerEl: HTMLElement,
): MarqueeConfig | null {
  const overflow = textEl.scrollWidth - containerEl.clientWidth;
  if (overflow <= 8) return null;
  return {
    distance: overflow,
    // Ritmo pausado: 12 s de base más ~15 px de recorrido extra por segundo.
    duration: Math.min(48, Math.max(16, 12 + overflow / 15)),
  };
}
