import React, { useEffect, useRef, useState } from 'react';
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
  RefreshCw,
  ChevronDown,
  MoreHorizontal,
} from 'lucide-react';
import { MotorSelector } from './MotorSelector';
import { IdiomaSwitch } from './IdiomaSwitch';
import { ColorPickerMenu } from './ColorPickerMenu';
import { EdgeAppearance, UserProfile } from '../types';
import { TemplateDefinition } from '../data/templates';
import { useIdioma } from '../i18n/useIdioma';

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
  templates?: TemplateDefinition[];
  onRefreshTemplates?: () => void;
  isRefreshingTemplates?: boolean;
  saveStatus?: 'saved' | 'saving' | 'unsaved';
  isAiProcessing?: boolean;
  isSidebarOpen?: boolean;
  onToggleSidebar?: () => void;
  onAutoLayout?: () => void;
  onOpenSynthesis?: () => void;
  onOpenObsidianModal?: () => void;
  onOpenJsonModal?: () => void;
  onOpenHitlModal?: () => void;
  hitlDecisionsCount?: number;
  onOpenShortcuts?: () => void;
  onOpenApiKeyModal?: () => void;
  hasCustomApiKey?: boolean;
  onOpenBrainDump?: () => void;
  onOpenBridgesModal?: () => void;
  bridgesCount?: number;
  searchQuery?: string;
  onSearchChange?: (q: string) => void;
}

interface MenuItemProps {
  id?: string;
  icon: React.ComponentType<{ size?: number; className?: string }>;
  label: string;
  hint?: string;
  onClick: () => void;
}

