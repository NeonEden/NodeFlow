import React, { useState, useEffect } from 'react';
import { SavedState, CustomNode, EdgeAppearance } from '../types';
import { Edge } from 'reactflow';
import {
  X,
  Save,
  Download,
  Upload,
  Trash2,
  Clock,
  Layers,
  Sparkles,
  FileJson,
  FileText,
  Copy,
  Check,
  CheckCircle2,
  AlertCircle,
  Share2,
  ExternalLink,
  Code2,
} from 'lucide-react';
import {
  parseGraphToObsidianMarkdown,
  parseGraphToObsidianCanvas,
  downloadObsidianMarkdown,
  downloadObsidianCanvas,
} from '../utils/obsidianExport';

interface SavedStatesModalProps {
  isOpen: boolean;
  onClose: () => void;
  savedStates: SavedState[];
  currentNodes: CustomNode[];
  currentEdges: Edge[];
  currentAppearance: EdgeAppearance;
  userId: string;
  initialTab?: 'saved' | 'obsidian' | 'export' | 'import';
  onLoadState: (state: SavedState) => void;
  onSaveNewState: (name: string) => void;
  onDeleteState: (id: string) => void;
  onImportJSON: (importedData: any) => void;
}

export const SavedStatesModal: React.FC<SavedStatesModalProps> = ({
  isOpen,
  onClose,
  savedStates,
  currentNodes,
  currentEdges,
  currentAppearance,
  userId,
  initialTab = 'saved',
  onLoadState,
  onSaveNewState,
  onDeleteState,
  onImportJSON,
}) => {
  const [newSnapshotName, setNewSnapshotName] = useState('');
  const [importJsonText, setImportJsonText] = useState('');
  const [importError, setImportError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'saved' | 'obsidian' | 'export' | 'import'>(initialTab);
  const [obsidianSubTab, setObsidianSubTab] = useState<'md' | 'canvas'>('md');
  const [copiedMd, setCopiedMd] = useState(false);
  const [copiedCanvas, setCopiedCanvas] = useState(false);

  // Derive default map title from root node
  const rootNode = currentNodes.find((n) => n.data.isRoot);
  const initialMapTitle = rootNode?.data.title || rootNode?.data.label || 'Mapa Conceptual NeuralMind';
  const [mapTitle, setMapTitle] = useState(initialMapTitle);

  useEffect(() => {
    if (isOpen) {
      if (initialTab) setActiveTab(initialTab);
      const root = currentNodes.find((n) => n.data.isRoot);
      if (root?.data.title) {
        setMapTitle(root.data.title);
      }
    }
  }, [isOpen, initialTab, currentNodes]);

  if (!isOpen) return null;

  const currentObsidianMarkdown = parseGraphToObsidianMarkdown(
    currentNodes,
    currentEdges,
    mapTitle.trim() || 'Mapa Conceptual NeuralMind'
  );

  const currentObsidianCanvas = parseGraphToObsidianCanvas(
    currentNodes,
    currentEdges,
    mapTitle.trim() || 'Mapa Conceptual NeuralMind'
  );

  const handleCopyObsidianMd = () => {
    navigator.clipboard.writeText(currentObsidianMarkdown);
    setCopiedMd(true);
    setTimeout(() => setCopiedMd(false), 2000);
  };

  const handleCopyObsidianCanvas = () => {
    navigator.clipboard.writeText(currentObsidianCanvas);
    setCopiedCanvas(true);
    setTimeout(() => setCopiedCanvas(false), 2000);
  };

  const handleDownloadObsidianMd = () => {
    downloadObsidianMarkdown(currentNodes, currentEdges, mapTitle.trim() || 'Mapa Conceptual NeuralMind');
  };

  const handleDownloadObsidianCanvasFile = () => {
    downloadObsidianCanvas(currentNodes, currentEdges, mapTitle.trim() || 'Mapa Conceptual NeuralMind');
  };

  const handleSaveSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newSnapshotName.trim()) return;
    onSaveNewState(newSnapshotName.trim());
    setNewSnapshotName('');
  };

  const handleDownloadJSON = () => {
    const payload = {
      app: 'NeuralMind',
      version: '1.0.0',
      exportedAt: new Date().toISOString(),
      metadata: {
        nodeCount: currentNodes.length,
        edgeCount: currentEdges.length,
      },
      edgeAppearance: currentAppearance,
      nodes: currentNodes,
      edges: currentEdges,
    };

    const dataStr = 'data:text/json;charset=utf-8,' + encodeURIComponent(JSON.stringify(payload, null, 2));
    const downloadAnchor = document.createElement('a');
    downloadAnchor.setAttribute('href', dataStr);
    downloadAnchor.setAttribute(
      'download',
      `neuralmind-map-${new Date().toISOString().slice(0, 10)}.json`
    );
    document.body.appendChild(downloadAnchor);
    downloadAnchor.click();
    downloadAnchor.remove();
  };

  const handleFileUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (event) => {
      try {
        const parsed = JSON.parse(event.target?.result as string);
        onImportJSON(parsed);
        onClose();
      } catch (err) {
        setImportError('El archivo seleccionado no contiene un JSON válido.');
      }
    };
    reader.readAsText(file);
  };

  const handleImportTextSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setImportError(null);
    try {
      const parsed = JSON.parse(importJsonText);
      onImportJSON(parsed);
      onClose();
    } catch (err) {
      setImportError('JSON inválido. Revisa la sintaxis e intenta de nuevo.');
    }
  };

  // Filter states for current user or shared
  const userStates = savedStates.filter(
    (s) => s.userId === userId || s.userId === 'default'
  );

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-sm animate-in fade-in duration-150">
      <div
        className="w-full max-w-xl bg-slate-900 border border-slate-700/80 rounded-2xl shadow-2xl p-6 text-slate-100 relative max-h-[90vh] flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between pb-4 border-b border-slate-800">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-lg bg-indigo-600 flex items-center justify-center text-white shadow-md">
              <Layers size={16} />
            </div>
            <div>
              <h2 className="text-base font-semibold text-white">Estados y Exportación</h2>
              <p className="text-xs text-slate-400">Guarda puntos de control o exporta tu red conceptual</p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-1.5 text-slate-400 hover:text-white hover:bg-slate-800 rounded-lg transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        {/* Tab switcher */}
        <div className="flex bg-slate-950 p-1 rounded-xl border border-slate-800 my-4 text-xs font-medium shrink-0">
          <button
            type="button"
            onClick={() => setActiveTab('saved')}
            className={`flex-1 py-1.5 rounded-lg flex items-center justify-center gap-1.5 transition-all ${
              activeTab === 'saved'
                ? 'bg-indigo-600 text-white shadow-sm'
                : 'text-slate-400 hover:text-white'
            }`}
          >
            <Save size={14} /> Estados ({userStates.length})
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('obsidian')}
            className={`flex-1 py-1.5 rounded-lg flex items-center justify-center gap-1.5 transition-all ${
              activeTab === 'obsidian'
                ? 'bg-purple-600 text-white shadow-sm font-semibold'
                : 'text-purple-400 hover:text-purple-200'
            }`}
          >
            <Sparkles size={14} className={activeTab === 'obsidian' ? 'text-white' : 'text-purple-400'} />
            <span>Obsidian (.md / .canvas)</span>
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('export')}
            className={`flex-1 py-1.5 rounded-lg flex items-center justify-center gap-1.5 transition-all ${
              activeTab === 'export'
                ? 'bg-indigo-600 text-white shadow-sm'
                : 'text-slate-400 hover:text-white'
            }`}
          >
            <FileJson size={14} /> JSON
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('import')}
            className={`flex-1 py-1.5 rounded-lg flex items-center justify-center gap-1.5 transition-all ${
              activeTab === 'import'
                ? 'bg-indigo-600 text-white shadow-sm'
                : 'text-slate-400 hover:text-white'
            }`}
          >
            <Upload size={14} /> Importar
          </button>
        </div>

        {/* Body content */}
        <div className="flex-1 overflow-y-auto space-y-4 pr-1">
          {/* TAB 1: SAVED STATES */}
          {activeTab === 'saved' && (
            <div className="space-y-4">
              {/* Quick save box */}
              <form onSubmit={handleSaveSubmit} className="p-3 bg-slate-950/80 rounded-xl border border-slate-800">
                <label className="text-xs font-medium text-slate-300 block mb-1.5">
                  Guardar Estado Actual
                </label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    required
                    placeholder="Nombre del estado (ej. Arquitectura v1.2)"
                    value={newSnapshotName}
                    onChange={(e) => setNewSnapshotName(e.target.value)}
                    className="flex-1 px-3 py-1.5 bg-slate-900 border border-slate-700 rounded-lg text-xs text-white focus:outline-none focus:border-indigo-500"
                  />
                  <button
                    type="submit"
                    className="px-4 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-semibold rounded-lg shadow transition-colors flex items-center gap-1 cursor-pointer"
                  >
                    <Save size={13} /> Guardar
                  </button>
                </div>
                <div className="text-[11px] text-slate-400 mt-2 flex items-center gap-3">
                  <span>Nodos actuales: <strong className="text-indigo-300">{currentNodes.length}</strong></span>
                  <span>Conexiones: <strong className="text-emerald-300">{currentEdges.length}</strong></span>
                </div>
              </form>

              {/* Saved list */}
              <div>
                <label className="text-xs font-medium text-slate-400 block mb-2">
                  Puntos de Restauración Guardados
                </label>
                {userStates.length === 0 ? (
                  <div className="p-6 text-center bg-slate-950/40 rounded-xl border border-dashed border-slate-800 text-xs text-slate-400">
                    Aún no has guardado ningún estado para este usuario. Guarda tu primer punto de control arriba.
                  </div>
                ) : (
                  <div className="space-y-2">
                    {userStates.map((state) => (
                      <div
                        key={state.id}
                        className="p-3 bg-slate-950/60 hover:bg-slate-950 rounded-xl border border-slate-800 flex items-center justify-between transition-colors group"
                      >
                        <div className="min-w-0 flex-1 mr-3">
                          <div className="text-xs font-semibold text-white flex items-center gap-2">
                            <span>{state.name}</span>
                            <span
                              className="w-2.5 h-2.5 rounded-full"
                              style={{ backgroundColor: state.edgeAppearance?.color || '#6366f1' }}
                              title="Color de conexiones"
                            />
                          </div>
                          <div className="text-[10px] text-slate-400 flex items-center gap-3 mt-1">
                            <span className="flex items-center gap-1 font-mono">
                              <Clock size={11} /> {new Date(state.timestamp).toLocaleDateString()} {new Date(state.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                            </span>
                            <span>{state.nodeCount} nodos</span>
                            <span>{state.edgeCount} conexiones</span>
                          </div>
                        </div>

                        <div className="flex items-center gap-2">
                          <button
                            type="button"
                            onClick={() => {
                              onLoadState(state);
                              onClose();
                            }}
                            className="px-3 py-1.5 bg-indigo-950 text-indigo-300 hover:bg-indigo-900 border border-indigo-700/50 rounded-lg text-xs font-medium transition-colors cursor-pointer"
                          >
                            Cargar
                          </button>
                          <button
                            type="button"
                            onClick={() => onDeleteState(state.id)}
                            className="p-1.5 text-slate-500 hover:text-rose-400 hover:bg-slate-800 rounded-lg transition-colors cursor-pointer"
                            title="Eliminar estado"
                          >
                            <Trash2 size={14} />
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}

          {/* TAB 2: OBSIDIAN EXPORT (MODULE 1: .md + Frontmatter & .canvas Spec) */}
          {activeTab === 'obsidian' && (
            <div className="space-y-4 text-xs">
              {/* Header explanation & Title input */}
              <div className="p-4 bg-purple-950/20 border border-purple-500/30 rounded-2xl space-y-3">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2 text-sm font-bold text-white">
                    <div className="w-6 h-6 rounded-lg bg-purple-600/40 border border-purple-400/50 flex items-center justify-center text-purple-200">
                      <Sparkles size={13} />
                    </div>
                    <span>Exportación e Integración con Obsidian</span>
                  </div>
                  <span className="px-2 py-0.5 rounded-full bg-purple-500/20 border border-purple-500/40 text-purple-300 text-[10px] font-mono">
                    Obsidian Ready
                  </span>
                </div>
                <p className="text-slate-300 text-[11px] leading-relaxed">
                  Traduce la red visual de NeuralMind a formatos nativos de Obsidian: notas interconectadas con <strong>Frontmatter YAML</strong> y <strong>[[wikilinks]]</strong>, o el archivo <strong>.canvas</strong> espacial oficial con posiciones y flechas.
                </p>

                {/* Map Title Input */}
                <div className="pt-2 border-t border-purple-500/20 flex flex-col sm:flex-row items-stretch sm:items-center gap-2">
                  <label className="text-[11px] font-medium text-purple-200 whitespace-nowrap">
                    Título del Documento:
                  </label>
                  <input
                    type="text"
                    value={mapTitle}
                    onChange={(e) => setMapTitle(e.target.value)}
                    placeholder="Nombre del archivo (ej. Ecosistema IA)"
                    className="flex-1 px-3 py-1.5 bg-slate-950 border border-slate-700 focus:border-purple-500 rounded-lg text-xs text-white placeholder-slate-500 focus:outline-none transition-colors"
                  />
                </div>
              </div>

              {/* 2 Delivery Methods Grid */}
              <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                {/* Method 1: Markdown + YAML + Wikilinks (.md) */}
                <div className="p-3.5 bg-slate-950 rounded-xl border border-slate-800 hover:border-purple-500/40 transition-colors flex flex-col justify-between space-y-3">
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-1.5 font-bold text-slate-100 text-xs">
                        <FileText size={15} className="text-emerald-400" />
                        <span>1.1 Formato Markdown (.md)</span>
                      </div>
                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-emerald-950/80 text-emerald-300 border border-emerald-800/60 font-mono">
                        [[wikilinks]]
                      </span>
                    </div>
                    <p className="text-slate-400 text-[11px] leading-snug">
                      Incluye Frontmatter YAML (título, fecha, tags, nodos_totales) y cada nodo enlazado bidireccionalmente con relaciones de salida.
                    </p>
                  </div>

                  <div className="flex items-center gap-2 pt-1">
                    <button
                      type="button"
                      onClick={handleDownloadObsidianMd}
                      className="flex-1 py-2 bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg text-xs font-semibold flex items-center justify-center gap-1.5 shadow transition-colors cursor-pointer"
                    >
                      <Download size={13} />
                      <span>Descargar .md</span>
                    </button>
                    <button
                      type="button"
                      onClick={handleCopyObsidianMd}
                      className="px-3 py-2 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-lg text-xs font-medium flex items-center justify-center gap-1 transition-colors cursor-pointer"
                      title="Copiar contenido Markdown"
                    >
                      {copiedMd ? <Check size={13} className="text-emerald-400" /> : <Copy size={13} />}
                      <span>{copiedMd ? 'Copiado' : 'Copiar'}</span>
                    </button>
                  </div>
                </div>

                {/* Method 2: Obsidian Canvas Spec (.canvas) */}
                <div className="p-3.5 bg-slate-950 rounded-xl border border-slate-800 hover:border-purple-500/40 transition-colors flex flex-col justify-between space-y-3">
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-1.5 font-bold text-slate-100 text-xs">
                        <Code2 size={15} className="text-purple-400" />
                        <span>1.2 Obsidian Canvas (.canvas)</span>
                      </div>
                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-purple-950/80 text-purple-300 border border-purple-800/60 font-mono">
                        JSON Espacial
                      </span>
                    </div>
                    <p className="text-slate-400 text-[11px] leading-snug">
                      JSON nativo para el plugin oficial Obsidian Canvas: conserva coordenadas exactas (x, y), colores, tarjetas y flechas de conexión.
                    </p>
                  </div>

                  <div className="flex items-center gap-2 pt-1">
                    <button
                      type="button"
                      onClick={handleDownloadObsidianCanvasFile}
                      className="flex-1 py-2 bg-purple-600 hover:bg-purple-500 text-white rounded-lg text-xs font-semibold flex items-center justify-center gap-1.5 shadow transition-colors cursor-pointer"
                    >
                      <Download size={13} />
                      <span>Descargar .canvas</span>
                    </button>
                    <button
                      type="button"
                      onClick={handleCopyObsidianCanvas}
                      className="px-3 py-2 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-lg text-xs font-medium flex items-center justify-center gap-1 transition-colors cursor-pointer"
                      title="Copiar JSON del Canvas"
                    >
                      {copiedCanvas ? <Check size={13} className="text-purple-400" /> : <Copy size={13} />}
                      <span>{copiedCanvas ? 'Copiado' : 'Copiar'}</span>
                    </button>
                  </div>
                </div>
              </div>

              {/* Preview Toggle & Inspection Box */}
              <div className="bg-slate-950 rounded-xl border border-slate-800 p-3 space-y-2">
                <div className="flex items-center justify-between border-b border-slate-800/80 pb-2">
                  <div className="flex items-center gap-2">
                    <span className="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">
                      Vista Previa de Salida
                    </span>
                    <div className="flex bg-slate-900 rounded-lg p-0.5 border border-slate-800 text-[10px]">
                      <button
                        type="button"
                        onClick={() => setObsidianSubTab('md')}
                        className={`px-2.5 py-1 rounded-md transition-colors ${
                          obsidianSubTab === 'md'
                            ? 'bg-slate-800 text-emerald-300 font-semibold'
                            : 'text-slate-400 hover:text-white'
                        }`}
                      >
                        Markdown (.md)
                      </button>
                      <button
                        type="button"
                        onClick={() => setObsidianSubTab('canvas')}
                        className={`px-2.5 py-1 rounded-md transition-colors ${
                          obsidianSubTab === 'canvas'
                            ? 'bg-slate-800 text-purple-300 font-semibold'
                            : 'text-slate-400 hover:text-white'
                        }`}
                      >
                        Canvas Spec (.canvas)
                      </button>
                    </div>
                  </div>

                  <span className="text-[10px] text-slate-500 font-mono">
                    {currentNodes.length} nodos • {currentEdges.length} conexiones
                  </span>
                </div>

                <div className="max-h-52 overflow-y-auto bg-slate-900/90 p-3 rounded-lg border border-slate-800/60 font-mono text-[11px] text-slate-300 whitespace-pre-wrap select-all leading-relaxed">
                  {obsidianSubTab === 'md' ? currentObsidianMarkdown : currentObsidianCanvas}
                </div>

                <p className="text-[10px] text-slate-500 italic pt-1">
                  💡 Tip: Puedes arrastrar o copiar el archivo descargado directamente a la carpeta de tu Vault en Obsidian.
                </p>
              </div>
            </div>
          )}

          {/* TAB 3: EXPORT JSON */}
          {activeTab === 'export' && (
            <div className="space-y-4 text-xs">
              <div className="p-4 bg-slate-950 rounded-xl border border-slate-800 space-y-3">
                <div className="flex items-center gap-2 text-sm font-semibold text-white">
                  <FileJson size={18} className="text-indigo-400" />
                  <span>Exportación de Diseño en JSON</span>
                </div>
                <p className="text-slate-400 leading-relaxed">
                  Exporta toda la estructura de nodos, posiciones relativas, etiquetas personalizadas, metadatos y configuración de conexiones a un archivo portable JSON estándar.
                </p>

                <div className="bg-slate-900 p-3 rounded-lg border border-slate-800 font-mono text-[11px] text-slate-300 space-y-1">
                  <div>• Nodos incluidos: <strong>{currentNodes.length}</strong></div>
                  <div>• Conexiones incluidas: <strong>{currentEdges.length}</strong></div>
                  <div>• Paleta de color: <strong style={{ color: currentAppearance.color }}>{currentAppearance.color}</strong> ({currentAppearance.type})</div>
                </div>

                <button
                  type="button"
                  onClick={handleDownloadJSON}
                  className="w-full flex items-center justify-center gap-2 py-2.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded-xl font-semibold shadow-lg shadow-indigo-600/30 transition-colors cursor-pointer"
                >
                  <Download size={15} /> Descargar Archivo JSON (.json)
                </button>
              </div>
            </div>
          )}

          {/* TAB 4: IMPORT JSON */}
          {activeTab === 'import' && (
            <div className="space-y-4 text-xs">
              {importError && (
                <div className="p-3 bg-rose-950/40 border border-rose-800/60 rounded-xl text-rose-300 flex items-center gap-2">
                  <AlertCircle size={16} />
                  <span>{importError}</span>
                </div>
              )}

              {/* Upload file */}
              <div className="p-4 bg-slate-950 rounded-xl border border-dashed border-slate-700 hover:border-indigo-500 transition-colors text-center">
                <Upload size={24} className="mx-auto text-indigo-400 mb-2" />
                <div className="font-semibold text-white mb-1">Cargar archivo JSON desde tu equipo</div>
                <p className="text-slate-400 text-[11px] mb-3">Arrastra o selecciona un archivo exportado previamente</p>
                <label className="inline-block px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-xl cursor-pointer font-medium transition-colors">
                  Seleccionar Archivo .json
                  <input
                    type="file"
                    accept=".json,application/json"
                    onChange={handleFileUpload}
                    className="hidden"
                  />
                </label>
              </div>

              {/* Paste JSON text */}
              <form onSubmit={handleImportTextSubmit} className="space-y-2">
                <label className="block text-slate-400 font-medium">O pega el contenido JSON directamente:</label>
                <textarea
                  rows={5}
                  value={importJsonText}
                  onChange={(e) => setImportJsonText(e.target.value)}
                  placeholder='{"nodes": [...], "edges": [...]}'
                  className="w-full p-3 bg-slate-950 border border-slate-800 rounded-xl text-xs font-mono text-slate-200 focus:outline-none focus:border-indigo-500 resize-none"
                />
                <button
                  type="submit"
                  disabled={!importJsonText.trim()}
                  className="w-full py-2 bg-slate-800 hover:bg-slate-700 disabled:opacity-50 text-white rounded-xl font-medium transition-colors"
                >
                  Importar y Cargar en Lienzo
                </button>
              </form>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
