import React, { useCallback, useEffect, useRef, useState } from 'react';
import { X, Brain, Loader2, Sparkles, AlertTriangle, CheckCircle2, Network, Clock } from 'lucide-react';
import { apiUrl } from '../services/apiBase';

/** Un turno del cerebro: lo que el agente respondió y **qué contexto** se le mandó. */
export interface TurnoCerebro {
  pedido?: string;
  ok?: boolean;
  salida?: string;
  ms?: number;
  /** Visión, recuerdos dirigidos, foco y contadores: el prompt no es una caja negra. */
  contexto?: string;
}

interface Props {
  isOpen: boolean;
  onClose: () => void;
  showToast: (text: string, type?: 'success' | 'info' | 'error') => void;
  /** Títulos del lienzo para sembrar un pedido (el padre ya los recorta). */
  sugerencias?: string[];
}

/**
 * «Pensar desde el lienzo»: la boca de la app hacia el cerebro residente.
 *
 * Corre en la **sesión nombrada** de Hermes (`nf-cerebro`), así que cada turno recuerda los anteriores, y
 * el contexto lo arma la app desde la bóveda (visión + recuerdo dirigido + foco), no el chat. Cada turno
 * deja una **nota episódica** en `<bóveda>/cerebro/`, y la respuesta se puede volcar al lienzo como
 * propuesta — el humano decide, igual que con la investigación.
 */
