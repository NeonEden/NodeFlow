import React, { useCallback, useEffect, useRef, useState } from 'react';
import { AlertTriangle, Bot, CheckCircle2, Loader2, Play, Wrench, X, XCircle } from 'lucide-react';
import { apiUrl } from '../services/apiBase';
import { useIdioma } from '../i18n/useIdioma';

/**
 * El panel del **agente propio** (etapas 1+3 del plan «sin Hermes»).
 *
 * Es la cara del bucle que corre en el backend de NodeFlow con herramientas de repo: leer con rango,
 * buscar, firmas, y correr los comandos de la lista blanca. **No escribe**: mira y verifica, propone.
 * El panel muestra cada paso (herramienta, argumentos, ms, ok) porque la diferencia entre «el agente
 * dijo algo» y «el agente hizo algo verificable» es justamente esa lista.
 */

interface Paso {
  herramienta: string;
  argumentos: Record<string, unknown>;
  ok: boolean;
  ms: number;
  salida: string;
}

interface Resultado {
  pedido: string;
  ok: boolean;
  respuesta: string;
  pasos: Paso[];
  herramientas_usadas: number;
  ms: number;
  motor: string;
  modelo: string;
  tokens: number;
  contexto?: string;
}

interface Edicion {
  ruta: string;
  buscar: string;
  reemplazar: string;
  todos: boolean;
}

interface Gate {
  comando: string;
  ok: boolean | null;
  ms?: number;
  salida?: string;
}

interface Propuesta {
  motivo: string;
  estado: 'pendiente' | 'aplicado' | 'rechazado' | 'revertido' | 'error';
  ediciones: Edicion[];
  archivos: { ruta: string; ediciones: number }[];
  aplicado?: { archivos: string[]; ediciones: number; mas: number; menos: number };
  verificacion?: Gate[];
  verificado_ok?: boolean;
  error?: string;
}

interface RespuestaTurno {
  success: boolean;
  corriendo?: boolean;
  error?: string;
}

