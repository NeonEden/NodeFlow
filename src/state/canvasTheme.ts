/**
 * Paleta del lienzo. Un solo lugar para cambiar de tema: por ahora "papel"
 * (fondo blanco, más claridad de lectura) y "noche" (el tema oscuro original),
 * que queda listo para volver con una línea.
 *
 * Los colores viven en JS y no en clases de Tailwind porque el mismo valor tiene que
 * llegar a tres consumidores distintos: el fondo SVG del lienzo (atributo `fill`),
 * los estilos inline de las tarjetas y los colores de las etiquetas de arista.
 */

import type { CSSProperties } from 'react';

/** Paleta de la estructura de la app (header, sidebar, modales, paneles). */
export interface ChromePalette {
  bg: string;
  bgA80: string;
  surface: string;
  surface2: string;
  border: string;
  borderStrong: string;
  text: string;
  textMuted: string;
  textDim: string;
}

export interface CanvasTheme {
  nombre: string;
  /** true si el lienzo es claro: habilita los ajustes de contraste (acentos y tintes). */
  claro: boolean;
  /** Fondo del área de trabajo. */
  canvasBg: string;
  /** Punto de la grilla. */
  grid: string;
  gridSize: number;
  /** Tarjeta. */
  cardBg: string;
  /** Superficie de tarjeta en modo cristal (semi-transparente + blur). */
  cardCristal: string;
  cardBorderMuted: string;
  title: string;
  body: string;
  muted: string;
  chipBg: string;
  chipBorder: string;
  maturityBg: string;
  maturityBorder: string;
  segmentOff: string;
  handleRing: string;
  /** Aristas: opacidad base en reposo. */
  edgeOpacity: number;
  edgeLabelBg: string;
  edgeLabelText: string;
  /** Zonas derivadas. */
  zonaTexto: string;
  zonaChipBg: string;
  /** Minimapa y HUD. */
  minimapBg: string;
  minimapMask: string;
  hudBg: string;
  hudBorder: string;
  hudText: string;
  hudHover: string;
  /** Estructura de la app (header, sidebar, modales, paneles). */
  chrome: ChromePalette;
}

export const PAPEL: CanvasTheme = {
  nombre: 'papel',
  claro: true,
  // Blanco puro para las tarjetas y un off-white mínimo para el lienzo: es lo que da
  // figura/fondo sin agregar sombras ni bordes extra (con blanco sobre blanco las
  // tarjetas se perdían en la vista general).
  canvasBg: '#f7f9fc',
  grid: '#dbe3ee',
  gridSize: 26,
  cardBg: 'rgba(255, 255, 255, 0.97)',
  cardCristal: 'rgba(255, 255, 255, 0.62)',
  cardBorderMuted: '#cbd5e1',
  title: '#0f172a',
  body: '#51607a',
  muted: '#7b8798',
  chipBg: 'rgba(241, 245, 249, 0.85)',
  chipBorder: '#e2e8f0',
  maturityBg: 'rgba(248, 250, 252, 0.95)',
  maturityBorder: '#e2e8f0',
  segmentOff: '#cbd5e1',
  handleRing: '#ffffff',
  edgeOpacity: 0.55,
  edgeLabelBg: '#ffffff',
  edgeLabelText: '#334155',
  zonaTexto: '#94a3b8',
  zonaChipBg: 'rgba(255, 255, 255, 0.92)',
  minimapBg: 'rgba(255, 255, 255, 0.92)',
  minimapMask: 'rgba(255, 255, 255, 0.78)',
  hudBg: 'rgba(255, 255, 255, 0.82)',
  hudBorder: '#e2e8f0',
  hudText: '#475569',
  hudHover: 'rgba(241, 245, 249, 0.9)',
  // Estructura clara: el lienzo es off-white, así que la estructura va en blanco
  // para que el lienzo se lea como el plano de trabajo y no al revés.
  chrome: {
    bg: '#ffffff',
    bgA80: 'rgba(255, 255, 255, 0.85)',
    surface: '#f8fafc',
    surface2: '#eef2f7',
    border: '#e5eaf1',
    borderStrong: '#cbd5e1',
    text: '#0f172a',
    // La estructura (sidebar/header) es oscura también en tema claro: estos textos van claros
    // en los dos temas. Medido: el sidebar daba 2.01 con un gris medio.
    textMuted: '#a9b6c7',
    textDim: '#94a3b8',
  },
};

export const NOCHE: CanvasTheme = {
  ...PAPEL,
  nombre: 'noche',
  claro: false,
  canvasBg: '#020617',
  grid: '#1e293b',
  cardBg: 'rgba(15, 23, 42, 0.95)',
  cardCristal: 'rgba(15, 23, 42, 0.6)',
  cardBorderMuted: '#64748b',
  title: '#f1f5f9',
  body: '#94a3b8',
  muted: '#64748b',
  chipBg: 'rgba(2, 6, 23, 0.7)',
  chipBorder: '#1e293b',
  maturityBg: 'rgba(2, 6, 23, 0.7)',
  maturityBorder: '#1e293b',
  segmentOff: '#1e293b',
  handleRing: '#020617',
  edgeOpacity: 0.42,
  edgeLabelBg: '#020617',
  edgeLabelText: '#cbd5e1',
  zonaTexto: '#64748b',
  zonaChipBg: 'rgba(2, 6, 23, 0.85)',
  minimapBg: 'rgba(2, 6, 23, 0.9)',
  minimapMask: 'rgba(2, 6, 23, 0.85)',
  hudBg: 'rgba(15, 23, 42, 0.8)',
  hudBorder: '#1e293b',
  hudText: '#cbd5e1',
  hudHover: 'rgba(30, 41, 59, 0.9)',
  // Estructura oscura: los valores originales de la app.
  chrome: {
    bg: '#020617',
    bgA80: 'rgba(2, 6, 23, 0.8)',
    surface: '#0f172a',
    surface2: '#1e293b',
    border: '#1e293b',
    borderStrong: '#334155',
    text: '#f1f5f9',
    textMuted: '#a9b6c7',
    // Antes #64748b (slate-500): 2.77 de contraste sobre el fondo casi negro de la estructura.
    // Los rótulos de 9-10 px no llegaban a AA. Medido con la auditoría: ahora 5.4.
    textDim: '#94a3b8',
  },
};

