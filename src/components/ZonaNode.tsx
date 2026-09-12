import React, { memo } from 'react';
import { NodeProps } from 'reactflow';
import { CANVAS_THEME } from '../state/canvasTheme';

export interface ZonaNodeData {
  nivel: number;
  conteo: number;
  /** Sólo si la pureza de categoría la sostiene; si no, null. */
  categoria: string | null;
  acento: string;
}

/**
 * Marco translúcido de una zona derivada (un nivel de profundidad del mapa).
 *
 * No es un nodo del lienzo: App lo inyecta en el array de render y el wrapper se
 * marca con `pointerEvents: none`, así el marco nunca roba un clic ni un arrastre
 * de paneo. No se guarda en el vault ni aparece en el minimapa.
 */
export const ZonaNode: React.FC<NodeProps<ZonaNodeData>> = memo(({ data }) => {
  const acento = data?.acento || '#6366f1';
  const tema = CANVAS_THEME;

  return (
    <div
      className="w-full h-full relative rounded-2xl flex flex-col items-start justify-start overflow-hidden"
      style={{
        border: `1.5px dashed ${acento}59`,
        background: `linear-gradient(180deg, ${acento}14 0%, ${acento}08 220px, transparent 100%)`,
      }}
    >
      <div className="flex items-center gap-2 px-3 py-1.5 m-2 rounded-lg"
        style={{
          backgroundColor: tema.zonaChipBg,
          border: `1px solid ${acento}33`,
          boxShadow: '0 1px 2px rgba(15,23,42,0.04)',
        }}
      >
        <span
          className="text-[10px] font-bold uppercase tracking-[0.14em] tabular-nums"
          style={{ color: acento }}
        >
          Nivel {data?.nivel ?? 0}
        </span>
        <span className="text-[10px] font-medium tabular-nums" style={{ color: tema.body }}>
          {data?.conteo ?? 0} {data?.conteo === 1 ? 'concepto' : 'conceptos'}
        </span>
        {data?.categoria && (
          <span
            className="text-[9px] font-semibold uppercase tracking-wider px-1.5 py-0.5 rounded"
            style={{ backgroundColor: `${acento}14`, color: acento }}
          >
            {data.categoria}
          </span>
        )}
      </div>

      {/* Marca de agua: orienta cuando el chip del encabezado queda fuera de pantalla. */}
      <span
        className="absolute top-3 right-4 text-[46px] font-black leading-none tabular-nums select-none"
        style={{ color: `${acento}14` }}
      >
        {String(data?.nivel ?? 0).padStart(2, '0')}
      </span>
    </div>
  );
});

ZonaNode.displayName = 'ZonaNode';
