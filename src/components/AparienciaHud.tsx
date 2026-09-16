import React, { useState } from 'react';
import { Palette, Check, RotateCcw } from 'lucide-react';
import {
  canvasPrefs,
  usePrefs,
  FONDOS,
  MODOS_TARJETA,
  PREFS_POR_DEFECTO,
} from '../state/canvasPrefs';
import { luminancia } from '../state/canvasTheme';
import { useIdioma } from '../i18n/useIdioma';

/**
 * Apariencia del lienzo: fondo y superficie de las tarjetas.
 *
 * Es una preferencia de MIRADA, no un dato: se guarda en localStorage y no viaja al
 * vault ni al grafo. El color de cada nodo sigue editándose en el nodo (o en el
 * modal de edición), que sí es un dato del concepto.
 */
export const AparienciaHud: React.FC = () => {
  const { t } = useIdioma();
  const prefs = usePrefs();
  const [abierto, setAbierto] = useState(false);
  const fondoActivo = FONDOS.find((f) => f.id === prefs.fondoId);

  return (
    <div className="relative">
      <button
        type="button"
        onClick={() => setAbierto((v) => !v)}
        className={`border rounded-lg p-2 backdrop-blur-md flex items-center gap-1.5 text-xs shadow-lg transition-colors cursor-pointer ${
          abierto
            ? 'bg-indigo-500/10 border-indigo-400/70 text-indigo-600'
            : 'bg-[var(--nf-hud-bg)] border-[var(--nf-hud-border)] text-[var(--nf-hud-text)] hover:bg-[var(--nf-hud-hover)]'
        }`}
        title={t('apariencia.colores.ayuda')}
      >
        <Palette size={14} />
        <span className="text-[10px] hidden sm:inline font-mono">{t('apariencia.colores')}</span>
      </button>

      {abierto && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setAbierto(false)} />
          <div
            className="absolute bottom-full left-0 mb-2 w-64 rounded-xl border p-3 shadow-2xl backdrop-blur-xl z-50"
            style={{
              backgroundColor: 'var(--nf-hud-bg)',
              borderColor: 'var(--nf-hud-border)',
              color: 'var(--nf-hud-text)',
            }}
          >
            <div className="flex items-center justify-between pb-2 mb-2 border-b border-[var(--nf-hud-border)]">
              <span className="text-[11px] font-semibold">{t('apariencia.titulo')}</span>
              <button
                type="button"
                onClick={() => canvasPrefs.reset()}
                title={t('apariencia.restaurar')}
                className="flex items-center gap-1 text-[10px] hover:opacity-80 transition-opacity cursor-pointer"
              >
                <RotateCcw size={11} />
                Reset
              </button>
            </div>

            {/* Fondo */}
            <label className="text-[10px] font-medium uppercase tracking-wider opacity-70 block mb-1.5">
              Fondo
            </label>
            <div className="grid grid-cols-6 gap-1.5">
              {FONDOS.map((f) => {
                const activo = prefs.fondoId === f.id;
                return (
                  <button
                    key={f.id}
                    type="button"
                    onClick={() => canvasPrefs.setFondo(f.id)}
                    title={f.nombre}
                    className={`h-7 rounded-lg flex items-center justify-center transition-transform hover:scale-105 cursor-pointer ${
                      activo ? 'ring-2 ring-indigo-500 scale-105' : 'border border-[var(--nf-hud-border)]'
                    }`}
                    style={{ backgroundColor: f.color }}
                  >
                    {activo && (
                      <Check
                        size={12}
                        className={luminancia(f.color) > 0.5 ? 'text-slate-900' : 'text-white'}
                      />
                    )}
                  </button>
                );
              })}
            </div>

            {/* Color propio: la paleta se deriva del fondo para que no queden
                combinaciones ilegibles (tarjetas y textos siguen el contraste). */}
            <div className="flex items-center gap-2 mt-2 pt-2 border-t border-[var(--nf-hud-border)]">
              <span className="text-[10px] opacity-70">{t('apariencia.colorPropio')}</span>
              <input
                type="color"
                value={prefs.fondoLibre}
                onChange={(e) => canvasPrefs.setFondoLibre(e.target.value)}
                className="w-6 h-6 rounded cursor-pointer bg-transparent border-0"
                title={t('apariencia.colorPropio.ayuda')}
              />
              <span className="text-[10px] font-mono opacity-70 uppercase">
                {prefs.fondoId === 'libre' ? prefs.fondoLibre : fondoActivo?.color || ''}
              </span>
              {prefs.fondoId !== 'libre' && fondoActivo && (
                <span className="text-[10px] ml-auto opacity-70">{fondoActivo.nombre}</span>
              )}
            </div>

            {/* Tarjetas */}
            <label className="text-[10px] font-medium uppercase tracking-wider opacity-70 block mt-3 mb-1.5">
              Tarjetas
            </label>
            <div className="grid grid-cols-3 gap-1 p-1 rounded-lg border border-[var(--nf-hud-border)]">
              {MODOS_TARJETA.map((m) => (
                <button
                  key={m.id}
                  type="button"
                  onClick={() => canvasPrefs.setTarjetas(m.id)}
                  title={m.ayuda}
                  className={`py-1 text-[10px] rounded transition-colors cursor-pointer ${
                    prefs.tarjetas === m.id
                      ? 'bg-indigo-600 text-white font-medium'
                      : 'hover:bg-[var(--nf-hud-hover)]'
                  }`}
                >
                  {m.nombre}
                </button>
              ))}
            </div>
            <p className="text-[9px] opacity-60 mt-1.5 leading-snug">
              El color de cada nodo se edita en el nodo (es un dato del concepto).
            </p>
          </div>
        </>
      )}
    </div>
  );
};
