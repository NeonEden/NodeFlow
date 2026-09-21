import { describe, expect, it } from 'vitest';
import type { VozComando } from '../services/vozService';
import { planEsConsulta, temasDeConsulta } from './voz';

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