export const AgentePanel: React.FC<{ isOpen: boolean; onClose: () => void }> = ({ isOpen, onClose }) => {
  const { t } = useIdioma();
  const [pedido, setPedido] = useState('');
  const [corriendo, setCorriendo] = useState(false);
  const [segundos, setSegundos] = useState(0);
  const [res, setRes] = useState<Resultado | null>(null);
  const [aviso, setAviso] = useState('');
  const [verSalidas, setVerSalidas] = useState(false);
  const [propuesta, setPropuesta] = useState<Propuesta | null>(null);
  const [gatesAbiertos, setGatesAbiertos] = useState('');
  const [accion, setAccion] = useState('');
  const corriendoRef = useRef(false);

  // Contador visible: un turno tarda decenas de segundos, y sin tiempo se lee como cuelgue.
  useEffect(() => {
    if (!corriendo) return;
    const id = window.setInterval(() => setSegundos((s) => s + 1), 1000);
    return () => window.clearInterval(id);
  }, [corriendo]);

  // La cola del parche (etapa 4): se consulta junto con el turno, porque se propone EN el turno.
  const consultarParche = useCallback(async () => {
    try {
      const d = await (await fetch(apiUrl('/api/agente/parche'))).json();
      setPropuesta((d?.propuesta as Propuesta) ?? null);
    } catch {
      /* sin propuesta: no es un error */
    }
  }, []);

  const accionParche = async (cual: 'aprobar' | 'rechazar' | 'revertir') => {
    setAccion(cual);
    try {
      const r = await fetch(apiUrl(`/api/agente/parche/${cual}`), { method: 'POST' });
      const d = await r.json();
      if (!r.ok || !d.success) setAviso(d?.error || `no pude ${cual}`);
      await consultarParche();
    } finally {
      setAccion('');
    }
  };

  const consultar = useCallback(async () => {
    try {
      const d = await (await fetch(apiUrl('/api/agente/estado'))).json();
      if (d?.resultado) setRes(d.resultado as Resultado);
      return !!d?.corriendo;
    } catch {
      return false;
    }
  }, []);

  // El turno se sigue por consulta: la petición de arranque vuelve enseguida, nunca se cuelga.
  useEffect(() => {
    if (!isOpen) return;
    void consultar();
    void consultarParche();
  }, [isOpen, consultar, consultarParche]);

  // Mientras hay un parche aplicado sin verificación, se consulta: los gates tardan.
  useEffect(() => {
    if (!isOpen || propuesta?.estado !== 'aplicado' || propuesta?.verificacion) return;
    const id = window.setInterval(() => void consultarParche(), 4000);
    return () => window.clearInterval(id);
  }, [isOpen, propuesta, consultarParche]);

  useEffect(() => {
    if (!corriendo) {
      corriendoRef.current = false;
      return;
    }
    corriendoRef.current = true;
    const id = window.setInterval(async () => {
      const sigue = await consultar();
      if (!sigue && corriendoRef.current) setCorriendo(false);
    }, 5000);
    return () => window.clearInterval(id);
  }, [corriendo, consultar]);

  const arrancar = async () => {
    setAviso('');
    if (pedido.trim().length < 4) {
      setAviso(t('agente.faltaPedido'));
      return;
    }
    setSegundos(0);
    try {
      const r = await fetch(apiUrl('/api/agente/turno'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pedido }),
      });
      const d = (await r.json()) as RespuestaTurno;
      if (!r.ok || !d.success) {
        setAviso(d?.error || t('agente.sinRepo'));
        return;
      }
      setCorriendo(true);
    } catch (e) {
      setAviso(String(e));
    }
  };

  if (!isOpen) return null;

  const pasos = res?.pasos ?? [];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm" id="agente-panel">
      <div className="relative w-full max-w-3xl max-h-[88vh] overflow-hidden flex flex-col bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl">
        {/* Encabezado */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-violet-500/10 border border-violet-500/30 flex items-center justify-center text-violet-300">
              <Bot size={17} />
            </div>
            <div>
              <div className="text-sm font-semibold text-slate-200 flex items-center gap-2">
                {t('agente.titulo')}
                <span className="text-[10px] font-mono px-1.5 py-0.5 rounded border border-slate-700 text-slate-400">
                  {res ? `${res.motor} · ${res.modelo}` : t('agente.sinHermes')}
                </span>
              </div>
              <div className="text-[11px] text-slate-400">{t('agente.ayuda')}</div>
            </div>
          </div>
          <button type="button" onClick={onClose} className="p-2 text-slate-400 hover:text-slate-200 rounded-lg hover:bg-slate-800 cursor-pointer">
            <X size={16} />
          </button>
        </div>

        {/* Cuerpo */}
        <div className="p-5 overflow-y-auto space-y-4 text-sm flex-1">
          <div className="flex items-start gap-2">
            <textarea
              value={pedido}
              onChange={(e) => setPedido(e.target.value)}
              rows={2}
              placeholder={t('agente.pedido')}
              className="flex-1 bg-slate-950/60 border border-slate-700 rounded-xl px-3 py-2 text-xs text-slate-200 placeholder:text-slate-500 focus:outline-none focus:border-violet-600 resize-none"
            />
            <button
              type="button"
              id="btn-agente-correr"
              onClick={arrancar}
              disabled={corriendo}
              className="flex items-center gap-2 px-4 py-2.5 rounded-xl text-sm font-semibold border transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed bg-violet-600/90 hover:bg-violet-500 text-white border-violet-400/60"
            >
              {corriendo ? <Loader2 size={15} className="animate-spin" /> : <Play size={15} />}
              {corriendo ? `${t('agente.corriendo')} · ${segundos}s` : t('agente.correr')}
            </button>
          </div>

          {aviso && (
            <div className="flex gap-2 items-start text-xs bg-slate-800 border border-slate-700 rounded-xl p-3">
              <AlertTriangle size={13} className="shrink-0 mt-0.5 text-amber-400" />
              <span className="text-slate-200">{aviso}</span>
            </div>
          )}

          {res && (
            <>
              <div className="flex items-center justify-between gap-3 text-[11px] text-slate-400">
                <span className="flex items-center gap-2">
                  <Wrench size={12} className="text-violet-300" />
                  {t('agente.pasos')}:{' '}
                  <strong className="text-slate-200 font-mono">{res.herramientas_usadas}</strong>
                  <span className="text-slate-500">
                    · {Math.round(res.ms / 1000)}s · {res.tokens} tok
                  </span>
                </span>
                <span className="text-slate-500">{t('agente.noEscribe')}</span>
              </div>

              {pasos.length === 0 ? (
                <p className="text-[11px] text-slate-500 italic">{t('agente.sinHerramientas')}</p>
              ) : (
                <ol className="space-y-1.5">
                  {pasos.map((p, i) => (
                    <li key={`${p.herramienta}-${i}`} className="bg-slate-950/50 border border-slate-800 rounded-xl px-3 py-2">
                      <div className="flex items-center gap-2 text-[11px]">
                        {p.ok ? (
                          <CheckCircle2 size={12} className="text-emerald-400 shrink-0" />
                        ) : (
                          <XCircle size={12} className="text-rose-400 shrink-0" />
                        )}
                        <span className="font-mono text-slate-200">{p.herramienta}</span>
                        <span className="font-mono text-slate-500 truncate">
                          {JSON.stringify(p.argumentos).slice(0, 70)}
                        </span>
                        <span className="ml-auto font-mono text-slate-500 shrink-0">{p.ms} ms</span>
                      </div>
                      {verSalidas && p.salida && (
                        <pre className="mt-1.5 text-[10px] text-slate-400 whitespace-pre-wrap max-h-40 overflow-y-auto">
                          {p.salida.slice(0, 900)}
                        </pre>
                      )}
                    </li>
                  ))}
                </ol>
              )}

              <button
                type="button"
                onClick={() => setVerSalidas((v) => !v)}
                className="text-[11px] text-slate-400 hover:text-slate-200 underline cursor-pointer"
              >
                {verSalidas ? t('agente.ocultarSalidas') : t('agente.verSalidas')}
              </button>

              <div className="bg-slate-950/60 border border-slate-700 rounded-xl p-4" id="agente-respuesta">
                <div className="text-[10px] uppercase tracking-wide text-slate-500 mb-1">{t('agente.respuesta')}</div>
                <p className="text-xs text-slate-200 whitespace-pre-wrap leading-relaxed">{res.respuesta}</p>
                {res.contexto && (
                  <p className="mt-2 text-[10px] text-slate-500 font-mono">{res.contexto}</p>
                )}
              </div>
            </>
          )}

          {!res && !corriendo && (
            <p className="text-[11px] text-slate-500 italic">{t('agente.ejemplo')}</p>
          )}

          {/* Etapa 4: el parche propuesto. Se escribe sólo al aprobarlo, y después corren los gates. */}
          <div className="border-t border-slate-800 pt-4 space-y-2">
            <div className="text-[10px] uppercase tracking-wide text-slate-500">{t('parche.titulo')}</div>
            {!propuesta ? (
              <p className="text-[11px] text-slate-500 italic">{t('parche.sinPendiente')}</p>
            ) : (
              <div className="bg-slate-950/50 border border-slate-800 rounded-xl p-3 space-y-2">
                <div className="text-[11px] text-slate-300">
                  <span className="text-slate-500">{t('parche.motivo')}: </span>
                  {propuesta.motivo}
                </div>
                <div className="text-[10px] font-mono text-slate-400">
                  {propuesta.archivos?.map((a) => `${a.ruta} (${a.ediciones} ed.)`).join(' · ')}
                </div>

                {/* Lo que el humano revisa: el antes y el después, literal */}
                <div className="space-y-1.5 max-h-56 overflow-y-auto">
                  {propuesta.ediciones?.slice(0, 6).map((e, i) => (
                    <div key={`${e.ruta}-${i}`} className="text-[10px] font-mono leading-relaxed">
                      <div className="text-slate-500">{e.ruta}</div>
                      <div className="text-rose-300/90 whitespace-pre-wrap">- {e.buscar.slice(0, 160)}</div>
                      <div className="text-emerald-300/90 whitespace-pre-wrap">+ {e.reemplazar.slice(0, 160)}</div>
                    </div>
                  ))}
                  {(propuesta.ediciones?.length || 0) > 6 && (
                    <div className="text-[10px] text-slate-500">+ {(propuesta.ediciones?.length || 0) - 6} ediciones más…</div>
                  )}
                </div>

                <div className="flex items-center gap-2 flex-wrap">
                  {propuesta.estado === 'pendiente' && (
                    <>
                      <button
                        type="button"
                        id="btn-parche-aprobar"
                        onClick={() => accionParche('aprobar')}
                        disabled={!!accion}
                        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-semibold bg-emerald-600/90 hover:bg-emerald-500 text-white border border-emerald-400/60 cursor-pointer disabled:opacity-50"
                      >
                        {accion === 'aprobar' ? <Loader2 size={12} className="animate-spin" /> : <CheckCircle2 size={12} />}
                        {t('parche.aprobar')}
                      </button>
                      <button
                        type="button"
                        onClick={() => accionParche('rechazar')}
                        disabled={!!accion}
                        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] border border-slate-700 bg-slate-800 text-slate-200 hover:text-white cursor-pointer disabled:opacity-50"
                      >
                        <XCircle size={12} /> {t('parche.rechazar')}
                      </button>
                      <span className="text-[10px] text-slate-500">{t('parche.aviso')}</span>
                    </>
                  )}
                  {propuesta.estado === 'aplicado' && !propuesta.verificacion && (
                    <span className="flex items-center gap-1.5 text-[11px] text-amber-300">
                      <Loader2 size={12} className="animate-spin" /> {t('parche.verificando')}
                    </span>
                  )}
                  {propuesta.estado === 'aplicado' && propuesta.verificacion && (
                    <>
                      <span className={`flex items-center gap-1.5 text-[11px] ${propuesta.verificado_ok ? 'text-emerald-300' : 'text-rose-300'}`}>
                        {propuesta.verificado_ok ? <CheckCircle2 size={12} /> : <XCircle size={12} />}
                        {propuesta.verificado_ok ? t('parche.verde') : t('parche.rojo')}
                        {propuesta.aplicado ? ` · +${propuesta.aplicado.mas}/-${propuesta.aplicado.menos} líneas` : ''}
                      </span>
                      <button
                        type="button"
                        onClick={() => accionParche('revertir')}
                        disabled={!!accion}
                        className="px-3 py-1.5 rounded-lg text-[11px] border border-slate-700 bg-slate-800 text-slate-200 hover:text-white cursor-pointer disabled:opacity-50"
                      >
                        {t('parche.revertir')}
                      </button>
                    </>
                  )}
                  {propuesta.estado === 'rechazado' && <span className="text-[11px] text-slate-400">{t('parche.rechazado')}</span>}
                  {propuesta.estado === 'revertido' && <span className="text-[11px] text-slate-400">{t('parche.revertido')}</span>}
                  {propuesta.estado === 'error' && (
                    <span className="text-[11px] text-rose-300">
                      {t('parche.error')}: {propuesta.error}
                    </span>
                  )}
                </div>

                {/* Los gates, con su salida real: es la diferencia entre "creo que anda" y "anduvo" */}
                {propuesta.verificacion?.map((g, i) => (
                  <div key={`${g.comando}-${i}`} className="text-[10px] font-mono">
                    <button
                      type="button"
                      onClick={() => setGatesAbiertos(gatesAbiertos === g.comando ? '' : g.comando)}
                      className="flex items-center gap-2 text-left cursor-pointer"
                    >
                      {g.ok === null ? (
                        <Loader2 size={10} className="animate-spin text-amber-300" />
                      ) : g.ok ? (
                        <CheckCircle2 size={10} className="text-emerald-400" />
                      ) : (
                        <XCircle size={10} className="text-rose-400" />
                      )}
                      <span className="text-slate-300">{g.comando}</span>
                      {g.ms ? <span className="text-slate-500">· {Math.round((g.ms || 0) / 1000)}s</span> : null}
                      <span className="text-slate-600 underline">{t('parche.verSalida')}</span>
                    </button>
                    {gatesAbiertos === g.comando && g.salida && (
                      <pre className="mt-1 text-[10px] text-slate-400 whitespace-pre-wrap max-h-40 overflow-y-auto">
                        {g.salida.slice(0, 1200)}
                      </pre>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
