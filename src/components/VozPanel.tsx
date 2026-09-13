import React, { useCallback, useEffect, useRef, useState } from 'react';
import { X, Mic, Square, Loader2, Sparkles, Check, AlertTriangle, Wand2, Target, Layers, MessageSquarePlus, Link2, Quote, Gauge } from 'lucide-react';
import { SpeechmaticsRt, EstadoVoz } from '../services/speechmaticsRt';
import { getVozEstado, getVozJwt, pedirPlanVoz, describirComando, VozEstado, PlanVoz, VozComando } from '../services/vozService';

interface VozPanelProps {
  isOpen: boolean;
  onClose: () => void;
  /** Aplica el plan aprobado. Devuelve cuántos nodos creó y a cuántos afectó. */
  onAplicar: (plan: PlanVoz) => Promise<{ creados: number; afectados: number } | null>;
  tituloNodo: (id: string) => string;
}

const ICONO: Record<VozComando['accion'], React.ReactNode> = {
  crear: <MessageSquarePlus size={12} />,
  enlazar: <Link2 size={12} />,
  enfocar: <Target size={12} />,
  condensar: <Layers size={12} />,
  criticar: <Quote size={12} />,
};

const EJEMPLOS = [
  'Dictá ideas nuevas: «el orquestador de voz se integra con NodeFlow y con el mapa conceptual por nodos»',
  'O comandá: «limpiá el lienzo y dejá sólo lo que se conecta con el orquestador de voz»',
];

/**
 * Panel de Voz (Speechmatics). Hablás, la transcripción aparece en vivo y al cortar el motor
 * propone un PLAN de operaciones sobre el lienzo — que se aprueba antes de aplicarse.
 */
