import React, { useState } from 'react';
import { Layers, Check, X } from 'lucide-react';

export type ModoZonas = 'nivel' | 'categoria';

export interface CategoriaZona {
  nombre: string;
  n: number;
  acento: string;
}

interface ZonasHudProps {
  visible: boolean;
  onVisible: (v: boolean) => void;
  modo: ModoZonas;
  onModo: (m: ModoZonas) => void;
  categorias: CategoriaZona[];
  categoriaLente: string | null;
  onCategoria: (c: string | null) => void;
}

/**
 * Zonas del lienzo, en dos modos:
 *
 * - **Niveles**: marcos translúcidos por profundidad del grafo. El eje X del layout ES
 *   la profundidad, así que el marco coincide con una columna real y la etiqueta es un
 *   hecho.
 * - **Categorías**: lente. En este lienzo los marcos por categoría NO se pueden dibujar
 *   (medido: 0 pares de nodos vecinos de la misma categoría; las categorías están
 *   repartidas por profundidad y un marco que las abarque cruzaría medio mapa). La lente
 *   consigue lo mismo de otra forma: atenúa lo que no es de la categoría y marca a sus
 *   miembros.
 */
export const ZonasHud: React.FC<ZonasHudProps> = ({
  visible,
  onVisible,
  modo,
  onModo,
  categorias,
  categoriaLente,
  onCategoria,
}) => {
  const [abierto, setAbierto] = useState(false);
  const activo = visible || Boolean(categoriaLente);

  return (
    <div className="relative">
      <button
        type="button"
        onClick={() => setAbierto((v) => !v)}
        className={`border rounded-lg p-2 backdrop-blur-md flex items-center gap-1.5 text-xs shadow-lg transition-colors cursor-pointer ${
          abierto || activo
            ? 'bg-indigo-500/10 border-indigo-400/70 text-indigo-600 hover:bg-indigo-500/20'
            : 'bg-[var(--nf-hud-bg)] border-[var(--nf-hud-border)] text-[var(--nf-hud-text)] hover:bg-[var(--nf-hud-hover)]'
        }`}
        title="Zonas del lienzo: por nivel o por categoría"
      >
        <Layers size={14} />
        <span className="text-[10px] hidden sm:inline font-mono">Zonas</span>
      </button>

      {abierto && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setAbierto(false)} />
          <div
            className="absolute bottom-full left-0 mb-2 w-72 rounded-xl border p-3 shadow-2xl backdrop-blur-xl z-50"
            style={{
              backgroundColor: 'var(--nf-hud-bg)',
              borderColor: 'var(--nf-hud-border)',
              color: 'var(--nf-hud-text)',
            }}
          >
            <div className="flex items-center justify-between pb-2 mb-2 border-b border-[var(--nf-hud-border)]">
              <span className="text-[11px] font-semibold">Zonas del lienzo</span>
              {categoriaLente && (
                <button
                  type="button"
                  onClick={() => onCategoria(null)}
                  className="flex items-center gap-1 text-[10px] hover:opacity-80 cursor-pointer"
                  title="Quitar la lente"
                >
                  <X size={11} /> Quitar lente
                </button>
              )}
            </div>

            {/* Modo */}
            <div className="grid grid-cols-2 gap-1 p-1 rounded-lg border border-[var(--nf-hud-border)]">
              {(['nivel', 'categoria'] as ModoZonas[]).map((m) => (
                <button
                  key={m}
                  type="button"
                  onClick={() => onModo(m)}
                  className={`py-1 text-[10px] rounded transition-colors cursor-pointer ${
                    modo === m ? 'bg-indigo-600 text-white font-medium' : 'hover:bg-[var(--nf-hud-hover)]'
                  }`}
                >
                  {m === 'nivel' ? 'Niveles' : 'Categorías'}
                </button>
              ))}
            </div>

            {modo === 'nivel' ? (
              <>
                <button
                  type="button"
                  onClick={() => onVisible(!visible)}
                  className="mt-2 w-full flex items-center justify-between px-2 py-1.5 rounded-lg border border-[var(--nf-hud-border)] hover:bg-[var(--nf-hud-hover)] transition-colors cursor-pointer"
                >
                  <span className="text-[11px]">Marcos por nivel</span>
                  <span
                    className={`text-[10px] font-mono px-1.5 py-0.5 rounded ${
                      visible ? 'bg-indigo-600 text-white' : 'opacity-60'
                    }`}
                  >
                    {visible ? 'ON' : 'OFF'}
                  </span>
                </button>
                <p className="text-[9px] opacity-60 mt-1.5 leading-snug">
                  Un marco por nivel de profundidad. El eje X del layout es la profundidad,
                  así que el marco coincide con una columna real del mapa.
                </p>
              </>
            ) : (
              <>
                <p className="text-[9px] opacity-60 mt-2 leading-snug">
                  Los marcos por categoría no se pueden dibujar en este mapa: sus nodos no son
                  vecinos (están repartidos por profundidad). La lente atenúa el resto y marca
                  a los miembros de la categoría.
                </p>
                <div className="mt-2 max-h-52 overflow-y-auto pr-0.5 space-y-0.5">
                  {categorias.map((c) => {
                    const activa = categoriaLente === c.nombre;
                    return (
                      <button
                        key={c.nombre}
                        type="button"
                        onClick={() => onCategoria(activa ? null : c.nombre)}
                        className={`w-full flex items-center gap-2 px-2 py-1 rounded-lg text-left transition-colors cursor-pointer ${
                          activa ? 'bg-indigo-500/15 ring-1 ring-indigo-400/60' : 'hover:bg-[var(--nf-hud-hover)]'
                        }`}
                      >
                        <span
                          className="w-2.5 h-2.5 rounded-full shrink-0"
                          style={{ backgroundColor: c.acento }}
                        />
                        <span className="text-[11px] truncate flex-1">{c.nombre}</span>
                        <span className="text-[10px] font-mono opacity-60">{c.n}</span>
                        {activa && <Check size={11} className="text-indigo-500" />}
                      </button>
                    );
                  })}
                </div>
              </>
            )}
          </div>
        </>
      )}
    </div>
  );
};
