import { describe, it, expect } from 'vitest';
import { parseGraphToObsidianMarkdown } from './obsidianExport';
import { CustomNode } from '../types';
import { Edge } from 'reactflow';

describe('parseGraphToObsidianMarkdown', () => {
  const createNode = (
    id: string,
    title: string,
    description: string = '',
    tags: string[] = []
  ): CustomNode => ({
    id,
    type: 'customNode',
    position: { x: 0, y: 0 },
    data: {
      id,
      title,
      description,
      tags,
    },
  });

  it('enlaza un wikilink entre dos nodos conectados mediante una arista', () => {
    const nodeA = createNode('n1', 'Hipótesis Alfa');
    const nodeB = createNode('n2', 'Evidencia Beta');
    const edge: Edge = {
      id: 'e1-2',
      source: 'n1',
      target: 'n2',
      label: 'valida',
    };

    const markdown = parseGraphToObsidianMarkdown([nodeA, nodeB], [edge]);

    // Debe contener el encabezado del origen y el enlace wikilink hacia el destino
    expect(markdown).toContain('### [[Hipótesis Alfa]]');
    expect(markdown).toContain('**Conexiones:**');
    expect(markdown).toContain('- _(valida)_ [[Evidencia Beta]]');
  });

  it('incluye el título de cada nodo en el texto exportado', () => {
    const node1 = createNode('id-1', 'Premisa de Arquitectura', 'Detalles de la arquitectura');
    const node2 = createNode('id-2', 'Patrón de Seguridad', 'Detalles de seguridad');

    const markdown = parseGraphToObsidianMarkdown([node1, node2], []);

    expect(markdown).toContain('Premisa de Arquitectura');
    expect(markdown).toContain('Patrón de Seguridad');
    expect(markdown).toContain('### [[Premisa de Arquitectura]]');
    expect(markdown).toContain('### [[Patrón de Seguridad]]');
    expect(markdown).toContain('Detalles de la arquitectura');
    expect(markdown).toContain('Detalles de seguridad');
  });

  it('emite el bloque frontmatter YAML con los metadatos requeridos', () => {
    const nodes: CustomNode[] = [
      createNode('a', 'Concepto 1'),
      createNode('b', 'Concepto 2'),
    ];
    const edges: Edge[] = [
      { id: 'e1', source: 'a', target: 'b' },
    ];

    const title = 'Investigación Grafos';
    const markdown = parseGraphToObsidianMarkdown(nodes, edges, title);

    // Frontmatter delimitado por --- al inicio
    expect(markdown.startsWith('---\n')).toBe(true);
    expect(markdown).toMatch(/^---\n[\s\S]+?\n---\n\n/);

    // Campos del frontmatter
    expect(markdown).toContain(`título: "${title}"`);
    expect(markdown).toMatch(/fecha: \d{4}-\d{2}-\d{2}/);
    expect(markdown).toContain('tags:\n  - nodeflow\n  - mapa-conceptual');
    expect(markdown).toContain('nodos_totales: 2');
    expect(markdown).toContain('conexiones_totales: 1');
    expect(markdown).toContain('generador: NodeFlow');
  });

  it('no rompe con un grafo vacío y devuelve una estructura markdown válida', () => {
    let markdown = '';
    expect(() => {
      markdown = parseGraphToObsidianMarkdown([], []);
    }).not.toThrow();

    expect(typeof markdown).toBe('string');
    expect(markdown.startsWith('---\n')).toBe(true);
    expect(markdown).toContain('nodos_totales: 0');
    expect(markdown).toContain('conexiones_totales: 0');
    expect(markdown).toContain('# Mapa Conceptual');
    expect(markdown).toContain('## Resumen del Lienzo');
    expect(markdown).toContain('## Nodos del Grafo');
  });

  it('formatea las etiquetas (tags) con el prefijo # en la sección del nodo', () => {
    const node = createNode('t1', 'Nodo Con Tags', 'Descripción', ['ai', '#local']);
    const markdown = parseGraphToObsidianMarkdown([node], []);

    expect(markdown).toContain('* **Tags:** #ai #local');
  });
});