export const VozPanel: React.FC<VozPanelProps> = ({ isOpen, onClose, onAplicar, tituloNodo }) => {
  const [servicio, setServicio] = useState<VozEstado | null>(null);
  const [estado, setEstado] = useState<EstadoVoz>('inactivo');
  const [detalleEstado, setDetalleEstado] = useState('');
  const [parcial, setParcial] = useState('');
  const [texto, setTexto] = useState('');
  const [plan, setPlan] = useState<PlanVoz | null>(null);
  const [metricas, setMetricas] = useState<{ asrSeg: number; ms: number; modelo: string; costo: number; cache: string } | null>(null);
  const [error, setError] = useState('');
  const [pensando, setPensando] = useState(false);
  const [aplicando, setAplicando] = useState(false);
  const [resultado, setResultado] = useState('');
  const rtRef = useRef<SpeechmaticsRt | null>(null);
  const inicioRef = useRef(0);

  const consultarEstado = useCallback(async () => {
    try {
      setServicio(await getVozEstado());
    } catch (e: any) {
      setError(e?.message || 'No pude consultar el servicio de voz.');
    }
  }, []);

  useEffect(() => {
    if (isOpen) void consultarEstado();
  }, [isOpen, consultarEstado]);

  // Al cerrar el panel, cortamos cualquier captura en curso: no dejamos el micrófono abierto.
  useEffect(() => {
    if (!isOpen && rtRef.current) {
      void rtRef.current.stop();
      rtRef.current = null;
      setEstado('inactivo');
    }
  }, [isOpen]);

  const empezar = async () => {
    setError('');
    setResultado('');
    setPlan(null);
    setMetricas(null);
    setTexto('');
    setParcial('');
    try {
      const { jwt, url, modelo, idioma } = await getVozJwt();
      const rt = new SpeechmaticsRt(
        { url, jwt, idioma, modelo },
        {
          onEstado: (e, d) => {
            setEstado(e);
            setDetalleEstado(d || '');
          },
          onParcial: (t) => setParcial(t),
          onFinal: (t) => setTexto(t),
          onError: (m) => setError(m),
        }
      );
      rtRef.current = rt;
      inicioRef.current = performance.now();
      await rt.start();
    } catch (e: any) {
      setError(e?.message || 'No pude empezar a escuchar.');
      setEstado('error');
    }
  };

  const cortar = async () => {
    const rt = rtRef.current;
    if (!rt) return;
    const asrSeg = Math.round((performance.now() - inicioRef.current) / 100) / 10;
    const dictado = (await rt.stop()).trim();
    rtRef.current = null;
    setParcial('');
    setTexto(dictado);
    if (!dictado) {
      setError('No se escuchó nada. Probá de nuevo hablando más cerca del micrófono.');
      setEstado('inactivo');
      return;
    }
    setPensando(true);
    try {
      const { plan: p, modelo, uso } = await pedirPlanVoz(dictado);
      setPlan(p);
      setMetricas({
        asrSeg,
        ms: Math.round(uso?.ms ?? 0),
        modelo: uso?.modelo || modelo || 'motor',
        costo: uso?.costo_usd ?? 0,
        cache: uso?.cache ?? 'miss',
      });
    } catch (e: any) {
      setError(e?.message || 'El motor no pudo interpretar el dictado.');
    } finally {
      setPensando(false);
    }
  };

  const aplicar = async () => {
    if (!plan) return;
    setAplicando(true);
    try {
      const r = await onAplicar(plan);
      if (r) setResultado(`Listo: ${r.creados} nodo(s) creado(s), ${r.afectados} afectado(s).`);
      setPlan(null);
    } finally {
      setAplicando(false);
    }
  };

  if (!isOpen) return null;

  const escuchando = estado === 'escuchando' || estado === 'conectando' || estado === 'cerrando';
  const colorEstado = estado === 'escuchando' ? 'bg-emerald-400' : estado === 'error' ? 'bg-rose-400' : estado === 'conectando' || estado === 'cerrando' ? 'bg-amber-400' : 'bg-slate-500';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm" id="voz-panel">
      <div className="relative w-full max-w-2xl max-h-[88vh] overflow-hidden flex flex-col bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl">
        {/* Encabezado */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400">
              <Mic size={17} />
            </div>
            <div>
              <div className="text-sm font-semibold text-slate-200 flex items-center gap-2">
                Voz
                <span className="text-[10px] font-mono px-1.5 py-0.5 rounded border border-slate-700 text-slate-400">
                  {servicio?.proveedor || 'Speechmatics'} · {servicio?.modelo || 'enhanced'}
                </span>
                <span className={`w-2 h-2 rounded-full ${colorEstado} ${estado === 'escuchando' ? 'animate-pulse' : ''}`} />
              </div>
              <div className="text-[11px] text-slate-400">
                {estado === 'escuchando'
                  ? 'Escuchando… hablá normal'
                  : estado === 'conectando'
                    ? 'Conectando con Speechmatics…'
                    : estado === 'cerrando'
                      ? 'Cerrando el dictado…'
                      : estado === 'error'
                        ? `Error: ${detalleEstado || 'ver abajo'}`
                        : 'Hablá y el lienzo se opera solo (vos aprobás)'}
              </div>
            </div>
          </div>
          <button type="button" onClick={onClose} className="p-2 text-slate-400 hover:text-slate-200 rounded-lg hover:bg-slate-800 cursor-pointer">
            <X size={16} />
          </button>
        </div>

        {/* Cuerpo */}
        <div className="p-5 overflow-y-auto space-y-4 text-sm flex-1">
          {servicio && !servicio.configurada && (
            <div className="flex gap-2.5 items-start bg-amber-950/40 border border-amber-700/50 rounded-xl p-3.5 text-xs text-amber-100">
              <AlertTriangle size={15} className="shrink-0 mt-0.5 text-amber-400" />
              <div>
                <div className="font-semibold mb-0.5">Falta la clave de Speechmatics</div>
                <div className="text-amber-200/90">{servicio.pista}</div>
              </div>
            </div>
          )}

          {/* Botón de escucha */}
          <div className="flex items-center gap-3">
            <button
              type="button"
              id="btn-voz-escuchar"
              onClick={escuchando ? cortar : empezar}
              disabled={pensando || aplicando || (servicio ? !servicio.configurada : false)}
              className={`flex items-center gap-2 px-4 py-2.5 rounded-xl text-sm font-semibold border transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed ${
                escuchando
                  ? 'bg-rose-600/90 hover:bg-rose-500 text-white border-rose-400/60'
                  : 'bg-cyan-600/90 hover:bg-cyan-500 text-white border-cyan-400/60'
              }`}
            >
              {escuchando ? <Square size={15} /> : pensando ? <Loader2 size={15} className="animate-spin" /> : <Mic size={15} />}
              {escuchando ? 'Cortar y armar el plan' : pensando ? 'Interpretando…' : 'Escuchar'}
            </button>
            {servicio && !servicio.configurada && (
              <button type="button" onClick={consultarEstado} className="text-xs text-slate-400 hover:text-slate-200 underline cursor-pointer">
                Ya la puse, reintentar
              </button>
            )}
            <span className="text-[11px] text-slate-500">{servicio?.codec} · latencia objetivo &lt; 1 s</span>
          </div>

          {/* Transcripción viva */}
          <div className="bg-slate-900/70 border border-slate-700 rounded-xl p-3.5 min-h-[110px] max-h-[200px] overflow-y-auto">
            {!texto && !parcial && (
              <div className="space-y-1.5">
                {EJEMPLOS.map((t) => (
                  <div key={t} className="text-[11px] text-slate-500 italic">· {t}</div>
                ))}
              </div>
            )}
            {texto && <p className="text-xs text-slate-200 leading-relaxed">{texto}</p>}
            {parcial && <p className="text-xs text-slate-400 italic leading-relaxed">{parcial}…</p>}
          </div>

          {error && (
            <div className="flex gap-2 items-start text-xs text-rose-200 bg-rose-950/40 border border-rose-800/60 rounded-xl p-3">
              <AlertTriangle size={13} className="shrink-0 mt-0.5 text-rose-400" />
              <span>{error}</span>
            </div>
          )}

          {/* Plan propuesto */}
          {plan && (
            <div className="bg-slate-900/70 border border-violet-600/50 rounded-xl p-4 space-y-3" id="voz-plan">
              <div className="flex items-center gap-2 text-[10px] uppercase tracking-widest text-violet-300 font-bold">
                <Sparkles size={12} /> Plan propuesto
                <span className="px-1.5 py-0.5 rounded border border-violet-700/60 text-violet-200 font-mono normal-case tracking-normal">
                  {plan.intencion === 'capturar' ? 'agregar al lienzo' : 'operar sobre el lienzo'}
                </span>
              </div>
              <p className="text-sm text-slate-200 leading-relaxed">{plan.respuesta}</p>
              {plan.motivo && <p className="text-[11px] text-slate-400 italic">{plan.motivo}</p>}

              <div className="space-y-1.5">
                {plan.comandos.length === 0 && (
                  <div className="text-xs text-slate-400">No encontré nada aplicable en el lienzo para eso.</div>
                )}
                {plan.comandos.map((c, i) => (
                  <div key={i} className="flex items-center gap-2 text-xs text-slate-200 bg-slate-800/60 rounded-lg px-2.5 py-1.5 border border-slate-700">
                    <span className="text-violet-300">{ICONO[c.accion]}</span>
                    <span>{describirComando(c, tituloNodo)}</span>
                    {c.criterio && <span className="text-slate-500 truncate">· {c.criterio}</span>}
                  </div>
                ))}
              </div>

              {!!plan.descartados && (
                <div className="text-[11px] text-amber-200/90">
                  Descarté {plan.descartados} operación(es) que no cerraban contra el lienzo.
                  {plan.motivo_descarte?.length ? ` (${plan.motivo_descarte.slice(0, 2).join('; ')})` : ''}
                </div>
              )}

              <div className="flex items-center gap-2 pt-1">
                <button
                  type="button"
                  id="btn-voz-aplicar"
                  onClick={aplicar}
                  disabled={aplicando || plan.comandos.length === 0}
                  className="flex items-center gap-1.5 px-3 py-2 bg-violet-600 hover:bg-violet-500 text-white rounded-xl text-xs font-semibold border border-violet-400/60 transition-colors cursor-pointer disabled:opacity-50"
                >
                  {aplicando ? <Loader2 size={13} className="animate-spin" /> : <Check size={13} />}
                  Aplicar al lienzo
                </button>
                <button
                  type="button"
                  onClick={() => setPlan(null)}
                  className="px-3 py-2 text-xs text-slate-300 hover:text-white bg-slate-800 hover:bg-slate-700 rounded-xl border border-slate-600 transition-colors cursor-pointer"
                >
                  Descartar
                </button>
                <span className="text-[11px] text-slate-500">Ctrl+Z lo deshace si no te gusta.</span>
              </div>
            </div>
          )}

          {resultado && (
            <div className="flex items-center gap-2 text-xs text-emerald-200 bg-emerald-950/40 border border-emerald-800/60 rounded-xl p-3">
              <Wand2 size={13} className="text-emerald-400" /> {resultado}
            </div>
          )}
        </div>

        {/* Pie: lo medido en esta corrida */}
        <div className="flex items-center justify-between gap-3 px-5 py-3 border-t border-slate-800 text-[11px] text-slate-400">
          <span className="flex items-center gap-1.5">
            <Gauge size={12} className="text-cyan-400" />
            {metricas
              ? `dictado ${metricas.asrSeg} s · plan ${metricas.ms} ms · ${metricas.modelo} · ${metricas.cache === 'hit' ? 'caché HIT' : `US$${metricas.costo.toFixed(6)}`}`
              : 'El costo y la latencia de cada dictado se miden acá'}
          </span>
          <span>Speechmatics Realtime + el motor elegido en la app</span>
        </div>
      </div>
    </div>
  );
};
