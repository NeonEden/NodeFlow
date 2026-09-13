/**
 * Auditoría de contraste (WCAG 2.1) sobre la UI REAL.
 *
 * El problema de fondo: la app tiene ~1100 clases de color hardcodeadas y el tema sólo remapea la
 * escala `slate`. Cualquier acento (`text-amber-100`, `text-cyan-300`…) sobre una superficie clara
 * queda casi invisible, y eso no se detecta mirando: hay que medirlo.
 *
 * Esto recorre el DOM, calcula el fondo REAL de cada texto (componiendo los fondos translúcidos de
 * sus ancestros), convierte todo a sRGB y devuelve lo que no llega al mínimo.
 *
 * Uso en la app (consola del WebView o desde la UI):  nfContraste()
 */

export interface Violacion {
  ratio: number;
  texto: string;
  color: string;
  fondo: string;
  tamano: number;
  negrita: boolean;
  selector: string;
  minimo: number;
}

const MIN_CHICO = 4.5; // AA para texto normal
const MIN_GRANDE = 3.0; // AA para texto grande o negrita

let lienzo: HTMLCanvasElement | null = null;
function aSrgb(color: string, sobre?: string): [number, number, number] | null {
  if (!lienzo) {
    lienzo = document.createElement('canvas');
    lienzo.width = lienzo.height = 1;
  }
  const ctx = lienzo.getContext('2d', { willReadFrequently: true });
  if (!ctx) return null;
  try {
    ctx.clearRect(0, 0, 1, 1);
    if (sobre) {
      ctx.fillStyle = sobre;
      ctx.fillRect(0, 0, 1, 1);
    }
    ctx.fillStyle = color;
    ctx.fillRect(0, 0, 1, 1);
    const d = ctx.getImageData(0, 0, 1, 1).data;
    return d[3] === 0 ? null : [d[0], d[1], d[2]];
  } catch {
    return null;
  }
}

function luminancia([r, g, b]: [number, number, number]): number {
  const f = (v: number) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

function ratio(a: [number, number, number], b: [number, number, number]): number {
  const la = luminancia(a);
  const lb = luminancia(b);
  const hi = Math.max(la, lb);
  const lo = Math.min(la, lb);
  return (hi + 0.05) / (lo + 0.05);
}

/** Fondo efectivo de un elemento: sube por los ancestros componiendo lo translúcido. */
function fondoEfectivo(el: Element): [number, number, number] {
  const capas: string[] = [];
  let actual: Element | null = el;
  while (actual) {
    const bg = getComputedStyle(actual).backgroundColor;
    if (bg && bg !== 'rgba(0, 0, 0, 0)' && bg !== 'transparent') {
      capas.unshift(bg);
      const c = aSrgb(bg);
      // opaco: ya no hace falta seguir subiendo
      const m = bg.match(/rgba?\(([^)]+)\)/);
      const alfa = m ? (m[1].split(',')[3] ? parseFloat(m[1].split(',')[3]) : 1) : 1;
      if (c && alfa >= 0.999) break;
    }
    actual = actual.parentElement;
  }
  let base: [number, number, number] = [255, 255, 255];
  for (const c of capas) {
    const s = aSrgb(c, `rgb(${base.join(',')})`);
    if (s) base = s;
  }
  return base;
}

function visible(el: Element): boolean {
  const s = getComputedStyle(el);
  if (s.display === 'none' || s.visibility === 'hidden' || parseFloat(s.opacity) < 0.15) return false;
  const r = el.getBoundingClientRect();
  return r.width > 1 && r.height > 1;
}

function textoPropio(el: Element): string {
  let t = '';
  el.childNodes.forEach((n) => {
    if (n.nodeType === Node.TEXT_NODE) t += n.textContent || '';
  });
  return t.trim();
}

function selectorDe(el: Element): string {
  const id = el.id ? `#${el.id}` : '';
  const cls = (el.className || '').toString().split(/\s+/).filter(Boolean).slice(0, 3).join('.');
  return `${el.tagName.toLowerCase()}${id}${cls ? '.' + cls : ''}`;
}

/** Devuelve los textos que no llegan al mínimo de contraste (ordenados del peor al mejor). */
export function medirContraste(root: ParentNode = document.body): Violacion[] {
  const out: Violacion[] = [];
  const vistos = new Set<string>();
  root.querySelectorAll('*').forEach((el) => {
    const t = textoPropio(el);
    if (!t || t.length < 2) return;
    if (!visible(el)) return;
    const s = getComputedStyle(el);
    const color = aSrgb(s.color);
    if (!color) return;
    // color realmente percibido: el texto puede tener alfa
    const fondo = fondoEfectivo(el);
    const colorCompuesto = aSrgb(s.color, `rgb(${fondo.join(',')})`) || color;
    const tamano = parseFloat(s.fontSize);
    const peso = parseInt(s.fontWeight || '400', 10);
    const negrita = peso >= 600;
    const minimo = tamano >= 24 || (tamano >= 18.66 && negrita) ? MIN_GRANDE : MIN_CHICO;
    const r = ratio(colorCompuesto, fondo);
    if (r < minimo) {
      const clave = `${selectorDe(el)}|${t.slice(0, 20)}`;
      if (vistos.has(clave)) return;
      vistos.add(clave);
      out.push({
        ratio: Math.round(r * 100) / 100,
        texto: t.slice(0, 60),
        color: `rgb(${colorCompuesto.join(',')})`,
        fondo: `rgb(${fondo.join(',')})`,
        tamano,
        negrita,
        selector: selectorDe(el),
        minimo,
      });
    }
  });
  return out.sort((a, b) => a.ratio - b.ratio);
}

/** Resumen corto para mostrar en pantalla o en la consola. */
export function resumenContraste(violaciones: Violacion[]): string {
  if (!violaciones.length) return 'Contraste OK: ningún texto por debajo del mínimo.';
  const lineas = violaciones
    .slice(0, 12)
    .map((v) => `  ${v.ratio.toFixed(2)} (mín ${v.minimo}) · ${v.selector} · «${v.texto}» ${v.color} sobre ${v.fondo}`);
  return `Contraste: ${violaciones.length} texto(s) por debajo del mínimo:\n${lineas.join('\n')}`;
}
