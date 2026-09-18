import React, { useMemo } from 'react';
import { X, History, HelpCircle, Scale, Clock, RefreshCw } from 'lucide-react';
import { CustomNode, MATURITY_CONFIGS } from '../types';

interface RetomarPanelProps {
  isOpen: boolean;
  onClose: () => void;
  nodos: CustomNode[];
  /** id de nodo → instante (ms) de la última vez que lo miraste. Vive en localStorage. */
  vistos: Record<string, number>;
  onIr: (nodoId: string) => void;
}

const DIAS = 24 * 60 * 60 * 1000;

function diasDesde(ts?: number): string {
  if (!ts) return 'nunca';
  const d = Math.floor((Date.now() - ts) / DIAS);
  if (d <= 0) return 'hoy';
  if (d === 1) return 'ayer';
  return `hace ${d} días`;
}

/**
 * «Retomar»: por dónde seguir.
 *
 * Un lienzo de pensamiento con 60+ nodos no dice dónde estabas. Esto ordena el trabajo por lo que
 * está **abierto**: preguntas sin responder, ideas esperando tu decisión, lo último que miraste y lo
 * que dejaste olvidado. Nada se calcula con IA: son los datos que el propio grafo ya tiene.
 */
export const RetomarPanel: React.FC<RetomarPanelProps> = ({ isOpen, onClose, nodos, vistos, onIr }) => {
  const listas = useMemo(() => {
    const utiles = nodos.filter((n) => !n.data.macro && !n.data.isRoot);
    const abiertas = utiles.filter((n) => n.data.pregunta?.estado === 'abierta');
    const sinDecidir = utiles.filter(
      (n) =>
        !n.data.decision &&
        !n.data.respuestaDe &&
        !n.data.pregunta &&
        (n.data.maturity || 1) <= 2
    );
    const respondidas = utiles
      .filter((n) => n.data.respuestaDe || n.data.pregunta?.estado === 'respondida')
      .sort((a, b) => (vistos[b.id] || 0) - (vistos[a.id] || 0));
    const ultimos = utiles
      .filter((n) => vistos[n.id])
      .sort((a, b) => (vistos[b.id] || 0) - (vistos[a.id] || 0))
      .slice(0, 5);
    const olvidados = utiles
      .filter((n) => !n.data.respuestaDe && (!vistos[n.id] || Date.now() - vistos[n.id] > 14 * DIAS))
      .sort((a, b) => (vistos[a.id] || 0) - (vistos[b.id] || 0))
      .slice(0, 6);
    return { abiertas, sinDecidir, respondidas, ultimos, olvidados };
  }, [nodos, vistos]);

  if (!isOpen) return null;

  const Fila: React.FC<{ n: CustomNode; nota?: string }> = ({ n, nota }) => {
    const cfg = MATURITY_CONFIGS[(n.data.maturity || 1) as 1 | 2 | 3 | 4 | 5];
    return (
      <button
        type="button"
        onClick={() => {
          onIr(n.id);
          onClose();
        }}
        className="w-full text-left rounded-lg border border-slate-800 bg-slate-950/40 hover:border-slate-600 hover:bg-slate-900/60 px-3 py-2 transition-colors cursor-pointer"
      >
        <div className="flex items-center gap-2">
          <span className="text-[10px] shrink-0">{cfg?.icon}</span>
          <span className="text-xs text-slate-200 truncate flex-1">{n.data.title || '(sin título)'}</span>
          <span className="text-[9px] font-mono text-slate-500 shrink-0">{nota || diasDesde(vistos[n.id])}</span>
        </div>
      </button>
    );
  };

  const Seccion: React.FC<{ icono: React.ReactNode; titulo: string; cuenta: number; children: React.ReactNode; vacio: string }> = ({
    icono,
    titulo,
    cuenta,
    children,
    vacio,
  }) => (
    <div>
      <div className="flex items-center gap-1.5 mb-1.5">
        <span className="text-slate-400">{icono}</span>
        <span className="text-[10px] uppercase tracking-widest text-slate-500 font-bold">{titulo}</span>
        <span className="text-[9px] font-mono px-1.5 py-0.5 rounded border border-slate-700 text-slate-400 ml-auto">
          {cuenta}
        </span>
      </div>
      {cuenta === 0 ? (
        <p className="text-[11px] text-slate-600 px-1">{vacio}</p>
      ) : (
        <div className="space-y-1.5">{children}</div>
      )}
    </div>
  );

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm">
      <div className="relative w-full max-w-2xl max-h-[88vh] overflow-hidden flex flex-col bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl">
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-amber-500/10 border border-amber-500/30 flex items-center justify-center text-amber-300">
              <History size={16} />
            </div>
            <div>
              <div className="text-sm font-semibold text-slate-200">Retomar</div>
              <div className="text-[11px] text-slate-400">
                Por dónde seguir, según lo que está abierto en el lienzo
              </div>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-2 text-slate-400 hover:text-slate-200 rounded-lg hover:bg-slate-800 cursor-pointer"
          >
            <X size={16} />
          </button>
        </div>

        <div className="p-5 overflow-y-auto space-y-5 flex-1">
          <Seccion
            icono={<HelpCircle size={12} />}
            titulo="Preguntas abiertas"
            cuenta={listas.abiertas.length}
            vacio="Ninguna: no hay preguntas sin responder."
          >
            {listas.abiertas.map((n) => (
              <Fila key={n.id} n={n} nota="responder" />
            ))}
          </Seccion>

          <Seccion
            icono={<Scale size={12} />}
            titulo="Esperando tu decisión"
            cuenta={listas.sinDecidir.length}
            vacio="Nada pendiente de decidir."
          >
            {listas.sinDecidir.map((n) => (
              <Fila key={n.id} n={n} nota="aceptar / descartar" />
            ))}
          </Seccion>

          <Seccion
            icono={<Clock size={12} />}
            titulo="Lo último que miraste"
            cuenta={listas.ultimos.length}
            vacio="Todavía no hay historial de visitas."
          >
            {listas.ultimos.map((n) => (
              <Fila key={n.id} n={n} />
            ))}
          </Seccion>

          <Seccion
            icono={<RefreshCw size={12} />}
            titulo="Olvidado hace tiempo"
            cuenta={listas.olvidados.length}
            vacio="Nada quedó sin mirar."
          >
            {listas.olvidados.map((n) => (
              <Fila key={n.id} n={n} nota={diasDesde(vistos[n.id])} />
            ))}
          </Seccion>
        </div>

        <div className="px-5 py-3 border-t border-slate-800 bg-slate-950/60">
          <p className="text-[10px] text-slate-500">
            Las visitas se guardan en tu navegador, no en la bóveda: es memoria de uso, no conocimiento.
          </p>
        </div>
      </div>
    </div>
  );
};
