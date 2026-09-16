import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  X,
  Brain,
  Loader2,
  Sparkles,
  AlertTriangle,
  CheckCircle2,
  Network,
  Clock,
  ShieldAlert,
  Radio,
  Wrench,
  Check,
  Ban,
  Layers,
} from 'lucide-react';
import { apiUrl } from '../services/apiBase';
import { GatewayCerebro, type Aprobacion } from '../services/gatewayCerebro';

/** Un turno del cerebro: lo que el agente respondió y **qué contexto** se le mandó. */
export interface TurnoCerebro {
  pedido?: string;
  ok?: boolean;
  salida?: string;
  ms?: number;
  /** Visión, recuerdos dirigidos, foco y contadores (modo clásico) o modelo/tokens/herramientas (en vivo). */
  contexto?: string;
}

interface Props {
  isOpen: boolean;
  onClose: () => void;
  showToast: (text: string, type?: 'success' | 'info' | 'error') => void;
  /** Títulos del lienzo para sembrar un pedido (el padre ya los recorta). */
  sugerencias?: string[];
}

/** Clave donde el panel recuerda la sesión del cerebro: sin esto, cada turno arrancaría en blanco. */
const CLAVE_SESION = 'nodeflow_cerebro_sesion';

/**
 * «Pensar desde el lienzo»: la boca de la app hacia el cerebro residente.
 *
 * Dos caminos, elegidos con el interruptor:
 * - **En vivo** (Fase 4): el panel levanta el gateway propio (`hermes serve` en loopback) y le habla por
 *   WebSocket. El turno se ve **token por token**, se ven los usos de herramientas y las **aprobaciones se
 *   resuelven acá** — con «aplicar a todo» para no frenar los cambios grandes.
 * - **Clásico**: el subproceso de la Fase 1 (`hermes chat -c nf-cerebro`), que ya está probado y sirve de
 *   red si el gateway no arranca.
 *
 * En los dos, cada turno deja nota con fecha en `<bóveda>/cerebro/` y nada toca el lienzo sin aprobación.
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
  const turnoVivo = useRef(false);

  // ── Fase 4: gateway propio, stream y aprobaciones ──────────────────────────
  const [vivo, setVivo] = useState(true);
  const [estadoGateway, setEstadoGateway] = useState<'apagado' | 'arrancando' | 'listo' | 'error'>('apagado');
  const [enVivo, setEnVivo] = useState('');
  const [pensando, setPensando] = useState('');
  const [herramientas, setHerramientas] = useState<string[]>([]);
  const [modelo, setModelo] = useState('');
  const [uso, setUso] = useState<Record<string, unknown> | null>(null);
  const [aprobacion, setAprobacion] = useState<Aprobacion | null>(null);
  const [resueltas, setResueltas] = useState(0);
  const cliente = useRef<GatewayCerebro | null>(null);

  const traer = useCallback(async () => {
    try {
      const d = await (await fetch(apiUrl('/api/ai/delegar'))).json();
      // El turno en vivo no lo reporta este endpoint: si está corriendo acá, no se pisa el estado.
      if (!turnoVivo.current) setCorriendo(Boolean(d?.corriendo));
      setSesion(String(d?.cerebro?.sesion || ''));
      setNotas(Number(d?.cerebro?.notas || 0));
      const r = (d?.resultado || null) as TurnoCerebro | null;
      if (r && r.salida && !turnoVivo.current) {
        setTurno(r);
        if (!pedidoEnCurso.current && r.pedido) pedidoEnCurso.current = String(r.pedido);
      }
      if (!turnoVivo.current && corriendoAntes.current && !d?.corriendo && r?.salida) {
        showToast(`El cerebro respondió en ${((r.ms || 0) / 1000).toFixed(0)} s.`, 'success');
      }
      corriendoAntes.current = Boolean(d?.corriendo);
    } catch {
      /* el backend puede estar ocupado: el poller vuelve solo */
    }
  }, [showToast]);

  // El estado del gateway se consulta al abrir (no se levanta solo: nada corriendo de más).
  const mirarGateway = useCallback(async () => {
    try {
      const d = await (await fetch(apiUrl('/api/cerebro/gateway'))).json();
      const listo = Boolean(d?.gateway?.listo);
      setEstadoGateway(listo ? 'listo' : d?.gateway?.vivo ? 'arrancando' : 'apagado');
      return { listo, url: String(d?.url || '') };
    } catch {
      setEstadoGateway('error');
      return { listo: false, url: '' };
    }
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    let vivoTimer = true;
    void traer();
    void mirarGateway();
    const t = setInterval(() => {
      if (vivoTimer) void traer();
    }, 3000);
    return () => {
      vivoTimer = false;
      clearInterval(t);
    };
  }, [isOpen, traer, mirarGateway]);

  // Cronómetro: un turno real tarda decenas de segundos. Un spinner sin tiempo se lee como cuelgue.
  useEffect(() => {
    if (!corriendo) return;
    setSegundos(0);
    const t = setInterval(() => setSegundos((s) => s + 1), 1000);
    return () => clearInterval(t);
  }, [corriendo]);

  /** Levanta el gateway si hace falta y devuelve su URL. Arrancar importa el agente: puede tardar. */
  const asegurarGateway = useCallback(async (): Promise<string> => {
    let { listo, url } = await mirarGateway();
    if (listo && url) return url;
    setEstadoGateway('arrancando');
    await fetch(apiUrl('/api/cerebro/gateway/arrancar'), { method: 'POST' }).catch(() => undefined);
    for (let i = 0; i < 24; i++) {
      await new Promise((r) => setTimeout(r, 1500));
      const m = await mirarGateway();
      if (m.listo && m.url) return m.url;
    }
    setEstadoGateway('error');
    return '';
  }, [mirarGateway]);

  const pensar = useCallback(
    async (texto: string) => {
      const limpio = texto.trim();
      if (limpio.length < 4 || corriendo) return;
      pedidoEnCurso.current = limpio;
      setPedido('');
      setCorriendo(true);
      turnoVivo.current = false;
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

  /** Fase 4: el turno por el gateway — stream en vivo y aprobaciones en el panel. */
  const pensarEnVivo = useCallback(
    async (texto: string) => {
      const limpio = texto.trim();
      if (limpio.length < 4 || corriendo) return;
      pedidoEnCurso.current = limpio;
      setPedido('');
      setEnVivo('');
      setPensando('');
      setHerramientas([]);
      setUso(null);
      setAprobacion(null);
      cliente.current?.cerrar();
      const url = await asegurarGateway();
      if (!url) {
        showToast('El gateway no arrancó; paso al modo clásico.', 'info');
        void pensar(limpio);
        return;
      }
      const cli = new GatewayCerebro();
      cliente.current = cli;
      try {
        await cli.conectar(url, {
          onDelta: (t) => setEnVivo(t),
          onPensando: (t) => setPensando(t),
          onInfo: (i) => setModelo(String(i.model || i.provider || '')),
          onHerramienta: (nombre, fase) =>
            setHerramientas((hs) => {
              const etiqueta = fase === 'inicio' ? `⋯ ${nombre}` : `✓ ${nombre}`;
              return hs.some((h) => h.endsWith(nombre)) ? [...hs.filter((h) => !h.endsWith(nombre)), etiqueta] : [...hs, etiqueta];
            }),
          onAprobacion: (a) => setAprobacion(a),
          onError: (e) => showToast(e, 'error'),
          onListo: (final, u) => {
            turnoVivo.current = false;
            setUso(u);
            setPensando('');
            setCorriendo(false);
            const tokens = u ? ` · entrada ${u.input ?? '?'} / salida ${u.output ?? '?'} tok` : '';
            const usadas = herramientas.length ? ` · herramientas: ${herramientas.join(', ')}` : '';
            setTurno({
              ok: true,
              pedido: limpio,
              salida: final,
              contexto: `gateway en vivo · ${modelo || String(u?.model || 'modelo')}${tokens}${usadas}`,
            });
          },
        });
        turnoVivo.current = true;
        setCorriendo(true);
        const previa = window.localStorage.getItem(CLAVE_SESION);
        await cli.turno(limpio, previa);
        if (cli.sesionActual) {
          try {
            window.localStorage.setItem(CLAVE_SESION, cli.sesionActual);
          } catch {
            /* sin storage se pierde la continuidad, no el turno */
          }
        }
      } catch (e) {
        turnoVivo.current = false;
        setCorriendo(false);
        showToast(String(e).slice(0, 120), 'error');
      }
    },
    [corriendo, asegurarGateway, pensar, showToast, herramientas, modelo]
  );

  /** Responde una aprobación desde el panel. `all` la aplica a todo lo pendiente (cambios grandes). */
  const responderAprobacion = useCallback(
    (choice: string, all = false) => {
      if (!aprobacion) return;
      cliente.current?.responder(aprobacion.requestId, choice, all);
      setResueltas((n) => n + 1);
      setAprobacion(null);
      showToast(
        all ? `Aplicado «${choice}» a todo lo pendiente.` : `Aprobación resuelta: ${choice}.`,
        'success'
      );
    },
    [aprobacion, showToast]
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

  // Al cerrar el panel se suelta el WebSocket (el gateway se puede bajar aparte; la sesión queda guardada).
  useEffect(() => {
    if (isOpen) return;
    cliente.current?.cerrar();
    cliente.current = null;
  }, [isOpen]);

  if (!isOpen) return null;

  const seg = turno?.ms ? Math.round(turno.ms / 1000) : 0;
  const etiquetaGateway =
    estadoGateway === 'listo'
      ? 'en vivo'
      : estadoGateway === 'arrancando'
        ? 'arrancando…'
        : estadoGateway === 'error'
          ? 'sin gateway'
          : 'apagado';

  const OPCIONES: Array<{ id: string; texto: string; icono: React.ReactNode; clase: string; todo?: boolean }> = [
    { id: 'once', texto: 'Aprobar una vez', icono: <Check size={13} />, clase: 'bg-emerald-950/70 text-emerald-200 border-emerald-800/60 hover:bg-emerald-900/80' },
    { id: 'session', texto: 'Aprobar esta sesión', icono: <Check size={13} />, clase: 'bg-slate-900/70 text-slate-200 border-slate-700 hover:bg-slate-800' },
    { id: 'always', texto: 'Aprobar siempre', icono: <Layers size={13} />, clase: 'bg-slate-900/70 text-slate-200 border-slate-700 hover:bg-slate-800' },
    { id: 'deny', texto: 'Rechazar', icono: <Ban size={13} />, clase: 'bg-rose-950/60 text-rose-200 border-rose-800/60 hover:bg-rose-900/70' },
  ];

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
            <span
              className={`text-[9px] font-mono px-1.5 py-0.5 rounded border ${
                estadoGateway === 'listo'
                  ? 'text-emerald-300 bg-emerald-900/40 border-emerald-700/50'
                  : 'text-slate-400 bg-slate-900/70 border-slate-700'
              }`}
              title="El gateway propio (`hermes serve`) es lo que trae el stream en vivo y las aprobaciones"
            >
              {vivo ? etiquetaGateway : 'modo clásico'}
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
                · {vivo ? 'se ve en vivo: texto, herramientas y aprobaciones' : 'una sola respuesta al final'}
              </span>
            </div>
            <div className="flex items-start gap-2">
              <textarea
                value={pedido}
                onChange={(e) => setPedido(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault();
                    void (vivo ? pensarEnVivo(pedido) : pensar(pedido));
                  }
                }}
                rows={2}
                placeholder="Ej: ¿por dónde sigo con el cerebro local, mirando lo que ya hay en el lienzo?"
                className="flex-1 px-3 py-1.5 rounded-xl bg-slate-900/70 border border-slate-700 text-xs text-slate-100 placeholder:text-slate-500 focus:outline-none focus:border-cyan-700 resize-y"
              />
              <div className="flex flex-col gap-1.5 shrink-0">
                <button
                  type="button"
                  onClick={() => void (vivo ? pensarEnVivo(pedido) : pensar(pedido))}
                  disabled={corriendo || pedido.trim().length < 4}
                  className="flex items-center gap-2 px-3 py-1.5 rounded-xl text-xs font-medium bg-cyan-950/70 text-cyan-200 hover:bg-cyan-900/80 border border-cyan-800/60 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
                  title={corriendo ? 'El cerebro está en un turno' : 'Corre un turno (Enter)'}
                >
                  {corriendo ? <Loader2 size={13} className="animate-spin" /> : <Sparkles size={13} />}
                  {corriendo ? `Pensando… ${segundos}s` : 'Pensar'}
                </button>
                <button
                  type="button"
                  onClick={() => setVivo((v) => !v)}
                  disabled={corriendo}
                  className="flex items-center gap-1.5 px-2 py-1 rounded-lg text-[10px] font-mono text-slate-300 bg-slate-900/70 border border-slate-700 hover:border-cyan-700 hover:text-cyan-200 disabled:opacity-40 cursor-pointer"
                  title={vivo ? 'Pasar al modo clásico (subproceso, una sola respuesta)' : 'Volver al modo en vivo (gateway)'}
                >
                  <Radio size={11} /> {vivo ? 'en vivo' : 'clásico'}
                </button>
              </div>
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

          {/* ── El turno en vivo (Fase 4) ─────────────────────────────────── */}
          {vivo && (corriendo || enVivo) && (
            <div className="rounded-xl border border-cyan-900/50 bg-cyan-950/10 p-3 space-y-2">
              <div className="flex items-center gap-2">
                <Radio size={13} className="text-cyan-300 shrink-0" />
                <span className="text-xs text-cyan-100 font-medium">Turno en vivo</span>
                {modelo && <span className="text-[10px] font-mono text-slate-400">{modelo}</span>}
                {pensando && (
                  <span className="text-[11px] text-slate-400 italic truncate">· {pensando}</span>
                )}
                <span className="ml-auto flex items-center gap-1 text-[11px] text-slate-400">
                  <Clock size={11} /> {segundos}s
                </span>
              </div>
              {herramientas.length > 0 && (
                <div className="flex flex-wrap gap-1.5">
                  {herramientas.map((h) => (
                    <span
                      key={h}
                      className="flex items-center gap-1 px-1.5 py-0.5 rounded border border-slate-700 bg-slate-900/70 text-[10px] font-mono text-slate-300"
                    >
                      <Wrench size={9} /> {h}
                    </span>
                  ))}
                </div>
              )}
              <p
                className="text-xs text-slate-100 whitespace-pre-wrap leading-relaxed font-mono"
                id="cerebro-en-vivo"
              >
                {enVivo || '…'}
              </p>
            </div>
          )}

          {/* ── Aprobaciones: acá, en el panel, con «aplicar a todo» ─────── */}
          {aprobacion && (
            <div
              className="rounded-xl border border-amber-600/50 bg-amber-950/20 p-3 space-y-2"
              id="cerebro-aprobacion"
            >
              <div className="flex items-center gap-2">
                <ShieldAlert size={14} className="text-amber-300 shrink-0" />
                <span className="text-xs text-amber-100 font-medium">
                  {aprobacion.metodo === 'approval' ? 'El cerebro pide permiso' : 'El cerebro pregunta'}
                </span>
                {resueltas > 0 && (
                  <span className="ml-auto text-[10px] font-mono text-slate-400">
                    {resueltas} resuelta(s) en este turno
                  </span>
                )}
              </div>
              {aprobacion.descripcion && (
                <p className="text-xs text-slate-200">{aprobacion.descripcion}</p>
              )}
              {aprobacion.comando && (
                <pre className="text-[11px] text-amber-100 bg-slate-950/60 border border-amber-900/40 rounded-lg p-2 overflow-x-auto">
                  {aprobacion.comando}
                </pre>
              )}
              <div className="flex flex-wrap items-center gap-2">
                {OPCIONES.filter((o) => aprobacion.opciones.includes(o.id) || o.id === 'deny').map((o) => (
                  <button
                    key={o.id}
                    type="button"
                    onClick={() => responderAprobacion(o.id)}
                    className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-xs font-medium border cursor-pointer ${o.clase}`}
                    title={o.texto}
                  >
                    {o.icono} {o.texto}
                  </button>
                ))}
                <button
                  type="button"
                  onClick={() => responderAprobacion('session', true)}
                  className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-xs font-medium border border-amber-600/60 bg-amber-950/60 text-amber-100 hover:bg-amber-900/60 cursor-pointer"
                  title="Aplica esta decisión a todo lo que quede pendiente en el turno (no frena los cambios grandes)"
                >
                  <Layers size={13} /> Aplicar a todo lo pendiente
                </button>
              </div>
            </div>
          )}

          {/* El turno clásico: spinner (no se ve nada hasta el final) */}
          {!vivo && corriendo && (
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
                <span className="text-xs text-slate-100 font-medium">
                  Turno {turno.ok ? 'completo' : 'fallido'}
                </span>
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
                    title="Qué contexto se le mandó / con qué corrió el turno"
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
            proyecto crece en la bóveda, no en el chat. Nada toca el lienzo sin tu aprobación: las
            aprobaciones del turno se resuelven acá, con «aplicar a todo» cuando son varias.
          </p>
        </div>
      </div>
    </div>
  );
};

export default CerebroPanel;
