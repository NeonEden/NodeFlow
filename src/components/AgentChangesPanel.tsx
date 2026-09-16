import React, { useState } from 'react';
import { Bot, Check, X, ShieldAlert, Loader2, Clock, ArrowRight, Inbox } from 'lucide-react';
import {
  Propuesta,
  Peligro,
  aprobarPropuestas,
  rechazarPropuestas,
} from '../services/agentService';
import { apiUrl } from '../services/apiBase';
import { useIdioma } from '../i18n/useIdioma';

interface AgentChangesPanelProps {
  isOpen: boolean;
  onClose: () => void;
  propuestas: Propuesta[];
  onResolved: () => void;
  showToast: (message: string, type?: 'success' | 'error' | 'info') => void;
}

const COLOR_PELIGRO: Record<Peligro, string> = {
  bajo: 'bg-emerald-500/15 text-emerald-300 border-emerald-500/30',
  medio: 'bg-amber-500/15 text-amber-300 border-amber-500/30',
  alto: 'bg-rose-500/15 text-rose-300 border-rose-500/30',
};

const ICONO_TIPO: Record<string, string> = {
  nodo: '◈',
  conectar: '→',
  borrar: '✕',
  sanear: '⌗',
  reacomodar: '⇅',
  herramienta: '⚒',
  fusionar: '⇥',
};

function hace(ms: number): string {
  if (!ms) return '';
  const seg = Math.max(0, Math.round((Date.now() - ms) / 1000));
  if (seg < 60) return `hace ${seg}s`;
  if (seg < 3600) return `hace ${Math.round(seg / 60)}min`;
  return `hace ${Math.round(seg / 3600)}h`;
}

