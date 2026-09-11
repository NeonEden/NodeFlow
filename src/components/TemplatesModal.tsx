import React, { useState } from 'react';
import {
  X,
  Sparkles,
  Bot,
  Rocket,
  Compass,
  Server,
  BookOpen,
  TrendingUp,
  PlusCircle,
  ArrowRight,
  Layers,
  Check,
  RefreshCw,
} from 'lucide-react';
import { INITIAL_TEMPLATES, TemplateDefinition } from '../data/templates';

interface TemplatesModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSelectTemplate: (templateId: string) => void;
  currentTemplateId?: string;
  currentNodeCount?: number;
  templates?: TemplateDefinition[];
  onRefreshTemplates?: () => Promise<void> | void;
  isRefreshingTemplates?: boolean;
}

export const TemplatesModal: React.FC<TemplatesModalProps> = ({
  isOpen,
  onClose,
  onSelectTemplate,
  currentTemplateId,
  currentNodeCount = 0,
  templates = INITIAL_TEMPLATES,
  onRefreshTemplates,
  isRefreshingTemplates = false,
}) => {
  const [selectedCategory, setSelectedCategory] = useState<string>('Todos');

  if (!isOpen) return null;

  const categories = ['Todos', 'Tecnología', 'Negocios', 'Diseño', 'Investigación', 'Esencial'];

  const filteredTemplates = (templates || INITIAL_TEMPLATES).filter((tmpl) => {
    if (selectedCategory === 'Todos') return true;
    return tmpl.category === selectedCategory;
  });

  const getTemplateIcon = (iconName: string) => {
    switch (iconName) {
      case 'Bot':
        return <Bot size={20} className="text-indigo-400" />;
      case 'Rocket':
        return <Rocket size={20} className="text-emerald-400" />;
      case 'Compass':
        return <Compass size={20} className="text-amber-400" />;
      case 'Server':
        return <Server size={20} className="text-cyan-400" />;
      case 'BookOpen':
        return <BookOpen size={20} className="text-purple-400" />;
      case 'TrendingUp':
        return <TrendingUp size={20} className="text-pink-400" />;
      case 'PlusCircle':
      default:
        return <PlusCircle size={20} className="text-indigo-400" />;
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-3 md:p-6 bg-slate-950/80 backdrop-blur-md animate-in fade-in duration-200">
      <div
        id="templates-gallery-modal"
        className="bg-slate-900 border border-slate-800 rounded-2xl w-full max-w-4xl max-h-[90vh] shadow-2xl flex flex-col overflow-hidden"
      >
        {/* Header */}
        <div className="p-5 border-b border-slate-800 flex items-center justify-between bg-slate-950/40 shrink-0">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-indigo-600/10 border border-indigo-500/20 flex items-center justify-center text-indigo-400">
              <Sparkles size={20} />
            </div>
            <div>
              <h2 className="text-base md:text-lg font-bold text-white flex items-center gap-2">
                Plantillas & Nuevos Núcleos de Ideas
              </h2>
              <p className="text-xs text-slate-400">
                Selecciona una estructura base conceptual optimizada para comenzar a crear y expandir con IA
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-slate-400 hover:text-white p-1.5 hover:bg-slate-800 rounded-lg transition-colors cursor-pointer"
          >
            <X size={18} />
          </button>
        </div>

        {/* Category Filter Pills & Refresh Action */}
        <div className="px-5 py-3 border-b border-slate-800/80 bg-slate-900/50 flex items-center justify-between gap-3 overflow-x-auto shrink-0 scrollbar-none">
          <div className="flex items-center gap-2">
            {categories.map((cat) => (
              <button
                key={cat}
                type="button"
                onClick={() => setSelectedCategory(cat)}
                className={`px-3 py-1.5 rounded-xl text-xs font-medium whitespace-nowrap transition-colors cursor-pointer ${
                  selectedCategory === cat
                    ? 'bg-indigo-600 text-white shadow-sm shadow-indigo-600/30'
                    : 'bg-slate-800/70 hover:bg-slate-800 text-slate-400 hover:text-slate-200 border border-slate-700/60'
                }`}
              >
                {cat}
              </button>
            ))}
          </div>

          {onRefreshTemplates && (
            <button
              type="button"
              id="btn-refresh-idea-cores"
              onClick={() => onRefreshTemplates()}
              disabled={isRefreshingTemplates}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-xs font-semibold bg-indigo-950/60 hover:bg-indigo-900 border border-indigo-500/40 hover:border-indigo-400 text-indigo-300 hover:text-white transition-all shadow-sm shadow-indigo-950 cursor-pointer disabled:opacity-50 shrink-0 group ml-auto"
              title="Generar nuevos núcleos de ideas manteniendo los 5 tópicos (Tecnología, Negocios, Diseño, Investigación, Esencial)"
            >
              <RefreshCw
                size={13}
                className={`text-indigo-400 group-hover:text-indigo-200 transition-transform ${
                  isRefreshingTemplates ? 'animate-spin' : 'group-hover:rotate-180 duration-500'
                }`}
              />
              <span className="whitespace-nowrap">
                {isRefreshingTemplates ? 'Generando Ideas...' : 'Refrescar Núcleos (Ideas Frescas)'}
              </span>
              <Sparkles size={12} className="text-amber-400 animate-pulse shrink-0" />
            </button>
          )}
        </div>

        {/* Templates Grid */}
        <div className="p-5 overflow-y-auto space-y-3.5 flex-1">
          {currentNodeCount > 0 && (
            <div className="bg-amber-950/30 border border-amber-500/20 rounded-xl p-3 flex items-center justify-between text-xs text-amber-200/90 mb-2">
              <span>
                💡 Al cargar una plantilla se reemplazará el lienzo actual ({currentNodeCount} nodos). Puedes recuperar el trabajo anterior con <strong>Ctrl + Z</strong> o guardarlo en <strong>Exportar &gt; Guardar Estado</strong>.
              </span>
            </div>
          )}

          <div className="grid grid-cols-1 md:grid-cols-2 gap-3.5">
            {filteredTemplates.map((template: TemplateDefinition) => {
              const isCurrent = currentTemplateId === template.id;

              return (
                <div
                  key={template.id}
                  id={`template-card-${template.id}`}
                  className="bg-slate-950/60 hover:bg-slate-950/90 border border-slate-800 hover:border-indigo-500/40 rounded-xl p-4 transition-all duration-200 flex flex-col justify-between group relative overflow-hidden"
                >
                  <div
                    className="absolute top-0 left-0 bottom-0 w-1 opacity-80"
                    style={{ backgroundColor: template.appearance.color }}
                  />

                  <div>
                    <div className="flex items-start justify-between gap-3 mb-2.5 pl-2">
                      <div className="flex items-center gap-2.5">
                        <div className="w-8 h-8 rounded-lg bg-slate-900 border border-slate-800 flex items-center justify-center shrink-0">
                          {getTemplateIcon(template.iconName)}
                        </div>
                        <div>
                          <h3 className="text-sm font-semibold text-white group-hover:text-indigo-300 transition-colors">
                            {template.title}
                          </h3>
                          <span className="text-[10px] uppercase font-bold tracking-wider text-slate-500">
                            {template.category}
                          </span>
                        </div>
                      </div>

                      <div className="flex items-center gap-1.5 text-[11px] text-slate-400 bg-slate-900/80 px-2 py-0.5 rounded-md border border-slate-800 shrink-0">
                        <Layers size={11} className="text-indigo-400" />
                        <span>{template.nodeCount} {template.nodeCount === 1 ? 'nodo' : 'nodos'}</span>
                      </div>
                    </div>

                    <p className="text-xs text-slate-300/90 leading-relaxed mb-4 pl-2">
                      {template.description}
                    </p>
                  </div>

                  <div className="pt-3 border-t border-slate-800/80 flex items-center justify-between pl-2">
                    <div className="flex items-center gap-1.5">
                      <span
                        className="w-2 h-2 rounded-full"
                        style={{ backgroundColor: template.appearance.color }}
                      />
                      <span className="text-[11px] text-slate-500 font-mono">
                        {template.appearance.type}
                      </span>
                    </div>

                    <button
                      type="button"
                      id={`btn-use-template-${template.id}`}
                      onClick={() => {
                        onSelectTemplate(template.id);
                        onClose();
                      }}
                      className="flex items-center gap-1.5 bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-medium px-3 py-1.5 rounded-lg transition-colors shadow-sm shadow-indigo-600/20 cursor-pointer"
                    >
                      <span>Usar Plantilla</span>
                      <ArrowRight size={13} />
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-slate-800 bg-slate-950/40 flex items-center justify-between text-xs text-slate-400 shrink-0">
          <span>También puedes crear tus propios núcleos desde el botón <strong>Nodo Raíz</strong> o duplicar nodos existentes.</span>
          <button
            type="button"
            onClick={onClose}
            className="px-3.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-200 transition-colors cursor-pointer"
          >
            Cerrar
          </button>
        </div>
      </div>
    </div>
  );
};
