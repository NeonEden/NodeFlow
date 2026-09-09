import React, { useState, useEffect } from 'react';
import { IdeaNodeData, IdeaMaturityLevel, MATURITY_CONFIGS } from '../types';
import { X, Tag, Palette, Check, Trash2, Plus } from 'lucide-react';
import { PRESET_COLORS } from './ColorPickerMenu';

interface NodeEditModalProps {
  nodeData: IdeaNodeData | null;
  isOpen: boolean;
  onClose: () => void;
  onSave: (updated: IdeaNodeData) => void;
  onDelete?: (id: string) => void;
}

const CATEGORY_PRESETS = [
  'IDEA',
  'ESTRATEGIA',
  'TECNOLOGÍA',
  'INVESTIGACIÓN',
  'PRODUCTO',
  'HIPÓTESIS',
  'MERCADO',
  'DRAFT',
];

export const NodeEditModal: React.FC<NodeEditModalProps> = ({
  nodeData,
  isOpen,
  onClose,
  onSave,
  onDelete,
}) => {
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [label, setLabel] = useState('IDEA');
  const [tags, setTags] = useState<string[]>([]);
  const [newTag, setNewTag] = useState('');
  const [colorAccent, setColorAccent] = useState('#6366f1');
  const [maturity, setMaturity] = useState<IdeaMaturityLevel>(1);

  useEffect(() => {
    if (nodeData) {
      setTitle(nodeData.title || '');
      setDescription(nodeData.description || '');
      setLabel(nodeData.label || 'IDEA');
      setTags(nodeData.tags || []);
      setColorAccent(nodeData.colorAccent || '#6366f1');
      setMaturity(nodeData.maturity || (nodeData.isRoot ? 3 : 1));
    }
  }, [nodeData]);

  if (!isOpen || !nodeData) return null;

  const handleAddTag = (e: React.KeyboardEvent | React.MouseEvent) => {
    if ('key' in e && e.key !== 'Enter') return;
    e.preventDefault();
    const clean = newTag.trim().replace(/^#/, '');
    if (clean && !tags.includes(clean)) {
      setTags([...tags, clean]);
      setNewTag('');
    }
  };

  const handleRemoveTag = (tagToRemove: string) => {
    setTags(tags.filter((t) => t !== tagToRemove));
  };

  const handleSave = (e: React.FormEvent) => {
    e.preventDefault();
    onSave({
      ...nodeData,
      title: title.trim() || 'Sin Título',
      description: description.trim(),
      label: label.trim() || 'IDEA',
      tags,
      colorAccent,
      maturity,
    });
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-sm animate-in fade-in duration-150">
      <div
        className="w-full max-w-lg bg-slate-900 border border-slate-700/80 rounded-2xl shadow-2xl p-6 text-slate-100 relative"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between pb-4 border-b border-slate-800">
          <div className="flex items-center gap-2">
            <span
              className="w-3 h-3 rounded-full"
              style={{ backgroundColor: colorAccent }}
            />
            <h3 className="text-base font-semibold text-white">Editar Nodo de Pensamiento</h3>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-1.5 text-slate-400 hover:text-white hover:bg-slate-800 rounded-lg transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        <form onSubmit={handleSave} className="space-y-4 mt-4 text-xs">
          {/* Category / Label Presets */}
          <div>
            <label className="block text-slate-400 font-medium mb-1.5">
              Categoría / Tipo de Nodo
            </label>
            <div className="flex flex-wrap gap-1.5">
              {CATEGORY_PRESETS.map((cat) => (
                <button
                  key={cat}
                  type="button"
                  onClick={() => setLabel(cat)}
                  className={`px-2.5 py-1 rounded-md text-[11px] font-semibold tracking-wider transition-all ${
                    label === cat
                      ? 'bg-indigo-600 text-white shadow-sm'
                      : 'bg-slate-800 text-slate-300 hover:bg-slate-700'
                  }`}
                >
                  {cat}
                </button>
              ))}
            </div>
          </div>

          {/* Title input */}
          <div>
            <label className="block text-slate-400 font-medium mb-1">Título de la Idea</label>
            <input
              type="text"
              required
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Ej. Red Neuronal Descentralizada"
              className="w-full px-3 py-2 bg-slate-950 border border-slate-700 rounded-xl text-white font-medium focus:outline-none focus:border-indigo-500"
            />
          </div>

          {/* Description input */}
          <div>
            <label className="block text-slate-400 font-medium mb-1">Descripción y Detalles</label>
            <textarea
              rows={3}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Describe el concepto, alcances técnicos o consideraciones clave..."
              className="w-full px-3 py-2 bg-slate-950 border border-slate-700 rounded-xl text-slate-200 leading-relaxed focus:outline-none focus:border-indigo-500 resize-none"
            />
          </div>

          {/* Tags */}
          <div>
            <label className="block text-slate-400 font-medium mb-1 flex items-center gap-1.5">
              <Tag size={13} className="text-slate-400" />
              Etiquetas
            </label>
            <div className="flex gap-2 mb-2">
              <input
                type="text"
                value={newTag}
                onChange={(e) => setNewTag(e.target.value)}
                onKeyDown={handleAddTag}
                placeholder="Añadir etiqueta (ej. Web3, IA, Edge)"
                className="flex-1 px-3 py-1.5 bg-slate-950 border border-slate-700 rounded-xl text-white focus:outline-none focus:border-indigo-500"
              />
              <button
                type="button"
                onClick={handleAddTag}
                className="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-xl flex items-center gap-1 font-medium"
              >
                <Plus size={14} /> Añadir
              </button>
            </div>
            {tags.length > 0 && (
              <div className="flex flex-wrap gap-1.5">
                {tags.map((t) => (
                  <span
                    key={t}
                    className="inline-flex items-center gap-1 bg-slate-800 border border-slate-700 text-slate-300 px-2 py-0.5 rounded-lg text-[11px]"
                  >
                    #{t}
                    <button
                      type="button"
                      onClick={() => handleRemoveTag(t)}
                      className="text-slate-400 hover:text-rose-400"
                    >
                      ×
                    </button>
                  </span>
                ))}
              </div>
            )}
          </div>

          {/* Calificador de Madurez */}
          <div>
            <label className="block text-slate-400 font-medium mb-1.5 flex items-center justify-between">
              <span>Calificador de Madurez</span>
              <span className={`text-[11px] font-bold ${MATURITY_CONFIGS[maturity].textColor}`}>
                {MATURITY_CONFIGS[maturity].icon} {MATURITY_CONFIGS[maturity].label} (Nivel {maturity}/4)
              </span>
            </label>
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
              {([1, 2, 3, 4] as IdeaMaturityLevel[]).map((lvl) => {
                const cfg = MATURITY_CONFIGS[lvl];
                const isSelected = maturity === lvl;
                return (
                  <button
                    key={lvl}
                    type="button"
                    onClick={() => setMaturity(lvl)}
                    className={`flex flex-col text-left p-2.5 rounded-xl border transition-all cursor-pointer ${
                      isSelected
                        ? `${cfg.badgeBg} ${cfg.borderColor} border-2 shadow-md ring-1 ring-white/10`
                        : 'bg-slate-950/60 border-slate-800 hover:border-slate-700 hover:bg-slate-800/50'
                    }`}
                  >
                    <div className="flex items-center justify-between gap-1 mb-1">
                      <span className="text-base">{cfg.icon}</span>
                      <span className="text-[10px] font-mono text-slate-500">Nivel {lvl}</span>
                    </div>
                    <span className={`text-xs font-bold leading-tight ${isSelected ? cfg.textColor : 'text-slate-200'}`}>
                      {cfg.label}
                    </span>
                    <span className="text-[10px] text-slate-400 mt-1 leading-tight line-clamp-2">
                      {cfg.desc}
                    </span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Node Accent Color */}
          <div>
            <label className="block text-slate-400 font-medium mb-1.5 flex items-center gap-1.5">
              <Palette size={13} className="text-slate-400" />
              Color de Distinción
            </label>
            <div className="flex items-center gap-2">
              {PRESET_COLORS.slice(0, 7).map((color) => (
                <button
                  key={color.hex}
                  type="button"
                  onClick={() => setColorAccent(color.hex)}
                  className={`w-6 h-6 rounded-full transition-transform ${
                    colorAccent.toLowerCase() === color.hex.toLowerCase()
                      ? 'ring-2 ring-white scale-110'
                      : 'opacity-70 hover:opacity-100'
                  }`}
                  style={{ backgroundColor: color.hex }}
                />
              ))}
              <input
                type="color"
                value={colorAccent}
                onChange={(e) => setColorAccent(e.target.value)}
                className="w-7 h-7 rounded cursor-pointer bg-transparent border-0 ml-2"
              />
            </div>
          </div>

          {/* Action Footer */}
          <div className="flex items-center justify-between pt-4 border-t border-slate-800 mt-6">
            {onDelete ? (
              <button
                type="button"
                onClick={() => {
                  onDelete(nodeData.id);
                  onClose();
                }}
                className="flex items-center gap-1 text-rose-400 hover:text-rose-300 hover:bg-rose-950/40 px-3 py-1.5 rounded-lg transition-colors"
              >
                <Trash2 size={14} /> Eliminar
              </button>
            ) : <div />}

            <div className="flex gap-2">
              <button
                type="button"
                onClick={onClose}
                className="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-xl transition-colors font-medium"
              >
                Cancelar
              </button>
              <button
                type="submit"
                className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-xl shadow-lg shadow-indigo-600/30 flex items-center gap-1.5 font-semibold transition-all"
              >
                <Check size={14} /> Guardar Cambios
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
};
