import { describe, it, expect } from 'vitest';
import { autoLayoutNodes } from './layout';
import { CustomNode } from '../types';
import { Edge } from 'reactflow';

describe('autoLayoutNodes', () => {
  const createNode = (id: string, title: string, isRoot = false): CustomNode => ({
    id,
    type: 'customNode',
    position: { x: 50, y: 50 },
    data: {
      id,
      title,
      description: `Descripción de ${title}`,
      isRoot,
    },
  });

  it('no muta la entrada original ni sus posiciones', () => {
    const inputNodes: CustomNode[] = [
      createNode('root', 'Nodo Raíz', true),
      createNode('child', 'Nodo Hijo'),
    ];
    const inputEdges: Edge[] = [
      { id: 'e1', source: 'root', target: 'child' },
    ];

    const nodesSnapshot = JSON.parse(JSON.stringify(inputNodes));
    const edgesSnapshot = JSON.parse(JSON.stringify(inputEdges));

    const result = autoLayoutNodes(inputNodes, inputEdges);

    expect(inputNodes).toEqual(nodesSnapshot);
    expect(inputEdges).toEqual(edgesSnapshot);
    expect(result).not.toBe(inputNodes);
    expect(inputNodes[0].position).toEqual({ x: 50, y: 50 });
  });

  it('asigna coordenadas x e y finitas a todos los nodos en dirección TB y LR', () => {
    const nodes: CustomNode[] = [
      createNode('n1', 'Primero'),
      createNode('n2', 'Segundo'),
      createNode('n3', 'Tercero'),
    ];
    const edges: Edge[] = [
      { id: 'e1-2', source: 'n1', target: 'n2' },
      { id: 'e2-3', source: 'n2', target: 'n3' },
    ];

    const resultTB = autoLayoutNodes(nodes, edges, 'TB');
    expect(resultTB).toHaveLength(3);
    for (const node of resultTB) {
      expect(Number.isFinite(node.position.x)).toBe(true);
      expect(Number.isFinite(node.position.y)).toBe(true);
      expect(Number.isNaN(node.position.x)).toBe(false);
      expect(Number.isNaN(node.position.y)).toBe(false);
    }

    // En TB, el nivel inferior debe tener mayor Y que el nivel superior
    const tbN1 = resultTB.find((n) => n.id === 'n1')!;
    const tbN2 = resultTB.find((n) => n.id === 'n2')!;
    const tbN3 = resultTB.find((n) => n.id === 'n3')!;
    expect(tbN2.position.y).toBeGreaterThan(tbN1.position.y);
    expect(tbN3.position.y).toBeGreaterThan(tbN2.position.y);

    const resultLR = autoLayoutNodes(nodes, edges, 'LR');
    expect(resultLR).toHaveLength(3);
    for (const node of resultLR) {
      expect(Number.isFinite(node.position.x)).toBe(true);
      expect(Number.isFinite(node.position.y)).toBe(true);
    }

    // En LR, el nivel derecho debe tener mayor X que el nivel izquierdo
    const lrN1 = resultLR.find((n) => n.id === 'n1')!;
    const lrN2 = resultLR.find((n) => n.id === 'n2')!;
    expect(lrN2.position.x).toBeGreaterThan(lrN1.position.x);
  });

  it('ubica correctamente un nodo aislado sin aristas', () => {
    const connectedA = createNode('cA', 'Conectado A');
    const connectedB = createNode('cB', 'Conectado B');
    const isolatedNode = createNode('iso', 'Nodo Desconectado');

    const edges: Edge[] = [
      { id: 'e-ab', source: 'cA', target: 'cB' },
    ];

    const result = autoLayoutNodes([connectedA, connectedB, isolatedNode], edges);
    const placedIsolated = result.find((n) => n.id === 'iso');

    expect(placedIsolated).toBeDefined();
    expect(Number.isFinite(placedIsolated!.position.x)).toBe(true);
    expect(Number.isFinite(placedIsolated!.position.y)).toBe(true);
    expect(placedIsolated!.position.x).not.toBe(50); // no conserva el mock inicial si fue reubicado
  });

  it('produce el mismo resultado de posiciones ante la misma entrada (determinismo)', () => {
    const nodes: CustomNode[] = [
      createNode('a', 'Alfa', true),
      createNode('b', 'Beta'),
      createNode('c', 'Gamma'),
      createNode('d', 'Delta'),
    ];
    const edges: Edge[] = [
      { id: 'e-ab', source: 'a', target: 'b' },
      { id: 'e-ac', source: 'a', target: 'c' },
      { id: 'e-bd', source: 'b', target: 'd' },
    ];

    const run1 = autoLayoutNodes(nodes, edges);
    const run2 = autoLayoutNodes(nodes, edges);

    expect(run1).toEqual(run2);
    expect(run1.map((n) => n.position)).toEqual(run2.map((n) => n.position));
  });

  it('devuelve un array vacío si la lista de nodos está vacía', () => {
    const result = autoLayoutNodes([], []);
    expect(result).toEqual([]);
  });
});