export const CerebroPanel: React.FC<Props> = ({ isOpen, onClose, showToast, sugerencias = [] }) => {
  const [pedido, setPedido] = useState('');
  const [corriendo, setCorriendo] = useState(false);
  const [segundos, setSegundos] = useState(0);
  const [turno, setTurno] = useState<TurnoCerebro | null>(null);
  const [sesion, setSesion] = useState('');
  const [notas, setNotas] = useState(0);
  const [proponiendo, setProponiendo] = useState(false);
  const [verContexto, setVerContexto] = useState(false);
  const pedidoEnCurso = useRef('');
  const corriendoAntes = useRef(false);

  const traer = useCallback(async () => {
    try {
      const d = await (await fetch(apiUrl('/api/ai/delegar'))).json();
      setCorriendo(Boolean(d?.corriendo));
      setSesion(String(d?.cerebro?.sesion || ''));
      setNotas(Number(d?.cerebro?.notas || 0));
      const r = (d?.resultado || null) as TurnoCerebro | null;
      if (r && r.salida) {
        setTurno(r);
        if (!pedidoEnCurso.current && r.pedido) pedidoEnCurso.current = String(r.pedido);
      }
      // El turno terminó mientras el panel estaba abierto: se avisa una sola vez.
      if (corriendoAntes.current && !d?.corriendo && r?.salida) {
        showToast(`El cerebro respondió en ${((r.ms || 0) / 1000).toFixed(0)} s.`, 'success');
      }
      corriendoAntes.current = Boolean(d?.corriendo);
    } catch {
      /* el backend puede estar ocupado: el poller vuelve solo */
    }
  }, [showToast]);

  // Abrir el panel: se ve el último turno (no un modal vacío) y, si sigue corriendo, se lo sigue.
  useEffect(() => {
    if (!isOpen) return;
    let vivo = true;
    void traer();
    const t = setInterval(() => {
      if (vivo) void traer();
    }, 3000);
    return () => {
      vivo = false;
      clearInterval(t);
    };
  }, [isOpen, traer]);

  // Cronómetro: un turno real tarda decenas de segundos. Un spinner sin tiempo se lee como cuelgue.
  useEffect(() => {
    if (!corriendo) return;
    setSegundos(0);
    const t = setInterval(() => setSegundos((s) => s + 1), 1000);
    return () => clearInterval(t);
  }, [corriendo]);

  const pensar = useCallback(
    async (texto: string) => {
      const limpio = texto.trim();
      if (limpio.length < 4 || corriendo) return;
      pedidoEnCurso.current = limpio;
      setPedido('');
      setCorriendo(true);
      try {
        const r = await fetch(apiUrl('/api/ai/delegar'), {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ pedido: limpio }),
        });
        if (!r.ok) {
          showToast('El cerebro no pudo arrancar: probá de nuevo.', 'error');
          setCorriendo(false);
        }
      } catch {
        showToast('El backend no respondió.', 'error');
        setCorriendo(false);
      }
    },
    [corriendo, showToast]
  );

  /** La respuesta ya es conocimiento del proyecto: va a la cola como nodo, con el contexto usado adentro. */
  const proponerNodo = useCallback(async () => {
    if (!turno?.salida || proponiendo) return;
    setProponiendo(true);
    const titulo = String(turno.pedido || pedidoEnCurso.current || 'Turno del cerebro').slice(0, 70);
    try {
      const r = await fetch(apiUrl('/api/graph/node'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          title: titulo,
          description: `${turno.salida}${turno.contexto ? `\n\nContexto usado: ${turno.contexto}` : ''}`,
          category: 'ARQUITECTURA',
          maturity: 3,
          parent: 'Norte Estratégico · NodeFlow',
          link_label: 'alimenta',
          prompt_original: 'Cerebro residente',
        }),
      });
      const d = await r.json().catch(() => null);
      if (!r.ok || d?.success === false) showToast(d?.error || 'No se pudo proponer el nodo.', 'error');
      else showToast('1 propuesta en «Cambios del agente»: el turno, con su contexto adentro.', 'success');
    } catch {
      showToast('El backend no respondió al proponer.', 'error');
    } finally {
      setProponiendo(false);
    }
  }, [turno, proponiendo, showToast]);

  if (!isOpen) return null;

  const seg = turno?.ms ? Math.round(turno.ms / 1000) : 0;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm"
      id="cerebro-panel"
    >
      <div className="relative w-full max-w-3xl max-h-[88vh] overflow-y-auto rounded-2xl border border-slate-700 bg-slate-900 shadow-2xl">
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800">
          <div className="flex items-center gap-2">
            <Brain size={16} className="text-cyan-400" />
            <h2 className="text-sm font-medium text-slate-100">Pensar desde el lienzo</h2>
            <span className="text-[11px] text-slate-400">
              sesión {sesion || '—'} · {notas} nota(s) de turno en la bóveda
            </span>
          </div>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-white transition-colors cursor-pointer"
            title="Cerrar"
          >
            <X size={16} />
          </button>
        </div>

        <div className="p-5 space-y-4">
          {/* La boca: un pedido real, con el cerebro que ya conoce el proyecto */}
          <div className="rounded-xl border border-slate-700 bg-slate-800/60 p-3 space-y-2">
            <div className="flex items-center gap-2">
              <Brain size={13} className="text-cyan-400 shrink-0" />
              <span className="text-xs text-slate-100 font-medium">Pedirle algo al cerebro</span>
              <span className="text-[11px] text-slate-400">
                · corre en la app, con las herramientas y la bóveda a mano; recuerda los turnos anteriores
              </span>
            </div>
            <div className="flex items-start gap-2">
              <textarea
                value={pedido}
                onChange={(e) => setPedido(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault();
                    void pensar(pedido);
                  }
                }}
                rows={2}
                placeholder="Ej: ¿por dónde sigo con el cerebro local, mirando lo que ya hay en el lienzo?"
                className="flex-1 px-3 py-1.5 rounded-xl bg-slate-900/70 border border-slate-700 text-xs text-slate-100 placeholder:text-slate-500 focus:outline-none focus:border-cyan-700 resize-y"
              />
              <button
                type="button"
                onClick={() => void pensar(pedido)}
                disabled={corriendo || pedido.trim().length < 4}
                className="flex items-center gap-2 px-3 py-1.5 rounded-xl text-xs font-medium bg-cyan-950/70 text-cyan-200 hover:bg-cyan-900/80 border border-cyan-800/60 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer shrink-0"
                title={corriendo ? 'El cerebro está en un turno' : 'Corre un turno en la sesión del cerebro (Enter)'}
              >
                {corriendo ? <Loader2 size={13} className="animate-spin" /> : <Sparkles size={13} />}
                {corriendo ? `Pensando… ${segundos}s` : 'Pensar'}
              </button>
            </div>
            {sugerencias.length > 0 && (
              <div className="flex flex-wrap gap-1.5 pt-0.5">
                {sugerencias.slice(0, 6).map((t) => (
                  <button
                    key={t}
                    type="button"
                    onClick={() => setPedido(`¿Qué hago con «${t}» mirando el resto del lienzo?`)}
                    disabled={corriendo}
                    title={`Sembrar un pedido sobre: ${t}`}
                    className="px-2 py-0.5 rounded-lg text-[10px] font-mono text-slate-300 bg-slate-900/70 border border-slate-700 hover:border-cyan-700 hover:text-cyan-200 disabled:opacity-40 cursor-pointer truncate max-w-[220px]"
                  >
                    {t}
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* El turno en curso */}
          {corriendo && (
            <div className="rounded-xl border border-cyan-900/50 bg-cyan-950/20 p-3 flex items-start gap-2">
              <Loader2 size={14} className="text-cyan-300 animate-spin mt-0.5 shrink-0" />
              <div className="space-y-1">
                <p className="text-xs text-cyan-100">El cerebro está en un turno…</p>
                <p className="text-[11px] text-slate-400">
                  {pedidoEnCurso.current || 'turno en curso'} · {segundos}s. Puede tardar minutos: consulta
                  el lienzo y la bóveda antes de responder.
                </p>
              </div>
            </div>
          )}

          {/* La respuesta */}
          {turno?.salida && !corriendo && (
            <div className="rounded-xl border border-slate-700 bg-slate-800/60 p-3 space-y-2">
              <div className="flex items-center gap-2">
                {turno.ok ? (
                  <CheckCircle2 size={13} className="text-emerald-400 shrink-0" />
                ) : (
                  <AlertTriangle size={13} className="text-amber-400 shrink-0" />
                )}
                <span className="text-xs text-slate-100 font-medium">Turno {turno.ok ? 'completo' : 'fallido'}</span>
                {seg > 0 && (
                  <span className="flex items-center gap-1 text-[11px] text-slate-400">
                    <Clock size={11} /> {seg}s
                  </span>
                )}
                {turno.pedido && <span className="text-[11px] text-slate-500 truncate">· {turno.pedido}</span>}
              </div>
              <p className="text-xs text-slate-200 whitespace-pre-wrap leading-relaxed" id="cerebro-respuesta">
                {turno.salida}
              </p>

              {turno.contexto && (
                <div className="pt-1">
                  <button
                    type="button"
                    onClick={() => setVerContexto((v) => !v)}
                    className="text-[10px] uppercase tracking-widest text-slate-500 hover:text-cyan-300 font-bold cursor-pointer"
                    title="Qué visión, recuerdos y foco se le mandaron al agente"
                  >
                    {verContexto ? '▾' : '▸'} contexto que usó
                  </button>
                  {verContexto && (
                    <p className="mt-1 text-[11px] text-slate-400 font-mono break-words" id="cerebro-contexto">
                      {turno.contexto}
                    </p>
                  )}
                </div>
              )}

              <div className="flex items-center gap-2 pt-1">
                <button
                  type="button"
                  onClick={() => void proponerNodo()}
                  disabled={proponiendo}
                  className="flex items-center gap-2 px-3 py-1.5 rounded-xl text-xs font-medium bg-slate-900/70 text-slate-200 hover:bg-slate-800 border border-slate-700 disabled:opacity-40 cursor-pointer"
                  title="Propone el turno como nodo colgado del Norte (queda en «Cambios del agente»)"
                >
                  {proponiendo ? <Loader2 size={13} className="animate-spin" /> : <Network size={13} />}
                  Proponer como nodo
                </button>
                <button
                  type="button"
                  onClick={() => {
                    void navigator.clipboard?.writeText(turno.salida || '');
                    showToast('Copiado.', 'info');
                  }}
                  className="px-3 py-1.5 rounded-xl text-xs font-medium bg-slate-900/70 text-slate-300 hover:bg-slate-800 border border-slate-700 cursor-pointer"
                >
                  Copiar
                </button>
              </div>
            </div>
          )}

          <p className="text-[11px] text-slate-500">
            Cada turno deja una nota con fecha en <span className="font-mono">cerebro/</span> — la memoria del
            proyecto crece en la bóveda, no en el chat. Nada toca el lienzo sin tu aprobación.
          </p>
        </div>
      </div>
    </div>
  );
};

export default CerebroPanel;
