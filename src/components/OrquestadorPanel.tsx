import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Workflow,
  X,
  Loader2,
  Play,
  Copy,
  Check,
  Sparkles,
  AlertTriangle,
  Plus,
  RefreshCw,
  Cpu,
} from 'lucide-react';
import {
  listarExpertos,
  ejecutarExperto,
  Experto,
  TipoArtefacto,
  RespuestaExperto,
} from '../services/expertoService';
import { apiUrl } from '../services/apiBase';
import { triggerFileDownload } from '../utils/obsidianExport';

interface NodoOpcion {
  id: string;
  title: string;
}

interface OrquestadorPanelProps {
  isOpen: boolean;
  onClose: () => void;
  nodos: NodoOpcion[];
  nodoSeleccionado?: string;
  onPropuestasCreadas: () => void;
  showToast: (message: string, type?: 'success' | 'error' | 'info') => void;
}

const COLOR_TIPO: Record<string, string> = {
  prompt_visual: 'text-sky-300 border-sky-500/40 bg-sky-500/10',
  brief_documento: 'text-violet-300 border-violet-500/40 bg-violet-500/10',
  critica: 'text-amber-300 border-amber-500/40 bg-amber-500/10',
  spec_td: 'text-emerald-300 border-emerald-500/40 bg-emerald-500/10',
};

/** Resumen compacto del artefacto según su tipo (el texto completo va aparte, listo para pegar). */
const Resumen: React.FC<{ r: RespuestaExperto }> = ({ r }) => {
  const a = (r.artefacto || {}) as Record<string, unknown>;
  const linea = (k: string, v: unknown) =>
    v ? (
      <p className="text-[11px] text-slate-300">
        <span className="text-slate-500">{k}: </span>
        {String(v)}
      </p>
    ) : null;
  if (r.tipo === 'prompt_visual') {
    return (
      <div className="space-y-1">
        {linea('Sujeto', a.sujeto)}
        {linea('Ambiente', a.ambiente)}
        {linea('Estilo', a.estilo)}
        {linea('Iluminación', a.iluminacion)}
        {linea('Cámara', a.camara)}
        {linea('Aspect', a.aspect_ratio)}
        {linea('Negativo', a.negativo)}
      </div>
    );
  }
  if (r.tipo === 'brief_documento') {
    const secs = (a.secciones as { titulo?: string }[]) || [];
    return (
      <div className="space-y-1">
        {linea('Objetivo', a.objetivo)}
        {linea('Audiencia', a.audiencia)}
        {linea('Formato', a.formato)}
        {secs.length > 0 && (
          <p className="text-[11px] text-slate-400">
            Secciones: {secs.map((s) => s.titulo).filter(Boolean).join(' · ')}
          </p>
        )}
      </div>
    );
  }
  if (r.tipo === 'critica') {
    const l = (k: string) => ((a[k] as string[]) || []).map((x, i) => <li key={i}>{x}</li>);
    return (
      <div className="space-y-2">
        {linea('Veredicto', a.veredicto)}
        <div>
          <p className="text-[10px] uppercase text-emerald-400/80">Fortalezas</p>
          <ul className="text-[11px] text-slate-300 list-disc ml-4">{l('fortalezas')}</ul>
        </div>
        <div>
          <p className="text-[10px] uppercase text-rose-400/80">Debilidades</p>
          <ul className="text-[11px] text-slate-300 list-disc ml-4">{l('debilidades')}</ul>
        </div>
        <div>
          <p className="text-[10px] uppercase text-amber-400/80">Riesgos</p>
          <ul className="text-[11px] text-slate-300 list-disc ml-4">{l('riesgos')}</ul>
        </div>
      </div>
    );
  }
  return null;
};

