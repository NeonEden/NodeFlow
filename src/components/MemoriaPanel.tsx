import React, { useState } from 'react';
import { Search, X, Loader2, FileText, Send, LibraryBig, ChevronDown, ChevronRight, Check } from 'lucide-react';
import {
  buscarEnVault,
  leerNotaDeVault,
  traerNotaAlLienzo,
  ResultadoMemoria,
} from '../services/memoriaService';

interface MemoriaPanelProps {
  isOpen: boolean;
  onClose: () => void;
  /** Se llama cuando se creó una propuesta (para refrescar el panel de aprobación). */
  onPropuestaCreada: () => void;
  showToast: (message: string, type?: 'success' | 'error' | 'info') => void;
}

export const MemoriaPanel: React.FC<MemoriaPanelProps> = ({
  isOpen,
  onClose,
  onPropuestaCreada,
  showToast,
}) => {
  const [consulta, setConsulta] = useState('');
  const [buscando, setBuscando] = useState(false);
  const [docs, setDocs] = useState<number | null>(null);
  const [raiz, setRaiz] = useState('');
  const [resultados, setResultados] = useState<ResultadoMemoria[] | null>(null);
  const [buscado, setBuscado] = useState('');
  const [abierta, setAbierta] = useState<string | null>(null);
  const [textoAbierto, setTextoAbierto] = useState<string>('');
  const [enviadas, setEnviadas] = useState<Record<string, string>>({});
  const [ocupado, setOcupado] = useState<string | null>(null);

  if (!isOpen) return null;

  const buscar = async () => {
    const q = consulta.trim();
    if (!q) return;
    setBuscando(true);
    const res = await buscarEnVault(q, 10);
    setBuscando(false);
    if (!res) {
      showToast('No pude consultar la memoria del vault', 'error');
      return;
    }
    if (!res.ok) {
      showToast(res.error || 'Consulta inválida', 'error');
      return;
    }
    setDocs(res.docs_indexados ?? null);
    setRaiz(res.raiz || '');
    setResultados(res.resultados || []);
    setBuscado(q);
    setAbierta(null);
  };

  const alternar = async (r: ResultadoMemoria) => {
    if (abierta === r.ruta) {
      setAbierta(null);
      return;
    }
    setAbierta(r.ruta);
    setTextoAbierto('cargando…');
    const nota = await leerNotaDeVault(r.ruta, 3000);
    setTextoAbierto(nota?.texto || nota?.error || 'no pude leer la nota');
  };

  const traer = async (r: ResultadoMemoria) => {
    setOcupado(r.ruta);
    const nota = await leerNotaDeVault(r.ruta, 6000);
    if (!nota?.texto) {
      setOcupado(null);
      showToast('No pude leer la nota', 'error');
      return;
    }
    const res = await traerNotaAlLienzo(r, nota.texto);
    setOcupado(null);
    if (!res || res.ok === false) {
      showToast(res?.error || 'No pude crear la propuesta', 'error');
      return;
    }
    setEnviadas((prev) => ({ ...prev, [r.ruta]: res.id_pendiente || 'pendiente' }));
    showToast(`Propuesta creada: «${r.titulo}» — aprobala en «Cambios del agente»`, 'success');
    onPropuestaCreada();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-fade-in">
      <div
        id="memoria-panel"
        className="w-full max-w-3xl max-h-[86vh] flex flex-col bg-slate-900 border border-slate-700/60 rounded-2xl shadow-2xl overflow-hidden"
      >
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800 bg-slate-950/60">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-sky-500 to-cyan-600 flex items-center justify-center">
              <LibraryBig className="w-5 h-5 text-white" />
            </div>
            <div>
              <h2 className="text-sm font-bold text-white">Memoria del vault</h2>
              <p className="text-[11px] text-slate-400">
                {docs !== null
                  ? `${docs} nota(s) indexadas${raiz ? ` · ${raiz}` : ''}`
                  : 'Buscá en todas tus notas de Obsidian y traelas al lienzo.'}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 text-slate-400 hover:text-white rounded-lg hover:bg-slate-800 transition-colors"
            title="Cerrar"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="px-5 py-3 border-b border-slate-800 flex items-center gap-2">
          <div className="relative flex-1">
            <Search className="w-4 h-4 text-slate-500 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              autoFocus
              value={consulta}
              onChange={(e) => setConsulta(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') void buscar();
              }}
              placeholder="Ej: automatización, visuales touchdesigner, cold outreach…"
              className="w-full bg-slate-950 border border-slate-700 rounded-lg pl-9 pr-3 py-2 text-sm text-slate-100 placeholder-slate-500 focus:outline-none focus:border-sky-500"
            />
          </div>
          <button
            onClick={() => void buscar()}
            disabled={buscando || !consulta.trim()}
            className="px-4 py-2 text-xs font-semibold rounded-lg bg-sky-600/25 hover:bg-sky-600/40 text-sky-200 border border-sky-500/30 disabled:opacity-40 transition-colors"
          >
            {buscando ? <Loader2 className="w-4 h-4 animate-spin" /> : 'Buscar'}
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-2">
          {resultados === null && (
            <div className="text-center py-12">
              <LibraryBig className="w-9 h-9 text-slate-600 mx-auto mb-3" />
              <p className="text-sm text-slate-300">Buscá en lo que ya escribiste</p>
              <p className="text-[11px] text-slate-500 mt-1 max-w-md mx-auto">
                El índice vive en memoria (BM25, sin modelos ni nube): encuentra por título, cuerpo y
                tags, ignorando acentos. «Traer al lienzo» crea una propuesta, no escribe directo.
              </p>
            </div>
          )}

          {resultados !== null && resultados.length === 0 && (
            <p className="text-sm text-slate-400 text-center py-10">
              Sin coincidencias para «{buscado}».
            </p>
          )}

          {resultados?.map((r) => (
            <div key={r.ruta} className="rounded-xl border border-slate-800 bg-slate-950/50 p-3 hover:border-slate-700 transition-colors">
              <div className="flex items-start justify-between gap-3">
                <button
                  onClick={() => void alternar(r)}
                  className="min-w-0 text-left flex-1"
                  title="Ver la nota completa"
                >
                  <div className="flex items-center gap-2 mb-1">
                    {abierta === r.ruta ? (
                      <ChevronDown className="w-3.5 h-3.5 text-slate-500 shrink-0" />
                    ) : (
                      <ChevronRight className="w-3.5 h-3.5 text-slate-500 shrink-0" />
                    )}
                    <span className="text-sm text-slate-100 truncate">{r.titulo}</span>
                    <span className="text-[10px] font-mono text-sky-300/80">{r.puntaje}</span>
                    {r.ya_en_el_lienzo && (
                      <span className="px-1.5 py-0.5 text-[9px] rounded bg-indigo-500/20 text-indigo-300 border border-indigo-500/30">
                        nodo del lienzo
                      </span>
                    )}
                  </div>
                  <p className="text-[10px] text-slate-500 font-mono truncate pl-6">{r.ruta}</p>
                  <p className="text-[11px] text-slate-400 pl-6 mt-1">{r.fragmento}</p>
                </button>
                <button
                  onClick={() => void traer(r)}
                  disabled={ocupado === r.ruta || !!enviadas[r.ruta]}
                  className="shrink-0 flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-semibold rounded-lg bg-indigo-600/20 hover:bg-indigo-600/35 text-indigo-200 border border-indigo-500/30 disabled:opacity-40 transition-colors"
                  title="Crear una propuesta de nodo desde esta nota"
                >
                  {ocupado === r.ruta ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  ) : enviadas[r.ruta] ? (
                    <Check className="w-3.5 h-3.5" />
                  ) : (
                    <Send className="w-3.5 h-3.5" />
                  )}
                  {enviadas[r.ruta] ? 'Propuesto' : 'Traer al lienzo'}
                </button>
              </div>

              {abierta === r.ruta && (
                <pre className="mt-2 ml-6 max-h-56 overflow-y-auto whitespace-pre-wrap text-[11px] text-slate-300 bg-slate-900/70 border border-slate-800 rounded-lg p-2">
                  {textoAbierto}
                </pre>
              )}
            </div>
          ))}
        </div>

        <div className="px-5 py-3 border-t border-slate-800 bg-slate-950/60 flex items-center justify-between">
          <p className="text-[10px] text-slate-500 flex items-center gap-1.5">
            <FileText className="w-3 h-3" /> Índice local, se refresca solo cada 30 s.
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
