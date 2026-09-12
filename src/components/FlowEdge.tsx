import React, { memo, useSyncExternalStore } from 'react';
import {
  EdgeProps,
  getBezierPath,
  getSmoothStepPath,
  getStraightPath,
} from 'reactflow';
import { focusStore, focusedNodeId } from '../state/focusStore';
import { useTema } from '../state/canvasPrefs';

export interface FlowEdgeData {
  /** Etiqueta textual de la arista (antes vivía en `edge.label`, que React Flow
   *  dibuja SIEMPRE; acá se muestra sólo en hover). */
  label?: string;
  /** Curva guardada por el usuario en el panel Conectores. */
  curve?: string;
  color?: string;
  strokeWidth?: number;
  animated?: boolean;
  /** Acento del nodo origen/destino, inyectado por App en `processedEdges`. */
  sourceColor?: string;
  targetColor?: string;
}

/** Aristas fuera del foco: casi invisibles. */
const DIM_OPACITY = 0.1;

/**
 * Arista del lienzo: reemplaza al `smoothstep` nativo para que la etiqueta, la
 * atenuación y el color del foco se decidan en un solo lugar.
 *
 * Reglas:
 * - Sin foco: 42% de opacidad, sin animación, sin etiqueta (el ruido de fondo baja).
 * - Con foco (hover o selección de un nodo): las conexiones directas al 100% y con
 *   el color del nodo origen; el resto al 10%.
 * - La etiqueta textual aparece sólo al pasar el mouse sobre la línea.
 */
export const FlowEdge: React.FC<EdgeProps<FlowEdgeData>> = memo(
  ({
    id,
    source,
    target,
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
    data,
    markerEnd,
    selected,
    style,
  }) => {
    const focus = useSyncExternalStore(focusStore.subscribe, focusStore.getState);
    const tema = useTema();
    const baseOpacity = tema.edgeOpacity;
    const lente = focus.lente;
    const focusId = focusedNodeId(focus);

    const isDirect = !!focusId && (source === focusId || target === focusId);
    const isHovered = focus.hoveredEdgeId === id;
    const active = isDirect || isHovered || Boolean(selected);
    const dimmed = !!focusId && !active;

    // Lente semántica por categoría (manda sobre el foco por hover):
    // interna = los dos extremos son de la categoría; borde = uno solo; ajena = ninguno.
    const enLente = lente
      ? lente.ids.has(source) && lente.ids.has(target)
        ? 'interna'
        : lente.ids.has(source) || lente.ids.has(target)
        ? 'borde'
        : 'ajena'
      : null;

    const opacity = enLente
      ? enLente === 'interna' || isHovered
        ? 1
        : enLente === 'borde'
        ? 0.5
        : 0.06
      : dimmed
      ? DIM_OPACITY
      : active
      ? 1
      : baseOpacity;

    const curve = data?.curve || 'smoothstep';
    let path = '';
    let labelX = 0;
    let labelY = 0;

    if (curve === 'straight') {
      [path, labelX, labelY] = getStraightPath({ sourceX, sourceY, targetX, targetY });
    } else if (curve === 'default') {
      [path, labelX, labelY] = getBezierPath({
        sourceX,
        sourceY,
        sourcePosition,
        targetX,
        targetY,
        targetPosition,
      });
    } else {
      // smoothstep (y `step`, que es smoothstep con radio 0)
      [path, labelX, labelY] = getSmoothStepPath({
        sourceX,
        sourceY,
        sourcePosition,
        targetX,
        targetY,
        targetPosition,
        borderRadius: curve === 'step' ? 0 : 26,
        offset: curve === 'step' ? 0 : 18,
      });
    }

    const baseColor = (style?.stroke as string) || data?.color || '#6366f1';
    const baseWidth = Number(style?.strokeWidth ?? data?.strokeWidth ?? 2);
    // El color del realce sale del nodo de origen; si el foco es el destino, del destino.
    const focusColor =
      source === focusId ? data?.sourceColor : data?.targetColor;
    const colorLente = enLente === 'interna' ? data?.sourceColor : undefined;
    const stroke = (isDirect && focusColor) || colorLente || baseColor;
    const realzada = (active && !dimmed) || enLente === 'interna';
    const width = realzada ? baseWidth + 0.9 : baseWidth;

    const label = data?.label;
    const showLabel = Boolean(label) && (isHovered || Boolean(selected));
    const labelWidth = label ? label.length * 5.6 + 18 : 0;
    const wantsDash = Boolean(data?.animated);
    // El dash animado queda como señal de foco: en reposo 53 líneas en movimiento
    // es exactamente el ruido que buscamos sacar.
    const dash = wantsDash && (enLente ? enLente === 'interna' : active && !dimmed);

    return (
      <g
        className={`nf-edge${active ? ' nf-edge--active' : ''}`}
        style={{ opacity, transition: 'opacity 180ms ease' }}
      >
        <path
          d={path}
          fill="none"
          className="react-flow__edge-path"
          markerEnd={markerEnd}
          stroke={stroke}
          strokeWidth={width}
          style={{ transition: 'stroke 180ms ease, stroke-width 180ms ease' }}
        />
        {dash && (
          <path
            d={path}
            fill="none"
            className="nf-edge-dash"
            stroke={stroke}
            strokeWidth={width}
          />
        )}
        {showLabel && label && (
          <g transform={`translate(${labelX}, ${labelY})`} className="nf-edge-label">
            <rect
              x={-labelWidth / 2}
              y={-10}
              width={labelWidth}
              height={20}
              rx={10}
              fillOpacity={0.96}
              stroke={stroke}
              strokeOpacity={0.55}
              strokeWidth={1}
              style={{ fill: 'var(--nf-edge-label-bg)' }}
            />
            <text
              textAnchor="middle"
              dominantBaseline="middle"
              fontSize={10}
              fontWeight={600}
              style={{ fill: 'var(--nf-edge-label-text)' }}
            >
              {label}
            </text>
          </g>
        )}
      </g>
    );
  }
);

FlowEdge.displayName = 'FlowEdge';
