import React from 'react';
import {
  Sparkles,
  Combine,
  Plus,
  Undo2,
  Redo2,
  Download,
  LayoutTemplate,
  RotateCcw,
  PanelLeftClose,
  PanelLeftOpen,
  Search,
  X,
  Network,
  Compass,
  Trash2,
  Check,
  Loader2,
  Brain,
  Keyboard,
  Key,
  Zap,
} from 'lucide-react';
import { ColorPickerMenu } from './ColorPickerMenu';
import { EdgeAppearance, UserProfile } from '../types';

interface ToolbarProps {
  canUndo: boolean;
  canRedo: boolean;
  undoCount: number;
  redoCount: number;
  selectedNodesCount: number;
  edgeAppearance: EdgeAppearance;
  currentUser: UserProfile | null;
  onUndo: () => void;
  onRedo: () => void;
  onAddNode: (isRoot?: boolean) => void;
  onHybridize: () => void;
  onOpenStatesModal: () => void;
  onOpenAuthModal: () => void;
  onEdgeAppearanceChange: (app: EdgeAppearance) => void;
  onApplyEdgeToSelected?: () => void;
  selectedEdgeCount?: number;
  onSelectTemplate: (templateId: string) => void;
  onResetCanvas: () => void;
  onOpenTemplatesModal?: () => void;
  onOpenClearModal?: () => void;
  saveStatus?: 'saved' | 'saving' | 'unsaved';
  isAiProcessing?: boolean;
  isSidebarOpen?: boolean;
  onToggleSidebar?: () => void;
  onAutoLayout?: () => void;
  onOpenSynthesis?: () => void;
  onOpenObsidianModal?: () => void;
  onOpenHitlModal?: () => void;
  hitlDecisionsCount?: number;
  onOpenShortcuts?: () => void;
  onOpenApiKeyModal?: () => void;
  hasCustomApiKey?: boolean;
  onOpenBrainDump?: () => void;
  onOpenBridgesModal?: () => void;
  searchQuery?: string;
  onSearchChange?: (q: string) => void;
}

