/**
 * Tests del borrador del turno de voz. Lo que se prueba acá es la REGLA, no el render: cuándo el
 * lienzo dibuja algo y cuándo no. Los casos salen de frases reales de la demo (ruido de arranque,
 * idea con dos palabras, monólogo largo con corrección).
 */
import { describe, expect, it } from 'vitest';
import {
  CLASE_FANTASMA,
  esCorreccion,
  esIdeaEnVivo,
  ID_FANTASMA,
  MIN_PALABRAS,
  nodoFantasma,
  OFFSET_FANTASMA,
  PALABRAS_TITULO,
  palabras,
  tituloDelBorrador,
} from './draftVoz';

describe('esIdeaEnVivo — el ruido no dibuja', () => {
  it('no dibuja con texto vacío ni con espacios', () => {
    expect(esIdeaEnVivo('')).toBe(false);
    expect(esIdeaEnVivo('   ')).toBe(false);
  });

  it('no dibuja con una o dos palabras (arranque de frase)', () => {
    expect(esIdeaEnVivo('un sinte')).toBe(false);
    expect(esIdeaEnVivo('un sintetizador visual')).toBe(true);
  });

  it('dibuja a partir del mínimo declarado', () => {
    expect(MIN_PALABRAS).toBe(3);
    expect(esIdeaEnVivo('nodo')).toBe(false);
    expect(esIdeaEnVivo('quiero un nodo')).toBe(true);
  });
});

describe('tituloDelBorrador — el encabezado de una idea a medio decir', () => {
  it('recorta a PALABRAS_TITULO y no repite el texto completo', () => {
    const largo = 'quiero un sintetizador visual que se conecte a un nodo de código y después a una salida de audio';
    const t = tituloDelBorrador(largo);
    expect(t.split(' ').length).toBe(PALABRAS_TITULO);
    expect(t).toBe('quiero un sintetizador visual que se conecte');
  });

  it('saca la puntuación de los bordes', () => {
    expect(tituloDelBorrador('«un sinte visual de prueba»')).toBe('un sinte visual de prueba');
    expect(tituloDelBorrador('una idea,')).toBe('una idea');
  });

  it('no devuelve vacío si el texto era sólo puntuación', () => {
    expect(tituloDelBorrador('...')).toBe('...');
    expect(tituloDelBorrador('')).toBe('');
  });

  it('normaliza espacios múltiples (lo que llega del parcial)', () => {
    expect(tituloDelBorrador('  una   idea   nueva  ')).toBe('una idea nueva');
  });
});

describe('esCorreccion — «nodo» no es un arrepentimiento', () => {
  it('no marca palabras que empiezan con no-', () => {
    expect(esCorreccion('nodo de código conectado al audio')).toBe(false);
    expect(esCorreccion('norte estratégico del proyecto')).toBe(false);
    expect(esCorreccion('no sé bien qué poner acá')).toBe(false);
  });

  it('marca los marcadores inequívocos', () => {
    expect(esCorreccion('no, mejor de código')).toBe(true);
    expect(esCorreccion('en realidad quería otra cosa')).toBe(true);
    expect(esCorreccion('olvidate de eso')).toBe(true);
    expect(esCorreccion('quise decir un nodo de salida')).toBe(true);
  });

  it('no mira el final de una frase larga: un «no» en el medio es contenido', () => {
    expect(esCorreccion('quiero un sintetizador visual que no dependa del audio de entrada')).toBe(false);
  });
});

describe('palabras', () => {
  it('separa por cualquier espacio y descarta vacíos', () => {
    expect(palabras('  a\tb\nc  ')).toEqual(['a', 'b', 'c']);
  });
});

describe('nodoFantasma — qué se dibuja y dónde (la parte que se ve mientras hablás)', () => {
  it('no dibuja nada si todavía no hay idea', () => {
    expect(nodoFantasma('', { position: { x: 10, y: 10 } })).toBeNull();
    expect(nodoFantasma('un sinte', null)).toBeNull();
  });

  it('cae al lado del ancla, no encima', () => {
    const f = nodoFantasma('quiero un sintetizador visual', { position: { x: 100, y: 200 } });
    expect(f?.position).toEqual({ x: 100 + OFFSET_FANTASMA, y: 200 + OFFSET_FANTASMA });
  });

  it('sin ancla (lienzo vacío) cae en un punto fijo y visible', () => {
    expect(nodoFantasma('quiero un sintetizador visual', null)?.position).toEqual({ x: 80, y: 80 });
  });

  it('el fantasma se marca como fantasma y no es interactivo', () => {
    const f = nodoFantasma('quiero un sintetizador visual', null);
    expect(f?.data.ghost).toBe(true);
    expect(f?.id).toBe(ID_FANTASMA);
    expect(f?.className).toBe(CLASE_FANTASMA);
    expect(f?.draggable).toBe(false);
    expect(f?.selectable).toBe(false);
  });

  it('el título se recorta pero la descripción guarda TODO lo que se dijo', () => {
    const largo = 'quiero un sintetizador visual que se conecte a un nodo de código y después a una salida de audio';
    const f = nodoFantasma(largo, null);
    expect(f?.data.title).toBe('quiero un sintetizador visual que se conecte');
    expect(f?.data.description).toBe(largo);
  });
});
