import { describe, expect, it } from 'vitest';
import type { VozComando } from '../services/vozService';
import {
  elegirVariante,
  esAfirmativo,
  esCorte,
  esNegativo,
  planEsConsulta,
  temaDe,
  temasDeConsulta,
} from './voz';

const consulta = (tema: string): VozComando => ({ accion: 'consultar', tema });
const crear = (titulo: string): VozComando => ({ accion: 'crear', titulo });

describe('planEsConsulta', () => {
  it('sin comandos no es una consulta', () => {
    expect(planEsConsulta([])).toBe(false);
    expect(planEsConsulta(null)).toBe(false);
    expect(planEsConsulta(undefined)).toBe(false);
  });

  it('una consulta sola sí lo es (es el caso que no se aprueba ni se aplica)', () => {
    expect(planEsConsulta([consulta('qué quedó abierto')])).toBe(true);
  });

  it('consultar mezclado con una operación NO lo es: hay algo que aplicar', () => {
    expect(planEsConsulta([consulta('qué hay'), crear('Otra idea')])).toBe(false);
  });

  it('una operación cualquiera no lo es', () => {
    expect(planEsConsulta([{ accion: 'enfocar', nodos: ['n-1'] }])).toBe(false);
  });
});

describe('temasDeConsulta', () => {
  it('devuelve sólo los temas, sin los vacíos', () => {
    expect(temasDeConsulta([consulta(' qué quedó abierto '), consulta('')])).toEqual(['qué quedó abierto']);
  });

  it('sin consultas devuelve una lista vacía', () => {
    expect(temasDeConsulta([crear('x')])).toEqual([]);
    expect(temasDeConsulta(null)).toEqual([]);
  });
});

// El título de una idea es el TEMA, no la frase dictada (medido 20/09/2026 con el modo conversación).
describe('temaDe', () => {
  it('saca las muletillas del habla y deja el tema', () => {
    expect(temaDe('quiero explorar la idea de comandos por voz')).toBe('Comandos por voz');
    expect(temaDe('me gustaría que la app hable sola')).toBe('Que la app hable sola');
    expect(temaDe('la idea es un cerebro local')).toBe('Un cerebro local');
    expect(temaDe('estaría bueno probar el canvas infinito')).toBe('Probar el canvas infinito');
    expect(temaDe('pensé en vender el one-pager')).toBe('Vender el one-pager');
  });

  it('no rompe un título que ya viene limpio', () => {
    expect(temaDe('comandos por voz')).toBe('Comandos por voz');
    expect(temaDe('  NodeFlow  ')).toBe('NodeFlow');
  });

  it('saca comillas y puntuación de los bordes', () => {
    expect(temaDe('«voz en tiempo real».')).toBe('Voz en tiempo real');
  });

  it('corta en 60 caracteres, en palabra completa', () => {
    const largo = temaDe('una idea que ocupa muchísimos caracteres y sigue y sigue sin parar nunca jamás');
    expect(largo.length).toBeLessThanOrEqual(60);
    expect(largo.endsWith(' ')).toBe(false);
    expect(largo).not.toContain('…');
  });

  it('una sola muletilla no deja el título vacío', () => {
    expect(temaDe('quiero')).toBe('Quiero');
    expect(temaDe('   ')).toBe('');
  });
});

// La app tiene que entender un sí dicho como lo dice la gente (antes era un regex de una palabra).
describe('esAfirmativo / esNegativo', () => {
  it('acepta las formas reales de decir que sí', () => {
    expect(esAfirmativo('sí')).toBe(true);
    expect(esAfirmativo('dale')).toBe(true);
    expect(esAfirmativo('bueno, me gustaría ver qué sale')).toBe(true);
    expect(esAfirmativo('podríamos probar')).toBe(true);
    expect(esAfirmativo('ok, hacelo')).toBe(true);
  });

  it('el no gana sobre lo que venga después', () => {
    expect(esAfirmativo('no')).toBe(false);
    expect(esAfirmativo('no, mejor no')).toBe(false);
    expect(esNegativo('no, gracias')).toBe(true);
    expect(esAfirmativo('todavía no')).toBe(false);
  });

  it('una frase cualquiera no es un sí', () => {
    expect(esAfirmativo('comandos por voz para el lienzo')).toBe(false);
  });

  it('reconoce el corte de la conversación', () => {
    expect(esCorte('cortá')).toBe(true);
    expect(esCorte('chau, gracias')).toBe(true);
    expect(esCorte('nada más por hoy')).toBe(true);
    expect(esCorte('quiero explorar una idea')).toBe(false);
  });
});

describe('elegirVariante', () => {
  it('nunca repite la última variante', () => {
    for (let vez = 0; vez < 200; vez++) {
      const i = elegirVariante(3, 1);
      expect(i).not.toBe(1);
      expect(i).toBeGreaterThanOrEqual(0);
      expect(i).toBeLessThan(3);
    }
  });

  it('con una sola variante devuelve esa', () => {
    expect(elegirVariante(1, 0)).toBe(0);
  });
});
