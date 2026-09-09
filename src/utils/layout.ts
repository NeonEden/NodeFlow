import { CustomNode } from '../types';
import { Edge } from 'reactflow';

/**
 * Organizes nodes in an aesthetic hierarchical layout from roots downward/rightward.
 * Avoids overlapping and aligns child branches systematically.
 */
export function autoLayoutNodes(
  nodes: CustomNode[],
  edges: Edge[],
  direction: 'TB' | 'LR' = 'TB'
): CustomNode[] {
  if (nodes.length === 0) return [];

  const NODE_WIDTH = 270;
  const NODE_HEIGHT = 160;
  const HORIZONTAL_GAP = 70;
  const VERTICAL_GAP = 90;

  // Build adjacency
  const childMap = new Map<string, string[]>();
  const parentMap = new Map<string, string[]>();

  nodes.forEach((n) => {
    childMap.set(n.id, []);
    parentMap.set(n.id, []);
  });

  edges.forEach((e) => {
    if (childMap.has(e.source) && childMap.has(e.target)) {
      childMap.get(e.source)!.push(e.target);
      parentMap.get(e.target)!.push(e.source);
    }
  });

  // Identify root nodes
  let rootNodes = nodes.filter(
    (n) => n.data.isRoot || (parentMap.get(n.id)?.length || 0) === 0
  );

  if (rootNodes.length === 0) {
    rootNodes = [nodes[0]];
  }

  // Assign levels (BFS)
  const levels = new Map<string, number>();
  const visited = new Set<string>();

  const queue: { id: string; level: number }[] = rootNodes.map((r) => ({
    id: r.id,
    level: 0,
  }));

  rootNodes.forEach((r) => {
    levels.set(r.id, 0);
    visited.add(r.id);
  });

  while (queue.length > 0) {
    const { id, level } = queue.shift()!;
    const children = childMap.get(id) || [];

    for (const childId of children) {
      if (!visited.has(childId)) {
        visited.add(childId);
        levels.set(childId, level + 1);
        queue.push({ id: childId, level: level + 1 });
      }
    }
  }

  // Any unvisited nodes (disconnected islands)
  let maxAssignedLevel = 0;
  levels.forEach((lvl) => {
    if (lvl > maxAssignedLevel) maxAssignedLevel = lvl;
  });

  nodes.forEach((n) => {
    if (!visited.has(n.id)) {
      visited.add(n.id);
      levels.set(n.id, 0);
    }
  });

  // Group nodes by level
  const nodesByLevel = new Map<number, CustomNode[]>();
  nodes.forEach((node) => {
    const lvl = levels.get(node.id) || 0;
    if (!nodesByLevel.has(lvl)) {
      nodesByLevel.set(lvl, []);
    }
    nodesByLevel.get(lvl)!.push(node);
  });

  // Calculate new coordinates
  const newPositions = new Map<string, { x: number; y: number }>();

  if (direction === 'TB') {
    // Top-to-Bottom
    const sortedLevels = Array.from(nodesByLevel.keys()).sort((a, b) => a - b);
    let currentY = 100;

    sortedLevels.forEach((lvl) => {
      const levelNodes = nodesByLevel.get(lvl)!;
      const totalWidth =
        levelNodes.length * NODE_WIDTH + (levelNodes.length - 1) * HORIZONTAL_GAP;
      let startX = -(totalWidth / 2) + 400;

      levelNodes.forEach((node, index) => {
        newPositions.set(node.id, {
          x: Math.round(startX + index * (NODE_WIDTH + HORIZONTAL_GAP)),
          y: Math.round(currentY),
        });
      });

      currentY += NODE_HEIGHT + VERTICAL_GAP;
    });
  } else {
    // Left-to-Right
    const sortedLevels = Array.from(nodesByLevel.keys()).sort((a, b) => a - b);
    let currentX = 100;

    sortedLevels.forEach((lvl) => {
      const levelNodes = nodesByLevel.get(lvl)!;
      const totalHeight =
        levelNodes.length * NODE_HEIGHT + (levelNodes.length - 1) * VERTICAL_GAP;
      let startY = -(totalHeight / 2) + 300;

      levelNodes.forEach((node, index) => {
        newPositions.set(node.id, {
          x: Math.round(currentX),
          y: Math.round(startY + index * (NODE_HEIGHT + VERTICAL_GAP)),
        });
      });

      currentX += NODE_WIDTH + HORIZONTAL_GAP;
    });
  }

  return nodes.map((n) => {
    const pos = newPositions.get(n.id);
    if (pos) {
      return {
        ...n,
        position: pos,
      };
    }
    return n;
  });
}