/** Cambiar acá para volver al lienzo oscuro. */
export const CANVAS_THEME: CanvasTheme = PAPEL;

/* --- Derivación de paletas: el usuario elige fondo y superficie de tarjeta ------- */

function aRgb(hex: string): [number, number, number] {
  const h = hex.replace('#', '');
  const n = parseInt(h.length === 3 ? h.split('').map((c) => c + c).join('') : h, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function aRgba(hex: string, alpha: number): string {
  const [r, g, b] = aRgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/** Mezcla dos colores (t = peso del segundo). */
export function mezclar(a: string, b: string, t: number): string {
  const [r1, g1, b1] = aRgb(a);
  const [r2, g2, b2] = aRgb(b);
  const m = (x: number, y: number) => Math.round(x + (y - x) * t);
  return `#${[m(r1, r2), m(g1, g2), m(b1, b2)]
    .map((v) => v.toString(16).padStart(2, '0'))
    .join('')}`;
}

/** Luminancia percibida (0 = negro, 1 = blanco). */
export function luminancia(hex: string): number {
  const [r, g, b] = aRgb(hex);
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255;
}

/**
 * Paleta completa a partir de un color de fondo: la grilla, la superficie de las
 * tarjetas y los grises de texto se derivan según si el fondo es claro u oscuro.
 * Es lo que permite ofrecer color libre sin dejar combinaciones ilegibles.
 */
export function temaDesdeFondo(bg: string, nombre = 'personalizado'): CanvasTheme {
  const claro = luminancia(bg) > 0.5;
  const base = claro ? PAPEL : NOCHE;
  // La estructura acompaña el fondo elegido (mismo tinte, misma jerarquía:
  // fondo < superficie < superficie2), así el lienzo y el chrome no se pelean.
  const cFondo = claro ? mezclar(bg, '#ffffff', 0.82) : bg;
  const cSuperficie = claro ? mezclar(bg, '#ffffff', 0.5) : mezclar(bg, '#ffffff', 0.07);
  const cSuperficie2 = claro ? mezclar(bg, '#ffffff', 0.24) : mezclar(bg, '#ffffff', 0.14);
  return {
    ...base,
    nombre,
    claro,
    canvasBg: bg,
    grid: claro ? mezclar(bg, '#0f172a', 0.16) : mezclar(bg, '#ffffff', 0.2),
    minimapBg: claro ? 'rgba(255,255,255,0.93)' : 'rgba(15,23,42,0.9)',
    minimapMask: claro ? 'rgba(255,255,255,0.8)' : 'rgba(2,6,23,0.85)',
    hudBg: claro ? 'rgba(255,255,255,0.85)' : 'rgba(15,23,42,0.85)',
    chrome: {
      ...base.chrome,
      bg: cFondo,
      bgA80: aRgba(cFondo, 0.85),
      surface: cSuperficie,
      surface2: cSuperficie2,
      border: claro ? mezclar(bg, '#0f172a', 0.08) : mezclar(bg, '#ffffff', 0.16),
      borderStrong: claro ? mezclar(bg, '#0f172a', 0.2) : mezclar(bg, '#ffffff', 0.26),
    },
  };
}

/**
 * Expone la paleta como variables CSS en el contenedor del lienzo: así los
 * componentes pueden usar clases (y sus `:hover`) en vez de estilos inline, que
 * ganarían siempre y romperían los estados. Único origen de verdad: este archivo.
 */
export function temaVars(t: CanvasTheme): CSSProperties {
  return {
    '--nf-card-bg': t.cardBg,
    '--nf-title': t.title,
    '--nf-body': t.body,
    '--nf-muted': t.muted,
    '--nf-chip-bg': t.chipBg,
    '--nf-chip-border': t.chipBorder,
    '--nf-maturity-bg': t.maturityBg,
    '--nf-maturity-border': t.maturityBorder,
    '--nf-segment-off': t.segmentOff,
    '--nf-edge-label-bg': t.edgeLabelBg,
    '--nf-edge-label-text': t.edgeLabelText,
    '--nf-hud-bg': t.hudBg,
    '--nf-hud-border': t.hudBorder,
    '--nf-hud-text': t.hudText,
    '--nf-hud-hover': t.hudHover,
    '--nf-zona-texto': t.zonaTexto,
    // Estructura
    '--nf-chrome-bg': t.chrome.bg,
    '--nf-chrome-bg-80': t.chrome.bgA80,
    '--nf-chrome-surface': t.chrome.surface,
    '--nf-chrome-surface-2': t.chrome.surface2,
    '--nf-chrome-border': t.chrome.border,
    '--nf-chrome-border-strong': t.chrome.borderStrong,
    '--nf-chrome-text': t.chrome.text,
    '--nf-chrome-text-muted': t.chrome.textMuted,
    '--nf-chrome-text-dim': t.chrome.textDim,
  } as CSSProperties;
}
