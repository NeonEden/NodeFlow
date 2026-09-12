import { Edge } from 'reactflow';
import { CustomNode } from '../types';

/**
 * Módulo 1: Exportación e Integración con Obsidian (.md + Frontmatter)
 * 
 * 1.1 Estructura del Parser JSON-to-Markdown con Frontmatter YAML y [[wikilinks]]
 */
export function parseGraphToObsidianMarkdown(
  nodes: CustomNode[],
  edges: Edge[],
  mapTitle: string = 'Mapa Conceptual'
): string {
  const dateStr = new Date().toISOString().split('T')[0];

  // 1. Crear el Frontmatter YAML
  let md = `---\n`;
  md += `título: "${mapTitle}"\n`;
  md += `fecha: ${dateStr}\n`;
  md += `tags:\n  - nodeflow\n  - mapa-conceptual\n`;
  md += `nodos_totales: ${nodes.length}\n`;
  md += `conexiones_totales: ${edges.length}\n`;
  md += `generador: NodeFlow\n`;
  md += `---\n\n`;

  md += `# ${mapTitle}\n\n`;
  md += `## Resumen del Lienzo\n`;
  md += `Sintetizado automáticamente desde **NodeFlow** con estructura de enlaces bidireccionales nativa para Obsidian.\n\n`;

  // 2. Transliterar Nodos
  md += `## Nodos del Grafo\n\n`;
  nodes.forEach((node) => {
    // Tomar el título o label del nodo
    const nodeLabel = node.data.title || node.data.label || 'Concepto';
    const category = node.data.category || (node.data.isRoot ? 'Núcleo Central' : node.type || 'Concepto');

    md += `### [[${nodeLabel}]]\n`;
    md += `* **Tipo/Categoría:** ${category}\n`;
    if (node.data.tags && node.data.tags.length > 0) {
      md += `* **Tags:** ${node.data.tags.map((t) => (t.startsWith('#') ? t : `#${t}`)).join(' ')}\n`;
    }
    md += `\n${node.data.description || 'Sin descripción.'}\n\n`;

    // Buscar conexiones de salida (outgoing edges)
    const outgoing = edges.filter((e) => e.source === node.id);
    if (outgoing.length > 0) {
      md += `**Conexiones:**\n`;
      outgoing.forEach((edge) => {
        const targetNode = nodes.find((n) => n.id === edge.target);
        if (targetNode) {
          const targetLabel = targetNode.data.title || targetNode.data.label || 'Concepto';
          const relation = edge.label ? `_(${edge.label})_` : '→';
          md += `- ${relation} [[${targetLabel}]]\n`;
        }
      });
      md += `\n`;
    }
  });

  return md;
}

export interface ObsidianCanvasNode {
  id: string;
  type: 'text';
  text: string;
  x: number;
  y: number;
  width: number;
  height: number;
  color?: string;
}

export interface ObsidianCanvasEdge {
  id: string;
  fromNode: string;
  fromSide?: 'top' | 'right' | 'bottom' | 'left';
  toNode: string;
  toSide?: 'top' | 'right' | 'bottom' | 'left';
  label?: string;
  color?: string;
}

export interface ObsidianCanvasSpec {
  nodes: ObsidianCanvasNode[];
  edges: ObsidianCanvasEdge[];
}

/**
 * 1.2 Obsidian Canvas Spec (.canvas)
 * Genera el JSON oficial compatible con el plugin Obsidian Canvas nativo
 */