export const OrquestadorPanel: React.FC<OrquestadorPanelProps> = ({
  isOpen,
  onClose,
  nodos,
  nodoSeleccionado,
  onPropuestasCreadas,
  showToast,
}) => {
  const [expertos, setExpertos] = useState<Experto[]>([]);
  const [tipos, setTipos] = useState<TipoArtefacto[]>([]);
  const [carpeta, setCarpeta] = useState('');
  const [experto, setExperto] = useState<string>('');
  const [nodo, setNodo] = useState<string>('');
  const [extra, setExtra] = useState('');
  const [corriendo, setCorriendo] = useState(false);
  const [segundos, setSegundos] = useState(0);
  const [res, setRes] = useState<RespuestaExperto | null>(null);
  const [copiado, setCopiado] = useState(false);
  const cronometro = useRef<number | null>(null);

  const cargar = useCallback(async () => {
    const r = await listarExpertos();
    if (!r) {
      showToast('No pude leer los expertos (¿la app está viva?)', 'error');
      return;
    }
    setExpertos(r.expertos || []);
    setTipos(r.tipos || []);
    setCarpeta(r.carpeta || '');
    if (!experto && r.expertos?.length) setExperto(r.expertos[0].nombre);
  }, [experto, showToast]);

  useEffect(() => {
    if (isOpen) void cargar();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen]);

  useEffect(() => {
    if (isOpen && nodoSeleccionado && !nodo) setNodo(nodoSeleccionado);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, nodoSeleccionado]);

  const activo = useMemo(() => expertos.find((e) => e.nombre === experto), [expertos, experto]);
  const tipoInfo = useMemo(
    () => tipos.find((t) => t.tipo === activo?.tipo_artefacto),
    [tipos, activo]
  );

  const ejecutar = async () => {
    if (!nodo.trim()) {
      showToast('Elegí un nodo del lienzo', 'error');
      return;
    }
    if (!experto) {
      showToast('Elegí un experto', 'error');
      return;
    }
    setCorriendo(true);
    setRes(null);
    setSegundos(0);
    cronometro.current = window.setInterval(() => setSegundos((s) => s + 1), 1000);
    const r = await ejecutarExperto(nodo.trim(), experto, extra);
    if (cronometro.current) window.clearInterval(cronometro.current);
    setCorriendo(false);
    if (!r) {
      showToast('No hubo respuesta del motor', 'error');
      return;
    }
    if (r.error) {
      showToast(r.error, 'error');
      return;
    }
    setRes(r);
    showToast(
      r.ok
        ? `Artefacto válido · ${r.proveedor} · ${(r.ms / 1000).toFixed(1)}s`
        : `Artefacto con ${r.problemas.length} problema(s) — revisá antes de usarlo`,
      r.ok ? 'success' : 'info'
    );
  };

  const copiar = async (texto: string) => {
    try {
      await navigator.clipboard.writeText(texto);
      setCopiado(true);
      window.setTimeout(() => setCopiado(false), 1800);
      showToast('Copiado al portapapeles', 'success');
    } catch {
      triggerFileDownload(texto, 'artefacto.txt', 'text/plain;charset=utf-8');
      showToast('Descargado como archivo', 'info');
    }
  };

  const crearNodo = async () => {
    if (!res) return;
    const titulo = `${experto}: ${res.nodo.titulo}`.slice(0, 70);
    try {
      const r = await fetch(apiUrl('/api/graph/node'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          title: titulo,
          description: res.texto.slice(0, 4000),
          category: 'ARTEFACTO',
          maturity: 2,
          parent: res.nodo.titulo,
          link_label: 'produce',
          motivo: `Artefacto ${res.tipo} generado por el experto «${experto}»`,
        }),
      });
      const d = await r.json().catch(() => null);
      if (d?.ok || d?.accion) {
        showToast('Propuesta creada — aprobala en «Cambios del agente»', 'success');
        onPropuestasCreadas();
      } else {
        showToast(d?.error || 'No pude crear la propuesta', 'error');
      }
    } catch {
      showToast('No pude crear la propuesta', 'error');
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-fade-in">
      <div
        id="orquestador-panel"
        className="w-full max-w-3xl max-h-[90vh] flex flex-col bg-slate-900 border border-slate-700/60 rounded-2xl shadow-2xl overflow-hidden"
      >
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800 bg-slate-950/60">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-violet-500 to-indigo-600 flex items-center justify-center">
              <Workflow className="w-5 h-5 text-white" />
            </div>
            <div>
              <h2 className="text-sm font-bold text-white">Orquestador</h2>
              <p className="text-[11px] text-slate-400">
                El concepto entra, el artefacto sale <span className="text-slate-300">validado</span> para
                su destino.
              </p>
            </div>
          </div>
          <div className="flex items-center gap-1">
            <button
              onClick={() => void cargar()}
              title="Recargar expertos (se editan en la bóveda)"
              className="p-2 text-slate-400 hover:text-white rounded-lg hover:bg-slate-800 transition-colors"
            >
              <RefreshCw className="w-4 h-4" />
            </button>
            <button
              onClick={onClose}
              className="p-2 text-slate-400 hover:text-white rounded-lg hover:bg-slate-800 transition-colors"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-3">
          {/* Expertos */}
          <div>
            <p className="text-[10px] uppercase tracking-wide text-slate-500 mb-1.5">
              Expertos ({expertos.length}) · se editan como notas en{' '}
              <span className="font-mono text-slate-600">expertos/</span>
            </p>
            <div className="grid grid-cols-1 gap-1.5">
              {expertos.map((e) => (
                <button
                  key={e.slug}
                  onClick={() => setExperto(e.nombre)}
                  className={`text-left rounded-xl border px-3 py-2 transition-colors ${
                    experto === e.nombre
                      ? 'border-violet-500/60 bg-violet-500/10'
                      : 'border-slate-800 bg-slate-950/40 hover:border-slate-700'
                  }`}
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-xs font-semibold text-slate-100">{e.nombre}</span>
                    <span
                      className={`text-[9px] px-1.5 py-0.5 rounded border uppercase ${
                        COLOR_TIPO[e.tipo_artefacto] || 'text-slate-400 border-slate-700'
                      }`}
                    >
                      {e.tipo_artefacto || 'sin tipo'}
                    </span>
                  </div>
                  <p className="text-[11px] text-slate-400 mt-0.5">{e.descripcion}</p>
                  <p className="text-[10px] text-slate-600 mt-0.5 font-mono">
                    {e.caracteres_system} car. de prompt
                    {e.proveedor ? ` · proveedor: ${e.proveedor}` : ''}
                    {e.valido ? '' : ' · TIPO INVÁLIDO'}
                  </p>
                </button>
              ))}
              {expertos.length === 0 && (
                <p className="text-[11px] text-slate-500 py-2">
                  No hay expertos. Se leen de <span className="font-mono">{carpeta}</span> — agregá un
                  .md con frontmatter <span className="font-mono">tipo_artefacto</span>.
                </p>
              )}
            </div>
          </div>

          {/* Nodo + extra */}
          <div className="grid grid-cols-1 gap-2">
            <div>
              <p className="text-[10px] uppercase tracking-wide text-slate-500 mb-1">
                Nodo del lienzo (el concepto)
              </p>
              <select
                value={nodos.some((n) => n.title === nodo) ? nodo : ''}
                onChange={(e) => setNodo(e.target.value)}
                className="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2 text-xs text-slate-100 focus:outline-none focus:border-violet-500"
              >
                <option value="">— elegí un nodo —</option>
                {nodos.map((n) => (
                  <option key={n.id} value={n.title}>
                    {n.title}
                  </option>
                ))}
              </select>
            </div>
            <textarea
              value={extra}
              onChange={(e) => setExtra(e.target.value)}
              rows={2}
              placeholder="Indicaciones extra para este artefacto (opcional): destino, tono, formato, qué evitar…"
              className="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2 text-xs text-slate-100 placeholder-slate-600 focus:outline-none focus:border-violet-500 resize-y"
            />
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={() => void ejecutar()}
              disabled={corriendo || !experto || !nodo}
              className="flex items-center gap-2 px-4 py-2 text-[11px] font-semibold rounded-lg bg-violet-600/25 hover:bg-violet-600/40 text-violet-100 border border-violet-500/40 disabled:opacity-40 transition-colors"
            >
              {corriendo ? <Loader2 className="w-4 h-4 animate-spin" /> : <Play className="w-4 h-4" />}
              {corriendo ? `Generando… ${segundos}s` : 'Ejecutar experto'}
            </button>
            {tipoInfo && (
              <p className="text-[10px] text-slate-500">
                produce <span className="text-slate-300">{tipoInfo.tipo}</span> para{' '}
                <span className="text-slate-300">{tipoInfo.destino}</span>
              </p>
            )}
          </div>

          {/* Resultado */}
          {res && (
            <div
              className={`rounded-xl border p-3 space-y-2 ${
                res.ok ? 'border-emerald-500/40 bg-emerald-500/5' : 'border-amber-500/40 bg-amber-500/5'
              }`}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-center gap-2">
                  {res.ok ? (
                    <Check className="w-4 h-4 text-emerald-400 shrink-0" />
                  ) : (
                    <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0" />
                  )}
                  <div>
                    <p className="text-xs font-semibold text-slate-100">
                      {res.ok ? 'Artefacto válido contra su destino' : 'El validador lo rechazó'}
                    </p>
                    <p className="text-[10px] text-slate-400 font-mono">
                      {res.tipo} · {res.proveedor} · {(res.ms / 1000).toFixed(1)}s · {res.intentos}{' '}
                      intento(s) · contexto {res.contexto_chars} car. · {res.costo}
                    </p>
                  </div>
                </div>
                <div className="flex items-center gap-1.5 shrink-0">
                  <button
                    onClick={() => void copiar(res.texto)}
                    className="flex items-center gap-1.5 px-3 py-1.5 text-[10px] font-semibold rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-100 transition-colors"
                  >
                    {copiado ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
                    Copiar
                  </button>
                  <button
                    onClick={() => void crearNodo()}
                    className="flex items-center gap-1.5 px-3 py-1.5 text-[10px] font-semibold rounded-lg bg-indigo-600/25 hover:bg-indigo-600/40 text-indigo-200 border border-indigo-500/30 transition-colors"
                  >
                    <Plus className="w-3.5 h-3.5" />
                    Crear nodo
                  </button>
                </div>
              </div>

              {!res.ok && res.problemas.length > 0 && (
                <ul className="text-[11px] text-amber-200 list-disc ml-5">
                  {res.problemas.map((p, i) => (
                    <li key={i}>{p}</li>
                  ))}
                </ul>
              )}

              <Resumen r={res} />

              <div>
                <p className="text-[10px] uppercase tracking-wide text-slate-500 mb-1">
                  Texto listo para el destino
                </p>
                <pre className="text-[11px] text-slate-200 whitespace-pre-wrap break-words bg-slate-950/70 border border-slate-800 rounded-lg p-2 max-h-64 overflow-y-auto font-mono">
                  {res.texto}
                </pre>
              </div>

              <div className="flex flex-wrap items-center gap-x-3 gap-y-1 pt-1">
                <span className="text-[10px] text-slate-500 flex items-center gap-1">
                  <Cpu className="w-3 h-3" /> proveedores probados:
                </span>
                {res.traza?.map((t, i) => (
                  <span
                    key={i}
                    className={`text-[10px] font-mono ${
                      t.valido ? 'text-emerald-400' : t.resultado ? 'text-rose-400' : 'text-amber-400'
                    }`}
                  >
                    {t.proveedor} {t.valido ? '✓' : t.resultado ? '✗ sin respuesta' : '✗ contrato'}{' '}
                    ({(t.ms / 1000).toFixed(0)}s)
                  </span>
                ))}
              </div>

              {res.fuentes?.length > 0 && (
                <p className="text-[10px] text-slate-500">
                  <Sparkles className="w-3 h-3 inline mr-1 text-sky-400" />
                  contexto de la bóveda: {res.fuentes.map((f) => f.titulo).filter(Boolean).join(' · ')}
                </p>
              )}
            </div>
          )}
        </div>

        <div className="px-5 py-3 border-t border-slate-800 bg-slate-950/60 flex items-center justify-between">
          <p className="text-[10px] text-slate-500">
            El experto propone · el validador decide · vos aprobás.
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
