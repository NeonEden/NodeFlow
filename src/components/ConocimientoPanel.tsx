import React, { useState } from 'react';
import {
  BookOpenCheck,
  X,
  Scissors,
  Loader2,
  Check,
  Download,
  FileText,
  Braces,
  AlertTriangle,
} from 'lucide-react';
import {
  previsualizar,
  capturar,
  generarDocumento,
  exportarJson,
  Candidato,
} from '../services/conocimientoService';
import { triggerFileDownload } from '../utils/obsidianExport';

interface ConocimientoPanelProps {
  isOpen: boolean;
  onClose: () => void;
  /** Se llama cuando se crearon propuestas (para refrescar el panel de aprobación). */
  onPropuestasCreadas: () => void;
  showToast: (message: string, type?: 'success' | 'error' | 'info') => void;
}

interface Fila extends Candidato {
  incluir: boolean;
}

export const ConocimientoPanel: React.FC<ConocimientoPanelProps> = ({
  isOpen,
  onClose,
  onPropuestasCreadas,
  showToast,
}) => {
  const [tab, setTab] = useState<'capturar' | 'exportar'>('capturar');
  const [texto, setTexto] = useState('');
  const [padre, setPadre] = useState('');
  const [categoria, setCategoria] = useState('CONOCIMIENTO');
  const [filas, setFilas] = useState<Fila[] | null>(null);
  const [extrayendo, setExtrayendo] = useState(false);
  const [enviando, setEnviando] = useState(false);
  const [exportando, setExportando] = useState<string | null>(null);

  if (!isOpen) return null;

  const extraer = async () => {
    if (texto.trim().length < 40) {
      showToast('Pegá un texto un poco más largo (40 caracteres mínimo)', 'error');
      return;
    }
    setExtrayendo(true);
    const res = await previsualizar(texto);
    setExtrayendo(false);
    if (!res || res.ok === false) {
      showToast(res?.error || 'No pude analizar el texto', 'error');
      return;
    }
    if (!res.candidatos.length) {
      showToast('El texto no produjo candidatos: los bloques son muy cortos', 'info');
      setFilas([]);
      return;
    }
    setFilas(
      res.candidatos.map((c) => ({
        ...c,
        incluir: !c.ya_en_el_lienzo,
        categoria,
        madurez: 2,
      }))
    );
    showToast(`${res.candidatos.length} candidato(s) extraídos de ${res.caracteres} caracteres`, 'info');
  };

  const proponer = async () => {
    const elegidas = (filas || []).filter((f) => f.incluir && f.titulo.trim().length >= 4);
    if (!elegidas.length) {
      showToast('No hay candidatos seleccionados', 'error');
      return;
    }
    setEnviando(true);
    const res = await capturar({
      nodos: elegidas.map((f) => ({
        titulo: f.titulo.trim(),
        descripcion: f.descripcion,
        categoria: f.categoria || categoria,
        madurez: f.madurez ?? 2,
      })),
      parent: padre.trim(),
      categoria,
      madurez: 2,
      motivo: 'Capturado desde el panel Conocimiento',
    });
    setEnviando(false);
    if (!res || res.ok === false) {
      showToast(res?.error || 'No pude crear las propuestas', 'error');
      return;
    }
    showToast(
      `${res.propuestos} propuesta(s) creadas — aprobalas en «Cambios del agente»`,
      'success'
    );
    setFilas(null);
    setTexto('');
    onPropuestasCreadas();
  };

  const descargarDocumento = async () => {
    setExportando('doc');
    const res = await generarDocumento();
    setExportando(null);
    if (!res || res.ok === false) {
      showToast(res?.error || 'No pude generar el documento', 'error');
      return;
    }
    triggerFileDownload(res.contenido, res.nombre, 'text/markdown;charset=utf-8');
    showToast(`Documento generado: ${res.nodos} nodos, ${res.caracteres} caracteres`, 'success');
  };

  const descargarJson = async () => {
    setExportando('json');
    const res = await exportarJson();
    setExportando(null);
    if (!res || res.ok === false) {
      showToast(res?.error || 'No pude exportar el estado', 'error');
      return;
    }
    const nombre = (res as { nombre?: string }).nombre || 'nodeflow-estado.json';
    triggerFileDownload(JSON.stringify(res.estado, null, 2), nombre, 'application/json;charset=utf-8');
    showToast('Estado canónico exportado', 'success');
  };

  const seleccionadas = (filas || []).filter((f) => f.incluir).length;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-fade-in">
      <div
        id="conocimiento-panel"
        className="w-full max-w-3xl max-h-[88vh] flex flex-col bg-slate-900 border border-slate-700/60 rounded-2xl shadow-2xl overflow-hidden"
      >
        {/* Encabezado + pestañas */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800 bg-slate-950/60">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-emerald-500 to-teal-600 flex items-center justify-center">
              <BookOpenCheck className="w-5 h-5 text-white" />
            </div>
            <div>
              <h2 className="text-sm font-bold text-white">Conocimiento</h2>
              <p className="text-[11px] text-slate-400">
                Alimentar la bóveda sin engordarla: se propone, no se escribe.
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <div className="flex rounded-lg border border-slate-700 overflow-hidden">
              {(['capturar', 'exportar'] as const).map((t) => (
                <button
                  key={t}
                  onClick={() => setTab(t)}
                  className={`px-3 py-1.5 text-[11px] font-semibold capitalize transition-colors ${
                    tab === t ? 'bg-slate-800 text-white' : 'text-slate-400 hover:text-slate-200'
                  }`}
                >
                  {t}
                </button>
              ))}
            </div>
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
          {tab === 'capturar' && (
            <>
              <textarea
                value={texto}
                onChange={(e) => setTexto(e.target.value)}
                rows={7}
                placeholder={'Pegá acá tus apuntes, una lista de temas con una línea de detalle cada uno, o el fragmento de un documento.\n\n# Tema\nEl detalle que sirve como guía técnica utilizable…'}
                className="w-full bg-slate-950 border border-slate-700 rounded-xl px-3 py-2 text-sm text-slate-100 placeholder-slate-600 focus:outline-none focus:border-emerald-500 resize-y font-mono"
              />
              <div className="flex flex-wrap items-center gap-2">
                <button
                  onClick={() => void extraer()}
                  disabled={extrayendo}
                  className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-semibold rounded-lg bg-emerald-600/20 hover:bg-emerald-600/35 text-emerald-200 border border-emerald-500/30 disabled:opacity-40 transition-colors"
                >
                  {extrayendo ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Scissors className="w-3.5 h-3.5" />}
                  Extraer conocimiento
                </button>
                <input
                  value={padre}
                  onChange={(e) => setPadre(e.target.value)}
                  placeholder="colgar de (id o título, opcional)"
                  className="flex-1 min-w-[180px] bg-slate-950 border border-slate-700 rounded-lg px-3 py-1.5 text-[11px] text-slate-100 placeholder-slate-600 focus:outline-none focus:border-slate-500"
                />
                <input
                  value={categoria}
                  onChange={(e) => setCategoria(e.target.value.toUpperCase())}
                  placeholder="CATEGORÍA"
                  className="w-36 bg-slate-950 border border-slate-700 rounded-lg px-3 py-1.5 text-[11px] text-slate-100 placeholder-slate-600 focus:outline-none focus:border-slate-500"
                />
              </div>

              {filas !== null && filas.length === 0 && (
                <p className="text-[11px] text-slate-400 py-6 text-center">
                  No se extrajo nada con densidad suficiente. Probá con bloques más largos (70+
                  caracteres) o separá los temas con un título <span className="font-mono"># …</span>.
                </p>
              )}

              {filas && filas.length > 0 && (
                <>
                  <div className="flex items-center justify-between">
                    <p className="text-[11px] text-slate-400">
                      {seleccionadas} de {filas.length} seleccionados · revisá títulos y desmarcá lo que
                      no aporte
                    </p>
                    <button
                      onClick={() => void proponer()}
                      disabled={enviando || seleccionadas === 0}
                      className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-semibold rounded-lg bg-indigo-600/25 hover:bg-indigo-600/40 text-indigo-200 border border-indigo-500/30 disabled:opacity-40 transition-colors"
                    >
                      {enviando ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Check className="w-3.5 h-3.5" />}
                      Proponer {seleccionadas}
                    </button>
                  </div>

                  {filas.map((f, i) => (
                    <div
                      key={i}
                      className="rounded-xl border border-slate-800 bg-slate-950/50 p-3 flex items-start gap-3"
                    >
                      <input
                        type="checkbox"
                        checked={f.incluir}
                        onChange={(e) =>
                          setFilas((prev) =>
                            (prev || []).map((x, j) => (j === i ? { ...x, incluir: e.target.checked } : x))
                          )
                        }
                        className="mt-1 w-4 h-4 accent-indigo-500 shrink-0"
                      />
                      <div className="min-w-0 flex-1">
                        <input
                          value={f.titulo}
                          onChange={(e) =>
                            setFilas((prev) =>
                              (prev || []).map((x, j) => (j === i ? { ...x, titulo: e.target.value } : x))
                            )
                          }
                          className="w-full bg-transparent border-b border-slate-800 focus:border-indigo-500 text-sm text-slate-100 pb-1 mb-1 focus:outline-none"
                        />
                        <p className="text-[11px] text-slate-400 line-clamp-2">{f.descripcion}</p>
                        <p className="text-[10px] text-slate-600 mt-1 font-mono">
                          {f.caracteres} caracteres
                          {f.ya_en_el_lienzo ? ' · ya hay un nodo con ese título' : ''}
                        </p>
                      </div>
                    </div>
                  ))}

                  <p className="text-[10px] text-slate-500 flex items-start gap-1.5">
                    <AlertTriangle className="w-3.5 h-3.5 text-amber-400 shrink-0 mt-0.5" />
                    Una nota de bajo valor empeora las próximas generaciones y el síntoma aparece lejos
                    de la causa. Marcá solo lo que sirva como guía técnica.
                  </p>
                </>
              )}
            </>
          )}

          {tab === 'exportar' && (
            <div className="space-y-3">
              <div className="rounded-xl border border-slate-800 bg-slate-950/50 p-4">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <p className="text-sm text-slate-100 flex items-center gap-2">
                      <FileText className="w-4 h-4 text-sky-300" /> Documento del mapa
                    </p>
                    <p className="text-[11px] text-slate-400 mt-1">
                      El mapa como documento legible: nodos en orden de lectura, categoría, madurez,
                      descripción y conexiones. Para compartir, entregar o sumar al portafolio.
                    </p>
                  </div>
                  <button
                    onClick={() => void descargarDocumento()}
                    disabled={exportando === 'doc'}
                    className="shrink-0 flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-semibold rounded-lg bg-sky-600/20 hover:bg-sky-600/35 text-sky-200 border border-sky-500/30 disabled:opacity-40 transition-colors"
                  >
                    {exportando === 'doc' ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Download className="w-3.5 h-3.5" />}
                    .md
                  </button>
                </div>
              </div>

              <div className="rounded-xl border border-slate-800 bg-slate-950/50 p-4">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <p className="text-sm text-slate-100 flex items-center gap-2">
                      <Braces className="w-4 h-4 text-violet-300" /> Estado canónico (JSON)
                    </p>
                    <p className="text-[11px] text-slate-400 mt-1">
                      El grafo completo sin pérdida: nodos, aristas, apariencia. Sirve para respaldar o
                      re-importar en otra máquina.
                    </p>
                  </div>
                  <button
                    onClick={() => void descargarJson()}
                    disabled={exportando === 'json'}
                    className="shrink-0 flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-semibold rounded-lg bg-violet-600/20 hover:bg-violet-600/35 text-violet-200 border border-violet-500/30 disabled:opacity-40 transition-colors"
                  >
                    {exportando === 'json' ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Download className="w-3.5 h-3.5" />}
                    .json
                  </button>
                </div>
              </div>

              <p className="text-[10px] text-slate-500">
                El bundle completo (las notas <span className="font-mono">.md</span> + el{' '}
                <span className="font-mono">.canvas</span> + el estado) ya vive en tu bóveda de Obsidian:
                para empaquetarlo entero, pedíselo al agente en el chat.
              </p>
            </div>
          )}
        </div>

        <div className="px-5 py-3 border-t border-slate-800 bg-slate-950/60 flex items-center justify-between">
          <p className="text-[10px] text-slate-500">
            Capturar propone · el humano aprueba · la bóveda alimenta al agente.
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