/** Ítem de menú: icono, etiqueta y una pista corta a la derecha. */
const MenuItemP: React.FC<MenuItemProps> = ({ id, icon: Icono, label, hint, onClick }) => (
  <button
    type="button"
    id={id}
    onClick={onClick}
    className="w-full text-left px-2.5 py-1.5 rounded-lg hover:bg-slate-800 text-slate-300 hover:text-white flex items-center gap-2 transition-colors cursor-pointer"
  >
    <Icono size={13} className="text-slate-400 shrink-0" />
    <span className="truncate">{label}</span>
    {hint && <span className="ml-auto text-[10px] font-mono text-slate-500 shrink-0">{hint}</span>}
  </button>
);

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
  templates,
  onRefreshTemplates,
  isRefreshingTemplates = false,
  saveStatus = 'saved',
  isAiProcessing = false,
  isSidebarOpen = true,
  onToggleSidebar,
  onAutoLayout,
  onOpenSynthesis,
  onOpenObsidianModal,
  onOpenJsonModal,
  onOpenHitlModal,
  hitlDecisionsCount,
  onOpenShortcuts,
  onOpenApiKeyModal,
  hasCustomApiKey = false,
  onOpenBrainDump,
  onOpenBridgesModal,
  bridgesCount = 0,
  searchQuery = '',
  onSearchChange,
}) => {
  const { t } = useIdioma();
  // Un solo menú abierto por vez; se cierra al hacer click afuera o con Escape.
  const [menuAbierto, setMenuAbierto] = useState<'salida' | 'mas' | null>(null);
  const refSalida = useRef<HTMLDivElement>(null);
  const refMas = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!menuAbierto) return;
    const alClick = (e: MouseEvent) => {
      const t = e.target as Node;
      if (refSalida.current?.contains(t) || refMas.current?.contains(t)) return;
      setMenuAbierto(null);
    };
    const alTecla = (e: KeyboardEvent) => e.key === 'Escape' && setMenuAbierto(null);
    document.addEventListener('mousedown', alClick);
    document.addEventListener('keydown', alTecla);
    return () => {
      document.removeEventListener('mousedown', alClick);
      document.removeEventListener('keydown', alTecla);
    };
  }, [menuAbierto]);

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
            NODE<span className="text-indigo-500">FLOW</span>
          </h1>
        </div>

        {/* Estado de guardado: la barra lo muestra sólo en pantallas muy anchas; el sidebar
            ya informa "Local Activo" y el último guardado en disco, así que acá es redundante */}
        <span className="hidden 2xl:flex items-center gap-1.5 bg-slate-900/60 border border-slate-800 rounded-lg px-2 py-1 text-[10px] text-slate-400 font-mono">
          <span className={`w-1.5 h-1.5 rounded-full ${saveStatus === 'saving' ? 'bg-amber-400 animate-pulse' : saveStatus === 'unsaved' ? 'bg-rose-400' : 'bg-emerald-400'}`} />
          {saveStatus === 'saving' ? 'Guardando' : saveStatus === 'unsaved' ? 'Sin guardar' : 'Persistido'}
        </span>

        {/* Quick Search Bar */}
        {onSearchChange && (
          <div className="relative hidden md:flex items-center ml-1">
            <Search size={13} className="absolute left-2.5 text-slate-500" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => onSearchChange(e.target.value)}
              placeholder={t('toolbar.buscar')}
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
            title={t('toolbar.deshacer')}
            className="p-1.5 hover:bg-slate-800 rounded text-slate-400 hover:text-slate-200 disabled:opacity-30 disabled:hover:bg-transparent transition-colors cursor-pointer"
          >
            <Undo2 size={15} />
          </button>
          <button
            type="button"
            id="btn-redo"
            onClick={onRedo}
            disabled={!canRedo}
            title={t('toolbar.rehacer')}
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
            <span className="hidden 2xl:inline">{t('toolbar.organizar')}</span>
          </button>
        )}

        {/* Quick Color Dots */}
        <div className="hidden 2xl:flex items-center gap-1.5 bg-slate-900/60 p-1 rounded-lg border border-slate-800">
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

      {/* Right Controls — un solo lugar por acción, agrupado y sin repetir el sidebar */}
      <div className="flex items-center gap-1 md:gap-1.5">
        {/* Fase 12 — motor global de inferencia */}
        <MotorSelector />
        {/* Idioma de la interfaz y de la voz: el switch vive acá porque es una preferencia global. */}
        <IdiomaSwitch />

        <button
          type="button"
          onClick={onHybridize}
          disabled={isAiProcessing}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors shadow-lg cursor-pointer ${
            selectedNodesCount >= 2
              ? 'bg-indigo-600 hover:bg-indigo-500 shadow-indigo-500/30 text-white ring-2 ring-indigo-400'
              : 'bg-indigo-600 hover:bg-indigo-500 shadow-indigo-500/20 text-white'
          }`}
          title={t('toolbar.descubrir')}
        >
          <Combine size={14} className={isAiProcessing ? 'animate-spin' : ''} />
          <span className="hidden xl:inline">{t('toolbar.hibridar')}</span>
          {selectedNodesCount >= 2 && (
            <span className="bg-indigo-900 text-indigo-200 text-[10px] px-1.5 py-0.2 rounded-full font-mono">
              {selectedNodesCount}
            </span>
          )}
        </button>

        {onOpenBrainDump && (
          <button
            type="button"
            id="btn-toolbar-braindump"
            onClick={onOpenBrainDump}
            className="flex items-center gap-1.5 bg-emerald-950/40 hover:bg-emerald-900/60 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors border border-emerald-500/40 text-emerald-300 hover:text-white cursor-pointer group"
            title="Descarga Mental Rápida: convierte notas o viñetas en un mapa completo (Ctrl+B)"
          >
            <Zap size={14} className="text-emerald-400 group-hover:animate-pulse" />
            <span className="hidden 2xl:inline">{t('toolbar.descargaCorta')}</span>
          </button>
        )}

        {/* Salida: exportar tiene un solo punto de entrada */}
        <div className="relative" ref={refSalida}>
          <button
            type="button"
            id="btn-toolbar-exportar"
            onClick={() => setMenuAbierto(menuAbierto === 'salida' ? null : 'salida')}
            className="flex items-center gap-1.5 bg-purple-950/40 hover:bg-purple-900/60 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors border border-purple-500/40 text-purple-200 hover:text-white cursor-pointer"
            title={t('toolbar.exportar.ayuda')}
          >
            <Download size={14} className="text-purple-400" />
            <span className="hidden xl:inline">{t('toolbar.exportar')}</span>
            <ChevronDown size={12} className="opacity-70" />
          </button>
          {menuAbierto === 'salida' && (
            <div className="absolute right-0 top-full mt-1.5 w-60 bg-slate-900 border border-slate-800 rounded-xl shadow-2xl p-1.5 z-50 text-xs">
              <MenuItemP
                id="btn-toolbar-obsidian"
                icon={Sparkles}
                label="Exportar a Obsidian"
                hint=".md / .canvas"
                onClick={() => { setMenuAbierto(null); (onOpenObsidianModal || onOpenStatesModal)(); }}
              />
              <MenuItemP
                icon={Download}
                label="Descargar diseño JSON"
                hint=".json"
                onClick={() => { setMenuAbierto(null); (onOpenJsonModal || onOpenStatesModal)(); }}
              />
            </div>
          )}
        </div>

        {/* Todo lo demás, ordenado por tema en un menú */}
        <div className="relative" ref={refMas}>
          <button
            type="button"
            id="btn-toolbar-mas"
            onClick={() => setMenuAbierto(menuAbierto === 'mas' ? null : 'mas')}
            className="flex items-center gap-1 bg-slate-900 hover:bg-slate-800 border border-slate-800 text-slate-300 hover:text-white px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors cursor-pointer"
            title={t('toolbar.mas.ayuda')}
          >
            <MoreHorizontal size={15} />
            <span className="hidden xl:inline">{t('toolbar.mas')}</span>
          </button>
          {menuAbierto === 'mas' && (
            <div className="absolute right-0 top-full mt-1.5 w-64 bg-slate-900 border border-slate-800 rounded-xl shadow-2xl p-1.5 z-50 text-xs">
              <div className="px-2 py-1 text-[10px] uppercase font-bold text-slate-500 tracking-wider">{t('toolbar.inteligencia')}</div>
              {onOpenBridgesModal && (
                <MenuItemP id="btn-toolbar-bridges" icon={Network} label="Puentes"
                  hint={bridgesCount > 0 ? `${bridgesCount} sinergias` : 'conexiones ocultas'}
                  onClick={() => { setMenuAbierto(null); onOpenBridgesModal(); }} />
              )}
              {onOpenSynthesis && (
                <MenuItemP icon={Compass} label="Síntesis del mapa" hint="resumen y plan"
                  onClick={() => { setMenuAbierto(null); onOpenSynthesis(); }} />
              )}

              <div className="my-1 border-t border-slate-800" />
              <div className="px-2 py-1 text-[10px] uppercase font-bold text-slate-500 tracking-wider">{t('toolbar.nucleos')}</div>
              {onOpenTemplatesModal && (
                <MenuItemP id="btn-templates-menu" icon={LayoutTemplate} label="Plantillas y galería"
                  onClick={() => { setMenuAbierto(null); onOpenTemplatesModal(); }} />
              )}
              {onRefreshTemplates && (
                <MenuItemP id="btn-toolbar-refresh-templates" icon={RefreshCw}
                  label={isRefreshingTemplates ? 'Generando núcleos...' : 'Refrescar núcleos'}
                  onClick={() => { setMenuAbierto(null); onRefreshTemplates(); }} />
              )}

              <div className="my-1 border-t border-slate-800" />
              <div className="px-2 py-1 text-[10px] uppercase font-bold text-slate-500 tracking-wider">{t('toolbar.sistema')}</div>
              {onOpenHitlModal && (
                <MenuItemP id="btn-toolbar-hitl" icon={Brain} label="Auto-Mejora (HITL)"
                  hint={hitlDecisionsCount !== undefined ? String(hitlDecisionsCount) : undefined}
                  onClick={() => { setMenuAbierto(null); onOpenHitlModal(); }} />
              )}
              {onOpenApiKeyModal && (
                <MenuItemP id="btn-toolbar-apikey" icon={Key} label="API Key propia"
                  hint={hasCustomApiKey ? 'activa' : 'sin configurar'}
                  onClick={() => { setMenuAbierto(null); onOpenApiKeyModal(); }} />
              )}
            </div>
          )}
        </div>

        {/* User Account / Profile Button */}
        <div className="flex items-center border-l border-slate-800 pl-1.5 md:pl-2">
          <button
            type="button"
            id="btn-user-profile"
            onClick={onOpenAuthModal}
            className="flex items-center gap-2 group text-left transition-opacity hover:opacity-90"
            title={t('toolbar.perfil.ayuda')}
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