export function parseGraphToObsidianCanvas(
  nodes: CustomNode[],
  edges: Edge[],
  mapTitle: string = 'Mapa Conceptual'
): string {
  const canvasNodes: ObsidianCanvasNode[] = nodes.map((node) => {
    const nodeLabel = node.data.title || node.data.label || 'Concepto';
    const category = node.data.category || (node.data.isRoot ? 'Núcleo Central' : 'Concepto');
    const tagsText =
      node.data.tags && node.data.tags.length > 0
        ? `\n\n${node.data.tags.map((t) => (t.startsWith('#') ? t : `#${t}`)).join(' ')}`
        : '';

    const text = `### [[${nodeLabel}]]\n*${category}*\n\n${node.data.description || ''}${tagsText}`.trim();

    // Calcular dimensiones proporcionales
    const descLength = (node.data.description || '').length;
    const width = 280;
    const height = Math.min(320, Math.max(160, 120 + Math.round(descLength / 2.5)));

    // Obsidian admite color hex o números de color 1-6
    const color = node.data.colorAccent || (node.data.isRoot ? '#4f46e5' : '#059669');

    return {
      id: node.id,
      type: 'text',
      text,
      x: Math.round(node.position.x),
      y: Math.round(node.position.y),
      width,
      height,
      color,
    };
  });

  const canvasEdges: ObsidianCanvasEdge[] = edges.map((edge) => {
    // Determinar el lado de salida y llegada basándose en los handles o default
    let fromSide: 'top' | 'right' | 'bottom' | 'left' = 'right';
    let toSide: 'top' | 'right' | 'bottom' | 'left' = 'left';

    if (edge.sourceHandle) {
      if (edge.sourceHandle.includes('left')) fromSide = 'left';
      else if (edge.sourceHandle.includes('top')) fromSide = 'top';
      else if (edge.sourceHandle.includes('bottom')) fromSide = 'bottom';
      else if (edge.sourceHandle.includes('right')) fromSide = 'right';
    }

    if (edge.targetHandle) {
      if (edge.targetHandle.includes('right')) toSide = 'right';
      else if (edge.targetHandle.includes('top')) toSide = 'top';
      else if (edge.targetHandle.includes('bottom')) toSide = 'bottom';
      else if (edge.targetHandle.includes('left')) toSide = 'left';
    }

    const edgeObj: ObsidianCanvasEdge = {
      id: edge.id,
      fromNode: edge.source,
      fromSide,
      toNode: edge.target,
      toSide,
    };

    if (edge.label) {
      edgeObj.label = String(edge.label);
    }

    if (edge.style?.stroke) {
      edgeObj.color = String(edge.style.stroke);
    }

    return edgeObj;
  });

  const canvasData: ObsidianCanvasSpec = {
    nodes: canvasNodes,
    edges: canvasEdges,
  };

  return JSON.stringify(canvasData, null, 2);
}

/**
 * Disparar descarga directa del archivo en el navegador
 */
export function triggerFileDownload(content: string, filename: string, mimeType: string): void {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

/**
 * Descarga directa (.md) con Frontmatter y [[wikilinks]]
 */
export function downloadObsidianMarkdown(
  nodes: CustomNode[],
  edges: Edge[],
  mapTitle: string = 'NodeFlow-Mapa'
): void {
  const md = parseGraphToObsidianMarkdown(nodes, edges, mapTitle);
  const safeFilename = sanitizeFilename(mapTitle);
  triggerFileDownload(md, `${safeFilename}.md`, 'text/markdown;charset=utf-8');
}

/**
 * Descarga directa Obsidian Canvas Spec (.canvas)
 */
export function downloadObsidianCanvas(
  nodes: CustomNode[],
  edges: Edge[],
  mapTitle: string = 'NodeFlow-Canvas'
): void {
  const canvasJson = parseGraphToObsidianCanvas(nodes, edges, mapTitle);
  const safeFilename = sanitizeFilename(mapTitle);
  triggerFileDownload(canvasJson, `${safeFilename}.canvas`, 'application/json;charset=utf-8');
}

function sanitizeFilename(name: string): string {
  return name
    .trim()
    .replace(/[\\/*?:"<>|]/g, '')
    .replace(/\s+/g, '-')
    .toLowerCase() || 'nodeflow-mapa';
}