export const Toolbar: React.FC<ToolbarProps> = ({
  canUndo,
  canRedo,
  selectedNodesCount,
  edgeAppearance,
  currentUser,
  onUndo,
  onRedo,
  onAddNode,
  onHybridize,
  onOpenStatesModal,
  onOpenAuthModal,
  onEdgeAppearanceChange,
  onApplyEdgeToSelected,
  selectedEdgeCount = 0,
  onSelectTemplate,
  onResetCanvas,
  onOpenTemplatesModal,
  onOpenClearModal,
  saveStatus = 'saved',
  isAiProcessing = false,
  isSidebarOpen = true,
  onToggleSidebar,
  onAutoLayout,
  onOpenSynthesis,
  onOpenObsidianModal,
  onOpenHitlModal,
  hitlDecisionsCount,
  onOpenShortcuts,
  onOpenApiKeyModal,
  hasCustomApiKey = false,
  onOpenBrainDump,
  onOpenBridgesModal,
  searchQuery = '',
  onSearchChange,
}) => {
  // Preset quick dots from the design: Emerald, Indigo, Rose, Amber
  const quickColors = [
    { hex: '#10b981', title: 'Emerald' },
    { hex: '#6366f1', title: 'Indigo' },
    { hex: '#f43f5e', title: 'Rose' },
    { hex: '#f59e0b', title: 'Amber' },
  ];

  return (
    <header className="h-16 border-b border-slate-800 flex items-center justify-between px-3 md:px-5 bg-slate-950/80 backdrop-blur-md z-30 shrink-0 gap-2">
      {/* Brand Logo with Sparkles & Search */}
      <div className="flex items-center gap-2 md:gap-3">
        {onToggleSidebar && (
          <button
            type="button"
            onClick={onToggleSidebar}
            className="p-1.5 hover:bg-slate-800 rounded-lg text-slate-400 hover:text-slate-200 transition-colors"
            title={isSidebarOpen ? 'Ocultar barra lateral' : 'Mostrar barra lateral'}
          >
            {isSidebarOpen ? <PanelLeftClose size={16} /> : <PanelLeftOpen size={16} />}
          </button>
        )}
        <div className="w-8 h-8 bg-indigo-600 rounded-lg flex items-center justify-center shadow-lg shadow-indigo-500/30 shrink-0">
          <Sparkles className="text-white" size={18} />
        </div>
        <div className="flex items-center">
          <h1 className="text-lg md:text-xl font-bold tracking-tighter text-white">
            NEURAL<span className="text-indigo-500">MIND</span>
          </h1>
        </div>

        {/* Autosave persistence status indicator */}
        <div className="hidden sm:flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-slate-900/70 border border-slate-800/80 text-[11px] text-slate-400">
          {saveStatus === 'saving' ? (
            <>
              <Loader2 size={11} className="text-indigo-400 animate-spin" />
              <span>Guardando...</span>
            </>
          ) : (
            <>
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shadow-sm shadow-emerald-400/50" />
              <span>Persistido</span>
            </>
          )}
        </div>

        {/* Quick Search Bar */}
        {onSearchChange && (
          <div className="relative hidden md:flex items-center ml-1">
            <Search size={13} className="absolute left-2.5 text-slate-500" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => onSearchChange(e.target.value)}
              placeholder="Buscar nodos... (Ctrl+F)"
              className="bg-slate-900 border border-slate-800 text-xs text-slate-200 rounded-lg pl-7 pr-7 py-1.5 w-32 lg:w-40 focus:w-52 transition-all focus:outline-none focus:border-indigo-500 placeholder:text-slate-500"
            />
            {searchQuery && (
              <button
                type="button"
                onClick={() => onSearchChange('')}
                className="absolute right-2 text-slate-500 hover:text-slate-300"
              >
                <X size={12} />
              </button>
            )}
          </div>
        )}
      </div>

      {/* Center Tools: Undo/Redo, Auto-Layout, Connection Color Styling */}
      <div className="flex items-center gap-1.5 md:gap-2">
        {/* Undo / Redo Group */}
        <div className="flex items-center gap-0.5 border-r border-slate-800/80 pr-1.5">
          <button
            type="button"
            id="btn-undo"
            onClick={onUndo}
            disabled={!canUndo}
            title="Deshacer (Ctrl+Z)"
            className="p-1.5 hover:bg-slate-800 rounded text-slate-400 hover:text-slate-200 disabled:opacity-30 disabled:hover:bg-transparent transition-colors cursor-pointer"
          >
            <Undo2 size={15} />
          </button>
          <button
            type="button"
            id="btn-redo"
            onClick={onRedo}
            disabled={!canRedo}
            title="Rehacer (Ctrl+Y)"
            className="p-1.5 hover:bg-slate-800 rounded text-slate-400 hover:text-slate-200 disabled:opacity-30 disabled:hover:bg-transparent transition-colors cursor-pointer"
          >
            <Redo2 size={15} />
          </button>
        </div>

        {/* Auto Layout Button */}
        {onAutoLayout && (
          <button
            type="button"
            onClick={onAutoLayout}
            className="flex items-center gap-1 bg-slate-900 hover:bg-slate-800 border border-slate-800 text-slate-300 hover:text-white px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors cursor-pointer"
            title="Auto-organizar nodos en jerarquía limpia (evita solapamiento)"
          >
            <Network size={14} className="text-indigo-400" />
            <span className="hidden xl:inline">Organizar</span>
          </button>
        )}

        {/* Quick Color Dots */}
        <div className="hidden xl:flex items-center gap-1.5 bg-slate-900/60 p-1 rounded-lg border border-slate-800">
          {quickColors.map((qc) => {
            const isActive = edgeAppearance.color.toLowerCase() === qc.hex.toLowerCase();
            return (
              <button
                key={qc.hex}
                type="button"
                onClick={() => onEdgeAppearanceChange({ ...edgeAppearance, color: qc.hex })}
                className={`w-3.5 h-3.5 rounded-full cursor-pointer transition-transform ${
                  isActive ? 'border-2 border-white scale-110' : 'border border-slate-700 hover:scale-105'
                }`}
                style={{ backgroundColor: qc.hex }}
                title={`${qc.title} ${isActive ? '(Activo)' : ''}`}
              />
            );
          })}
        </div>

        {/* Full Connection Styles Popover */}
        <ColorPickerMenu
          appearance={edgeAppearance}
          onChange={onEdgeAppearanceChange}
          onApplyToSelectedEdges={onApplyEdgeToSelected}
          selectedEdgeCount={selectedEdgeCount}
        />
      </div>

      {/* Right Controls: Nodo Raíz, Hibridador IA, Síntesis, Exportar, and Profile */}
      <div className="flex items-center gap-1.5 md:gap-2">
        <button
          type="button"
          onClick={() => onAddNode(true)}
          className="flex items-center gap-1.5 bg-slate-800 hover:bg-slate-700 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors border border-slate-700 text-slate-200 cursor-pointer"
          title="Crear un nuevo nodo raíz (Ctrl+N)"
        >
          <Plus size={14} /> <span className="hidden sm:inline">Nodo Raíz</span>
        </button>

        <button
          type="button"
          onClick={onHybridize}
          disabled={isAiProcessing}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors shadow-lg cursor-pointer ${
            selectedNodesCount >= 2
              ? 'bg-indigo-600 hover:bg-indigo-500 shadow-indigo-500/30 text-white ring-2 ring-indigo-400'
              : 'bg-indigo-600 hover:bg-indigo-500 shadow-indigo-500/20 text-white'
          }`}
          title="Descubrir sinergias y fusionar ideas seleccionadas con IA"
        >
          <Combine size={14} className={isAiProcessing ? 'animate-spin' : ''} />
          <span>Hibridador IA</span>
          {selectedNodesCount >= 2 && (
            <span className="bg-indigo-900 text-indigo-200 text-[10px] px-1.5 py-0.2 rounded-full font-mono">
              {selectedNodesCount}
            </span>
          )}
        </button>

        {/* Brain Dump Rapid Input Button */}
        {onOpenBrainDump && (
          <button
            type="button"
            id="btn-toolbar-braindump"
            onClick={onOpenBrainDump}
            className="flex items-center gap-1.5 bg-emerald-950/40 hover:bg-emerald-900/60 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors border border-emerald-500/40 text-emerald-300 hover:text-white cursor-pointer group"
            title="Descarga Mental Rápida: convierte notas o viñetas en un mapa completo (Ctrl+B)"
          >
            <Zap size={14} className="text-emerald-400 group-hover:animate-pulse" />
            <span className="hidden lg:inline">Descarga Mental</span>
          </button>
        )}

        {/* Semantic Bridges (Hidden Connections) Button */}
        {onOpenBridgesModal && (
          <button
            type="button"
            id="btn-toolbar-bridges"
            onClick={onOpenBridgesModal}
            className="flex items-center gap-1.5 bg-violet-950/40 hover:bg-violet-900/60 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors border border-violet-500/40 text-violet-300 hover:text-white cursor-pointer group"
            title="Detector de Conexiones Ocultas: halla sinergias no obvias entre ideas distantes"
          >
            <Network size={14} className="text-violet-400 group-hover:scale-110 transition-transform" />
            <span className="hidden lg:inline">Puentes</span>
          </button>
        )}

        {/* AI Whole-Map Synthesis */}
        {onOpenSynthesis && (
          <button
            type="button"
            onClick={onOpenSynthesis}
            className="flex items-center gap-1.5 bg-slate-900 hover:bg-slate-800 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors border border-indigo-500/30 text-indigo-300 hover:text-white cursor-pointer"
            title="Generar resumen ejecutivo y plan de acción de la red"
          >
            <Compass size={14} className="text-emerald-400" />
            <span className="hidden md:inline">Síntesis</span>
          </button>
        )}

        <button
          type="button"
          onClick={onOpenStatesModal}
          className="flex items-center gap-1.5 bg-slate-800 hover:bg-slate-700 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors border border-slate-700 text-slate-200 cursor-pointer"
          title="Exportar diseño JSON y administrar estados"
        >
          <Download size={14} /> <span className="hidden sm:inline">Exportar</span>
        </button>

        <button
          type="button"
          id="btn-toolbar-obsidian"
          onClick={onOpenObsidianModal || onOpenStatesModal}
          className="flex items-center gap-1.5 bg-purple-950/40 hover:bg-purple-900/60 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors border border-purple-500/40 text-purple-200 hover:text-white cursor-pointer"
          title="Exportar directamente a Obsidian (.md con frontmatter o .canvas)"
        >
          <Sparkles size={14} className="text-purple-400" />
          <span className="hidden md:inline">Obsidian</span>
        </button>

        {/* HITL Continuous Learning Engine Button */}
        {onOpenHitlModal && (
          <button
            type="button"
            id="btn-toolbar-hitl"
            onClick={onOpenHitlModal}
            className="flex items-center gap-1.5 bg-violet-950/40 hover:bg-violet-900/60 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors border border-violet-500/40 text-violet-200 hover:text-white cursor-pointer group"
            title="Motor de Auto-Mejora y Aprendizaje Continuo (HITL Loop)"
          >
            <Brain size={14} className="text-violet-400 group-hover:animate-pulse" />
            <span className="hidden md:inline">Auto-Mejora</span>
            {hitlDecisionsCount !== undefined && (
              <span className="px-1.5 py-0.2 bg-violet-500/20 text-violet-300 rounded-full text-[10px] font-mono border border-violet-400/30">
                {hitlDecisionsCount}
              </span>
            )}
          </button>
        )}

        {/* Templates Gallery & Dropdown */}
        <div className="relative group">
          <button
            type="button"
            id="btn-templates-menu"
            onClick={onOpenTemplatesModal}
            className="flex items-center gap-1.5 bg-slate-900 hover:bg-slate-800 text-slate-300 hover:text-white px-2.5 py-1.5 rounded-lg text-xs font-medium border border-slate-800 hover:border-slate-700 transition-colors cursor-pointer"
            title="Explorar plantillas y nuevos núcleos de ideas"
          >
            <LayoutTemplate size={14} className="text-indigo-400" />
            <span className="hidden md:inline">Plantillas</span>
          </button>
          <div className="absolute right-0 top-full mt-1.5 w-60 bg-slate-900 border border-slate-800 rounded-xl shadow-2xl p-1.5 hidden group-hover:block z-50 text-xs text-slate-200">
            <div className="px-2 py-1 text-[10px] uppercase font-bold text-slate-500 tracking-wider">
              Núcleos de Ideas
            </div>
            <button
              type="button"
              onClick={() => onSelectTemplate('ai-startup')}
              className="w-full text-left px-2.5 py-1.5 rounded-lg hover:bg-slate-800 transition-colors text-slate-300 hover:text-white flex items-center justify-between"
            >
              <span>🤖 Ecosistema IA</span>
              <span className="text-[10px] text-slate-500">5 nodos</span>
            </button>
            <button
              type="button"
              onClick={() => onSelectTemplate('saas-launch')}
              className="w-full text-left px-2.5 py-1.5 rounded-lg hover:bg-slate-800 transition-colors text-slate-300 hover:text-white flex items-center justify-between"
            >
              <span>🚀 Startup SaaS B2B</span>
              <span className="text-[10px] text-slate-500">5 nodos</span>
            </button>
            <button
              type="button"
              onClick={() => onSelectTemplate('design-thinking')}
              className="w-full text-left px-2.5 py-1.5 rounded-lg hover:bg-slate-800 transition-colors text-slate-300 hover:text-white flex items-center justify-between"
            >
              <span>💡 Design Thinking</span>
              <span className="text-[10px] text-slate-500">5 nodos</span>
            </button>
            <button
              type="button"
              onClick={() => onSelectTemplate('microservices')}
              className="w-full text-left px-2.5 py-1.5 rounded-lg hover:bg-slate-800 transition-colors text-slate-300 hover:text-white flex items-center justify-between"
            >
              <span>⚡ Microservicios Cloud</span>
              <span className="text-[10px] text-slate-500">5 nodos</span>
            </button>
            <button
              type="button"
              onClick={() => onSelectTemplate('research-thesis')}
              className="w-full text-left px-2.5 py-1.5 rounded-lg hover:bg-slate-800 transition-colors text-slate-300 hover:text-white flex items-center justify-between"
            >
              <span>📚 Tesis / Investigación</span>
              <span className="text-[10px] text-slate-500">5 nodos</span>
            </button>
            <button
              type="button"
              onClick={() => onSelectTemplate('growth-marketing')}
              className="w-full text-left px-2.5 py-1.5 rounded-lg hover:bg-slate-800 transition-colors text-slate-300 hover:text-white flex items-center justify-between"
            >
              <span>📈 Growth & Marketing</span>
              <span className="text-[10px] text-slate-500">5 nodos</span>
            </button>
            <button
              type="button"
              onClick={() => onSelectTemplate('blank')}
              className="w-full text-left px-2.5 py-1.5 rounded-lg hover:bg-slate-800 transition-colors text-slate-300 hover:text-white flex items-center justify-between"
            >
              <span>🌱 Núcleo Minimalista</span>
              <span className="text-[10px] text-slate-500">1 nodo</span>
            </button>
            {onOpenTemplatesModal && (
              <>
                <div className="my-1 border-t border-slate-800" />
                <button
                  type="button"
                  onClick={onOpenTemplatesModal}
                  className="w-full text-left px-2.5 py-1.5 rounded-lg hover:bg-indigo-950/40 text-indigo-400 hover:text-indigo-300 transition-colors font-medium flex items-center gap-1.5"
                >
                  <LayoutTemplate size={12} />
                  <span>Ver Galería Completa...</span>
                </button>
              </>
            )}
          </div>
        </div>

        {/* Clear All Nodes / Reset Canvas Button */}
        <button
          type="button"
          id="btn-clear-canvas"
          onClick={onOpenClearModal || onResetCanvas}
          className="flex items-center gap-1.5 bg-rose-950/20 hover:bg-rose-950/50 text-rose-300 hover:text-rose-200 border border-rose-900/40 hover:border-rose-700/60 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors cursor-pointer"
          title="Borrar todos los nodos o reiniciar el lienzo"
        >
          <Trash2 size={13} className="text-rose-400" />
          <span className="hidden sm:inline">Limpiar</span>
        </button>

        {/* Quick Keyboard Shortcuts Guide */}
        {onOpenShortcuts && (
          <button
            type="button"
            id="btn-toolbar-shortcuts"
            onClick={onOpenShortcuts}
            className="flex items-center gap-1.5 bg-slate-900 hover:bg-slate-800 text-slate-300 hover:text-white px-2.5 py-1.5 rounded-lg text-xs font-medium border border-slate-800 hover:border-slate-700 transition-colors cursor-pointer group"
            title="Ver todos los atajos de teclado permitidos (Tecla ? o Ctrl+/)"
          >
            <Keyboard size={14} className="text-indigo-400 group-hover:text-indigo-300" />
            <span className="hidden lg:inline">Atajos</span>
            <kbd className="px-1.5 py-0.2 bg-slate-800 text-indigo-300 rounded text-[10px] font-mono border border-slate-700/80">
              ?
            </kbd>
          </button>
        )}

        {/* Bring Your Own Key (BYOK) Button */}
        {onOpenApiKeyModal && (
          <button
            type="button"
            id="btn-toolbar-apikey"
            onClick={onOpenApiKeyModal}
            className="flex items-center gap-1.5 bg-slate-900 hover:bg-slate-800 text-slate-300 hover:text-white px-2.5 py-1.5 rounded-lg text-xs font-medium border border-slate-800 hover:border-slate-700 transition-colors cursor-pointer group"
            title={hasCustomApiKey ? "API Key personal activa (usando tu cuota de Gemini)" : "Configurar API Key propia (opcional)"}
          >
            <Key size={14} className={hasCustomApiKey ? "text-emerald-400" : "text-slate-400 group-hover:text-slate-200"} />
            <span className="hidden xl:inline">API Key</span>
            <span className={`w-1.5 h-1.5 rounded-full ${hasCustomApiKey ? 'bg-emerald-400' : 'bg-slate-600'}`} />
          </button>
        )}

        {/* User Account / Profile Button */}
        <div className="flex items-center border-l border-slate-800 pl-1.5 md:pl-2">
          <button
            type="button"
            id="btn-user-profile"
            onClick={onOpenAuthModal}
            className="flex items-center gap-2 group text-left transition-opacity hover:opacity-90"
            title="Autenticación y perfil de usuario"
          >
            {currentUser?.avatar ? (
              <img
                src={currentUser.avatar}
                alt={currentUser.name}
                referrerPolicy="no-referrer"
                className="w-7 h-7 rounded-full object-cover border border-slate-700 ring-1 ring-indigo-500/40"
              />
            ) : (
              <div className="w-7 h-7 rounded-full bg-gradient-to-tr from-indigo-500 to-purple-500 border border-slate-700 flex items-center justify-center text-[10px] font-bold text-white shadow-sm">
                {currentUser?.name
                  ? currentUser.name
                      .split(' ')
                      .map((p) => p[0])
                      .join('')
                      .slice(0, 2)
                      .toUpperCase()
                  : 'AR'}
              </div>
            )}
          </button>
        </div>
      </div>
    </header>
  );
};
