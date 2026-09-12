import React, { useCallback, useEffect, useState } from 'react';
import {
  Sprout,
  X,
  RefreshCw,
  Loader2,
  Check,
  Wand2,
  LayoutGrid,
  AlertTriangle,
  Link2,
  Info,
} from 'lucide-react';
import {
  escanear,
  proponerArreglos,
  reacomodar,
  TIPOS,
  EscaneoJardin,
} from '../services/jardinService';

interface JardinPanelProps {
  isOpen: boolean;
  onClose: () => void;
  /** Se llama cuando el jardín encoló propuestas (para refrescar el panel de aprobación). */
  onPropuestasCreadas: () => void;
  showToast: (message: string, type?: 'success' | 'error' | 'info') => void;
}

const colorGravedad = (g: string) =>
  g === 'alta'
    ? 'text-rose-300 border-rose-500/40 bg-rose-500/10'
    : g === 'media'
      ? 'text-amber-300 border-amber-500/40 bg-amber-500/10'
      : 'text-slate-300 border-slate-600/50 bg-slate-700/20';

const iconoGravedad = (g: string) =>
  g === 'alta' ? 'text-rose-400' : g === 'media' ? 'text-amber-400' : 'text-slate-400';

export const JardinPanel: React.FC<JardinPanelProps> = ({
  isOpen,
  onClose,
  onPropuestasCreadas,
  showToast,
}) => {
  const [escaneo, setEscaneo] = useState<EscaneoJardin | null>(null);
  const [cargando, setCargando] = useState(false);
  const [ocupado, setOcupado] = useState<string | null>(null);

  const refrescar = useCallback(async () => {
    setCargando(true);
    const r = await escanear();
    setCargando(false);
    if (!r) {
      showToast('No pude leer el jardín (¿el backend está vivo?)', 'error');
      return;
    }
    setEscaneo(r);
  }, [showToast]);

  useEffect(() => {
    if (isOpen && !escaneo) void refrescar();
  }, [isOpen, escaneo, refrescar]);

  if (!isOpen) return null;

  const arreglar = async () => {
    setOcupado('fix');
    const r = await proponerArreglos();
    setOcupado(null);
    if (!r || r.ok === false) {
      showToast(r?.error || 'No pude proponer los arreglos', 'error');
      return;
    }
    const n = r.propuestas?.length ?? 0;
    if (n === 0) {
      showToast(r.mensaje || 'No hay hallazgos accionables: nada que proponer', 'info');
    } else {
      showToast(`${n} propuesta(s) del jardín — aprobalas en «Cambios del agente»`, 'success');
      onPropuestasCreadas();
    }
    void refrescar();
  };

  const ordenar = async () => {
    setOcupado('tidy');
    const r = await reacomodar();
    setOcupado(null);
    if (!r || r.ok === false) {
      showToast(r?.error || 'No pude proponer el reacomodo', 'error');
      return;
    }
    if (r.accion === 'ya_ordenado' || r.mensaje) {
      showToast(r.mensaje || 'El lienzo ya está en niveles', 'info');
    } else {
      showToast('Reacomodo propuesto — aprobalo en «Cambios del agente»', 'success');
      onPropuestasCreadas();
    }
  };

  const problemas = escaneo?.problemas ?? [];
  const arreglables = problemas.filter((p) => TIPOS[p.tipo]?.arreglable !== false && p.accion !== 'nada');
  const s = escaneo?.stats ?? {};

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-fade-in">
      <div
        id="jardin-panel"
        className="w-full max-w-2xl max-h-[88vh] flex flex-col bg-slate-900 border border-slate-700/60 rounded-2xl shadow-2xl overflow-hidden"
      >
        {/* Encabezado + veredicto */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800 bg-slate-950/60">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-lime-500 to-emerald-600 flex items-center justify-center">
              <Sprout className="w-5 h-5 text-white" />
            </div>
            <div>
              <h2 className="text-sm font-bold text-white flex items-center gap-2">
                Jardín del lienzo
                {escaneo && (
                  <span
                    className={`text-[10px] px-2 py-0.5 rounded-full border uppercase tracking-wide ${
                      escaneo.sano
                        ? 'text-emerald-300 border-emerald-500/40 bg-emerald-500/10'
                        : 'text-amber-300 border-amber-500/40 bg-amber-500/10'
                    }`}
                  >
                    {escaneo.sano ? 'sano' : `${problemas.length} hallazgo(s)`}
                  </span>
                )}
              </h2>
              <p className="text-[11px] text-slate-400">
                Diagnostica y propone. Vos aprobás: el jardín nunca escribe solo.
              </p>
            </div>
          </div>
          <div className="flex items-center gap-1">
            <button
              onClick={() => void refrescar()}
              disabled={cargando}
              title="Volver a escanear"
              className="p-2 text-slate-400 hover:text-white rounded-lg hover:bg-slate-800 transition-colors disabled:opacity-40"
            >
              {cargando ? <Loader2 className="w-4 h-4 animate-spin" /> : <RefreshCw className="w-4 h-4" />}
            </button>
            <button
              onClick={onClose}
              className="p-2 text-slate-400 hover:text-white rounded-lg hover:bg-slate-800 transition-colors"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Cuerpo */}
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-3">
          {!escaneo && cargando && (
            <p className="text-[11px] text-slate-400 py-8 text-center">
              <Loader2 className="w-4 h-4 animate-spin inline mr-2" />
              Revisando invariantes y hallazgos…
            </p>
          )}

          {escaneo?.error && (
            <p className="text-[11px] text-rose-300 py-6 text-center">{escaneo.error}</p>
          )}

          {escaneo && (
            <>
              {/* Cifras */}
              <div className="grid grid-cols-5 gap-2">
                {[
                  ['nodos', 'nodos'],
                  ['aristas', 'aristas'],
                  ['huerfanos', 'sin conexiones'],
                  ['sin_descripcion', 'sin descripción'],
                  ['sin_madurez', 'sin madurez'],
                ].map(([k, label]) => (
                  <div
                    key={k}
                    className="rounded-lg border border-slate-800 bg-slate-950/50 px-2 py-2 text-center"
                  >
                    <p className="text-base font-bold text-slate-100">{s[k as keyof typeof s] ?? '—'}</p>
                    <p className="text-[9px] text-slate-500 leading-tight">{label}</p>
                  </div>
                ))}
              </div>

              {/* Hallazgos */}
              {problemas.length === 0 && (
                <div className="flex items-center gap-2 rounded-xl border border-emerald-500/30 bg-emerald-500/5 px-3 py-3">
                  <Check className="w-4 h-4 text-emerald-400 shrink-0" />
                  <p className="text-[11px] text-emerald-200">
                    Sin hallazgos: invariantes en orden y el grafo conectado.
                  </p>
                </div>
              )}

              {problemas.map((p, i) => (
                <div
                  key={i}
                  className={`rounded-xl border px-3 py-3 ${colorGravedad(p.gravedad)}`}
                >
                  <div className="flex items-start gap-2">
                    <AlertTriangle className={`w-4 h-4 shrink-0 mt-0.5 ${iconoGravedad(p.gravedad)}`} />
                    <div className="min-w-0">
                      <p className="text-xs font-semibold">
                        {TIPOS[p.tipo]?.etiqueta ?? p.tipo}
                        <span className="ml-2 text-[10px] font-normal opacity-70 uppercase">
                          {p.gravedad}
                        </span>
                      </p>
                      <p className="text-[11px] text-slate-300 mt-0.5">{p.detalle}</p>
                      <p className="text-[10px] text-slate-500 mt-1">
                        {TIPOS[p.tipo]?.arreglable === false || p.accion === 'nada'
                          ? 'Requiere criterio humano: el jardín no propone cambios automáticos.'
                          : `El jardín propone: ${p.accion}`}
                      </p>
                    </div>
                  </div>
                </div>
              ))}

              {/* Conexiones sugeridas */}
              {(escaneo.padrinos?.length ?? 0) > 0 && (
                <div className="rounded-xl border border-slate-800 bg-slate-950/50 p-3">
                  <p className="text-xs font-semibold text-slate-200 flex items-center gap-2 mb-2">
                    <Link2 className="w-4 h-4 text-sky-300" /> Conexiones sugeridas (
                    {escaneo.padrinos.length})
                  </p>
                  {escaneo.padrinos.map((p, i) => (
                    <p key={i} className="text-[11px] text-slate-300">
                      <span className="text-slate-100">{p.titulo ?? p.nodo}</span>
                      {' → '}
                      <span className="text-sky-200">{p.padre_titulo ?? p.padre_sugerido}</span>
                      {typeof p.similitud === 'number' && (
                        <span className="text-slate-500"> · afinidad {p.similitud.toFixed(2)}</span>
                      )}
                      {p.confianza && <span className="text-slate-500"> · {p.confianza}</span>}
                    </p>
                  ))}
                </div>
              )}

              {/* Acciones */}
              <div className="grid grid-cols-2 gap-2 pt-1">
                <button
                  onClick={() => void arreglar()}
                  disabled={ocupado !== null || arreglables.length === 0}
                  title={
                    arreglables.length === 0
                      ? 'No hay hallazgos accionables ahora mismo'
                      : 'Convierte los hallazgos en propuestas'
                  }
                  className="flex items-center justify-center gap-2 px-3 py-2 text-[11px] font-semibold rounded-lg bg-lime-600/20 hover:bg-lime-600/35 text-lime-200 border border-lime-500/30 disabled:opacity-40 transition-colors"
                >
                  {ocupado === 'fix' ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    <Wand2 className="w-3.5 h-3.5" />
                  )}
                  Proponer {arreglables.length > 0 ? arreglables.length : ''} arreglo(s)
                </button>
                <button
                  onClick={() => void ordenar()}
                  disabled={ocupado !== null}
                  className="flex items-center justify-center gap-2 px-3 py-2 text-[11px] font-semibold rounded-lg bg-sky-600/20 hover:bg-sky-600/35 text-sky-200 border border-sky-500/30 disabled:opacity-40 transition-colors"
                >
                  {ocupado === 'tidy' ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    <LayoutGrid className="w-3.5 h-3.5" />
                  )}
                  Reacomodar por niveles
                </button>
              </div>

              <p className="text-[10px] text-slate-500 flex items-start gap-1.5">
                <Info className="w-3.5 h-3.5 text-sky-400 shrink-0 mt-0.5" />
                El reacomodo mueve todos los nodos: el <span className="font-mono">undo</span> de la
                app no lo cubre, los respaldos rotativos de{' '}
                <span className="font-mono">.nodeflow/</span> son el camino de vuelta.
              </p>
            </>
          )}
        </div>

        <div className="px-5 py-3 border-t border-slate-800 bg-slate-950/60 flex items-center justify-between">
          <p className="text-[10px] text-slate-500">
            Rechazar al agente, reparar al humano: los invariantes nunca borran tu trabajo.
          </p>
          <button
            onClick={onClose}
            className="px-4 py-1.5 text-[11px] rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-200 transition-colors"
          >
            Cerrar
          </button>
        </div>
      </div>
    </div>
  );
};
