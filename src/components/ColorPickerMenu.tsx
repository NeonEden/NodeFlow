import React, { useState } from 'react';
import { Palette, Check, Sparkles, Sliders, ChevronDown } from 'lucide-react';
import { EdgeAppearance, ConnectionCurve } from '../types';

interface ColorPickerMenuProps {
  appearance: EdgeAppearance;
  onChange: (appearance: EdgeAppearance) => void;
  onApplyToSelectedEdges?: () => void;
  selectedEdgeCount?: number;
}

export const PRESET_COLORS = [
  { name: 'Índigo Neón', hex: '#6366f1' },
  { name: 'Esmeralda', hex: '#10b981' },
  { name: 'Cian Cósmico', hex: '#06b6d4' },
  { name: 'Rosa Vibrante', hex: '#f43f5e' },
  { name: 'Ámbar Cálido', hex: '#f59e0b' },
  { name: 'Violeta Astral', hex: '#a855f7' },
  { name: 'Azul Eléctrico', hex: '#3b82f6' },
  { name: 'Lima Fresco', hex: '#84cc16' },
  { name: 'Gris Neutro', hex: '#94a3b8' },
  { name: 'Blanco Puro', hex: '#f8fafc' },
];

export const ColorPickerMenu: React.FC<ColorPickerMenuProps> = ({
  appearance,
  onChange,
  onApplyToSelectedEdges,
  selectedEdgeCount = 0,
}) => {
  const [isOpen, setIsOpen] = useState(false);

  const handleColorSelect = (hex: string) => {
    onChange({ ...appearance, color: hex });
  };

  const handleWidthChange = (strokeWidth: number) => {
    onChange({ ...appearance, strokeWidth });
  };

  const handleTypeChange = (type: ConnectionCurve) => {
    onChange({ ...appearance, type });
  };

  const handleAnimatedToggle = () => {
    onChange({ ...appearance, animated: !appearance.animated });
  };

  return (
    <div className="relative">
      {/* Trigger button */}
      <button
        type="button"
        id="btn-connection-styles"
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-1.5 bg-slate-800 hover:bg-slate-700 border border-slate-700 text-slate-200 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-all shadow-sm"
        title="Personalizar color y estilo de conexiones"
      >
        <span
          className="w-3.5 h-3.5 rounded-full ring-2 ring-slate-900 shadow-sm inline-block"
          style={{ backgroundColor: appearance.color }}
        />
        <Palette size={14} className="text-slate-300" />
        <span className="hidden sm:inline">Conexiones</span>
        <ChevronDown size={13} className={`text-slate-400 transition-transform ${isOpen ? 'rotate-180' : ''}`} />
      </button>

      {/* Popover overlay */}
      {isOpen && (
        <>
          <div
            className="fixed inset-0 z-40"
            onClick={() => setIsOpen(false)}
          />
          <div className="absolute right-0 top-full mt-2 w-72 bg-slate-900 border border-slate-700/80 rounded-xl shadow-2xl p-4 z-50 text-slate-200 backdrop-blur-xl animate-in fade-in zoom-in-95 duration-150">
            <div className="flex items-center justify-between pb-3 border-b border-slate-800">
              <div className="flex items-center gap-1.5 text-xs font-semibold text-slate-100">
                <Sliders size={14} className="text-indigo-400" />
                <span>Estilo de Conexiones</span>
              </div>
              <span className="text-[10px] uppercase font-mono px-1.5 py-0.5 rounded bg-slate-800 text-slate-400">
                {appearance.type}
              </span>
            </div>

            {/* Live Preview */}
            <div className="my-3 p-3 bg-slate-950/80 rounded-lg border border-slate-800 flex items-center justify-between">
              <span className="text-[11px] text-slate-400">Vista previa:</span>
              <div className="w-36 h-6 flex items-center justify-center relative overflow-hidden">
                <svg className="w-full h-full" viewBox="0 0 140 24">
                  <line
                    x1="10"
                    y1="12"
                    x2="130"
                    y2="12"
                    stroke={appearance.color}
                    strokeWidth={appearance.strokeWidth}
                    strokeDasharray={appearance.animated ? '5,5' : 'none'}
                    className={appearance.animated ? 'animate-pulse' : ''}
                  />
                  <circle cx="10" cy="12" r="3" fill={appearance.color} />
                  <circle cx="130" cy="12" r="3" fill={appearance.color} />
                </svg>
              </div>
            </div>

            {/* Color Swatches Grid */}
            <div className="space-y-1.5 mb-3">
              <label className="text-[11px] font-medium text-slate-400 block">
                Paleta de Color
              </label>
              <div className="grid grid-cols-5 gap-2">
                {PRESET_COLORS.map((item) => (
                  <button
                    key={item.hex}
                    type="button"
                    onClick={() => handleColorSelect(item.hex)}
                    title={item.name}
                    className={`w-7 h-7 rounded-lg flex items-center justify-center transition-transform hover:scale-110 relative ${
                      appearance.color.toLowerCase() === item.hex.toLowerCase()
                        ? 'ring-2 ring-white scale-105'
                        : 'border border-slate-700/50'
                    }`}
                    style={{ backgroundColor: item.hex }}
                  >
                    {appearance.color.toLowerCase() === item.hex.toLowerCase() && (
                      <Check size={13} className={item.hex === '#f8fafc' ? 'text-black' : 'text-white'} />
                    )}
                  </button>
                ))}
              </div>

              {/* Custom Hex input */}
              <div className="flex items-center gap-2 mt-2 pt-2 border-t border-slate-800/80">
                <span className="text-[11px] text-slate-400">Personalizado:</span>
                <input
                  type="color"
                  value={appearance.color}
                  onChange={(e) => handleColorSelect(e.target.value)}
                  className="w-6 h-6 rounded cursor-pointer bg-transparent border-0"
                />
                <input
                  type="text"
                  value={appearance.color}
                  onChange={(e) => handleColorSelect(e.target.value)}
                  className="flex-1 bg-slate-800 text-xs px-2 py-1 rounded border border-slate-700 font-mono text-slate-200 uppercase"
                  maxLength={7}
                />
              </div>
            </div>

            {/* Curve Style & Stroke Width */}
            <div className="space-y-3 pt-3 border-t border-slate-800 text-xs">
              <div>
                <label className="text-[11px] font-medium text-slate-400 block mb-1.5">
                  Tipo de Curva
                </label>
                <div className="grid grid-cols-3 gap-1 bg-slate-950 p-1 rounded-lg border border-slate-800">
                  <button
                    type="button"
                    onClick={() => handleTypeChange('smoothstep')}
                    className={`py-1 text-[11px] rounded transition-colors ${
                      appearance.type === 'smoothstep'
                        ? 'bg-indigo-600 text-white font-medium'
                        : 'text-slate-400 hover:text-white'
                    }`}
                  >
                    Escalón
                  </button>
                  <button
                    type="button"
                    onClick={() => handleTypeChange('default')}
                    className={`py-1 text-[11px] rounded transition-colors ${
                      appearance.type === 'default'
                        ? 'bg-indigo-600 text-white font-medium'
                        : 'text-slate-400 hover:text-white'
                    }`}
                  >
                    Curva
                  </button>
                  <button
                    type="button"
                    onClick={() => handleTypeChange('straight')}
                    className={`py-1 text-[11px] rounded transition-colors ${
                      appearance.type === 'straight'
                        ? 'bg-indigo-600 text-white font-medium'
                        : 'text-slate-400 hover:text-white'
                    }`}
                  >
                    Recta
                  </button>
                </div>
              </div>

              {/* Stroke Width & Animation */}
              <div className="flex items-center justify-between gap-2">
                <div>
                  <label className="text-[11px] font-medium text-slate-400 block mb-1">
                    Grosor
                  </label>
                  <div className="flex gap-1 bg-slate-950 p-1 rounded-lg border border-slate-800">
                    {[1.5, 2.5, 3.5].map((w) => (
                      <button
                        key={w}
                        type="button"
                        onClick={() => handleWidthChange(w)}
                        className={`px-2 py-0.5 text-[11px] rounded transition-colors ${
                          appearance.strokeWidth === w
                            ? 'bg-slate-700 text-white font-medium'
                            : 'text-slate-400 hover:text-white'
                        }`}
                      >
                        {w}x
                      </button>
                    ))}
                  </div>
                </div>

                <div>
                  <label className="text-[11px] font-medium text-slate-400 block mb-1">
                    Animado
                  </label>
                  <button
                    type="button"
                    onClick={handleAnimatedToggle}
                    className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs transition-colors ${
                      appearance.animated
                        ? 'bg-indigo-950/80 border-indigo-500 text-indigo-300'
                        : 'bg-slate-950 border-slate-800 text-slate-400'
                    }`}
                  >
                    <Sparkles size={12} className={appearance.animated ? 'text-indigo-400' : 'text-slate-500'} />
                    <span>{appearance.animated ? 'Activo' : 'Fijo'}</span>
                  </button>
                </div>
              </div>

              {/* Selected edge applicator */}
              {selectedEdgeCount > 0 && onApplyToSelectedEdges && (
                <button
                  type="button"
                  onClick={onApplyToSelectedEdges}
                  className="w-full mt-2 py-1.5 px-3 bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-medium rounded-lg transition-colors shadow"
                >
                  Aplicar a {selectedEdgeCount} {selectedEdgeCount === 1 ? 'conexión seleccionada' : 'conexiones seleccionadas'}
                </button>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
};