export const AgentChangesPanel: React.FC<AgentChangesPanelProps> = ({
  isOpen,
  onClose,
  propuestas,
  onResolved,
  showToast,
}) => {
  const { t } = useIdioma();
  const [ocupado, setOcupado] = useState<string | null>(null);

  /** Fase 5.5 — el curador: mira el lienzo y propone fusiones y podas con motivo. No aplica nada. */
  const curar = async () => {
    setOcupado('curar');
    try {
      const r = await fetch(apiUrl('/api/cerebro/curaduria'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: '{}',
      });
      const d = await r.json();
      if (d?.success) {
        const n = (d.propuestas || []).length;
        const decl = (d.declarados || []).length;
        showToast(
          n > 0
            ? `Curaduría: ${n} propuesta(s) con motivo${decl ? ` y ${decl} aviso(s) de lo que no toqué` : ''}.`
            : 'El lienzo no tiene ruido mecánico (sin duplicados, fragmentos ni sueltos).',
          n > 0 ? 'info' : 'success',
        );
        onResolved();
      } else {
        showToast(String(d?.error || 'No pude curar el lienzo.'), 'error');
      }
    } catch {
      showToast('No pude curar el lienzo (¿el backend está corriendo?).', 'error');
    } finally {
      setOcupado(null);
    }
  };

  if (!isOpen) return null;

  const resolver = async (accion: 'approve' | 'reject', payload: { id?: string; todos?: boolean }) => {
    const clave = payload.todos ? `todos-${accion}` : `${accion}-${payload.id}`;
    setOcupado(clave);
    const res = accion === 'approve'
      ? await aprobarPropuestas(payload)
      : await rechazarPropuestas(payload);
    setOcupado(null);

    if (!res) {
      showToast('No pude hablar con el backend de NodeFlow', 'error');
      return;
    }
    if (res.accion === 'nada_pendiente') {
      showToast('No había nada pendiente', 'info');
      onResolved();
      return;
    }
    const verbo = accion === 'approve' ? 'Aprobadas' : 'Rechazadas';
    const fallos = res.errores?.length || 0;
    showToast(
      `${verbo}: ${res.cantidad}${fallos ? ` · ${fallos} quedaron pendientes con error` : ''}`,
      fallos ? 'error' : 'success'
    );
    onResolved();
  };

  const total = propuestas.length;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-fade-in">
      <div
        id="agent-changes-panel"
        className="w-full max-w-3xl max-h-[86vh] flex flex-col bg-slate-900 border border-slate-700/60 rounded-2xl shadow-2xl overflow-hidden"
      >
        {/* Encabezado */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800 bg-slate-950/60">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-indigo-500 to-violet-600 flex items-center justify-center">
              <Bot className="w-5 h-5 text-white" />
            </div>
            <div>
              <h2 className="text-sm font-bold text-white flex items-center gap-2">
                Cambios del agente
                {total > 0 && (
                  <span className="px-2 py-0.5 text-[10px] rounded-full bg-indigo-500/20 text-indigo-300 border border-indigo-500/30">
                    {total} pendiente{total === 1 ? '' : 's'}
                  </span>
                )}
              </h2>
              <p className="text-[11px] text-slate-400">
                El agente propone; nada toca el lienzo hasta que lo apruebes.
              </p>
            </div>
          </div>
          <button
            onClick={() => void curar()}
            disabled={ocupado === 'curar'}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-slate-900/70 text-slate-200 hover:bg-slate-800 border border-slate-700 disabled:opacity-40 cursor-pointer"
            title="El curador mira el lienzo y propone fusiones y podas, cada una con su motivo y su evidencia. No aplica nada: entran acá."
          >
            {ocupado === 'curar' ? <Loader2 size={13} className="animate-spin" /> : <Bot size={13} />}
            Curar lienzo
          </button>
          <button
            onClick={onClose}
            className="p-2 text-slate-400 hover:text-white rounded-lg hover:bg-slate-800 transition-colors"
            title={t('modal.cerrarCorto')}
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Cuerpo */}
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-3">
          {total === 0 ? (
            <div className="flex flex-col items-center justify-center py-14 text-center">
              <Inbox className="w-10 h-10 text-slate-600 mb-3" />
              <p className="text-sm text-slate-300">{t('agente.sinPendientes')}</p>
              <p className="text-[11px] text-slate-500 mt-1 max-w-md">
                Cuando el agente escriba en el lienzo (por MCP), cada escritura va a aparecer acá con su
                vista previa para que la apruebes o la rechaces. El panel se refresca solo cada 3 s.
              </p>
            </div>
          ) : (
            propuestas.map((p) => {
              const v = p.vista || ({} as Propuesta['vista']);
              const peligro: Peligro = (v.peligro as Peligro) || 'bajo';
              const trabajando = ocupado === `approve-${p.id}` || ocupado === `reject-${p.id}`;
              return (
                <div
                  key={p.id}
                  className="rounded-xl border border-slate-800 bg-slate-950/50 p-4 hover:border-slate-700 transition-colors"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <div className="flex items-center gap-2 mb-1.5 flex-wrap">
                        <span className="text-[10px] font-mono text-slate-500">
                          {ICONO_TIPO[p.tipo] || '•'} {p.id}
                        </span>
                        <span className={`px-2 py-0.5 text-[10px] rounded-full border ${COLOR_PELIGRO[peligro]}`}>
                          {v.accion_legible || p.tipo} · peligro {peligro}
                        </span>
                        <span className="text-[10px] text-slate-500 flex items-center gap-1">
                          <Clock className="w-3 h-3" />
                          {hace(p.creado_ms)}
                        </span>
                      </div>
                      <p className="text-sm text-slate-100 leading-snug">{v.resumen}</p>

                      {v.cambios && v.cambios.length > 0 && (
                        <ul className="mt-2 space-y-0.5">
                          {v.cambios.map((c, i) => (
                            <li key={i} className="text-[11px] text-slate-400 flex items-start gap-1.5">
                              <ArrowRight className="w-3 h-3 mt-0.5 text-indigo-400 shrink-0" />
                              {c}
                            </li>
                          ))}
                        </ul>
                      )}

                      {v.despues?.descripcion ? (
                        <p className="mt-2 text-[11px] text-slate-400 bg-slate-900/70 border border-slate-800 rounded-lg p-2 max-h-24 overflow-y-auto">
                          {String(v.despues.descripcion)}
                        </p>
                      ) : null}

                      {p.payload?.evidencia ? (
                        <p className="mt-1 text-[10px] text-slate-500 font-mono">{p.payload.evidencia}</p>
                      ) : null}
                      {p.motivo ? (
                        <p className="mt-2 text-[11px] text-slate-500 italic">motivo: {p.motivo}</p>
                      ) : null}

                      {peligro === 'alto' && (
                        <p className="mt-2 text-[11px] text-rose-300 flex items-center gap-1.5">
                          <ShieldAlert className="w-3.5 h-3.5" />
                          Borra contenido del lienzo. Revisá antes de aprobar.
                        </p>
                      )}
                    </div>

                    <div className="flex flex-col gap-2 shrink-0">
                      <button
                        onClick={() => resolver('approve', { id: p.id })}
                        disabled={trabajando}
                        className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-semibold rounded-lg bg-emerald-600/20 hover:bg-emerald-600/35 text-emerald-200 border border-emerald-500/30 disabled:opacity-40 transition-colors"
                      >
                        {ocupado === `approve-${p.id}` ? (
                          <Loader2 className="w-3.5 h-3.5 animate-spin" />
                        ) : (
                          <Check className="w-3.5 h-3.5" />
                        )}
                        Aprobar
                      </button>
                      <button
                        onClick={() => resolver('reject', { id: p.id })}
                        disabled={trabajando}
                        className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-semibold rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 border border-slate-700 disabled:opacity-40 transition-colors"
                      >
                        {ocupado === `reject-${p.id}` ? (
                          <Loader2 className="w-3.5 h-3.5 animate-spin" />
                        ) : (
                          <X className="w-3.5 h-3.5" />
                        )}
                        Rechazar
                      </button>
                    </div>
                  </div>
                </div>
              );
            })
          )}
        </div>

        {/* Pie */}
        <div className="flex items-center justify-between gap-3 px-5 py-3 border-t border-slate-800 bg-slate-950/60">
          <p className="text-[10px] text-slate-500">
            La cola vive en el vault: sobrevive reinicios de la app.
          </p>
          <div className="flex items-center gap-2">
            <button
              onClick={() => resolver('reject', { todos: true })}
              disabled={!total || !!ocupado}
              className="px-3 py-1.5 text-[11px] rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 border border-slate-700 disabled:opacity-40 transition-colors"
            >
              {ocupado === 'todos-reject' ? <Loader2 className="w-3.5 h-3.5 animate-spin inline" /> : null} Rechazar todo
            </button>
            <button
              onClick={() => resolver('approve', { todos: true })}
              disabled={!total || !!ocupado}
              className="px-3 py-1.5 text-[11px] font-semibold rounded-lg bg-indigo-600/25 hover:bg-indigo-600/40 text-indigo-200 border border-indigo-500/30 disabled:opacity-40 transition-colors"
            >
              {ocupado === 'todos-approve' ? <Loader2 className="w-3.5 h-3.5 animate-spin inline" /> : null} Aprobar todo
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
