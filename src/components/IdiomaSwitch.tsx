import { IDIOMAS } from '../i18n/idioma';
import { useIdioma } from '../i18n/useIdioma';

/**
 * Switch de idioma (ES / EN).
 *
 * Cambia la interfaz **y** la voz: la transcripción y la voz de salida siguen el mismo idioma, así que
 * hablarle en inglés a la app la hace responder en inglés con voz nativa, no con acento prestado.
 */
export function IdiomaSwitch() {
  const { idioma, t, cambiar } = useIdioma();

  return (
    <div
      className="flex items-center gap-0.5 rounded-lg border border-slate-700/60 bg-slate-900/60 p-0.5"
      title={t('app.idioma.ayuda')}
      role="group"
      aria-label={t('app.idioma')}
    >
      {IDIOMAS.map((i) => (
        <button
          key={i.id}
          onClick={() => void cambiar(i.id)}
          title={`${t('app.idioma')}: ${i.etiqueta}`}
          aria-pressed={idioma === i.id}
          className={`px-2 py-1 text-[11px] rounded-md transition-colors ${
            idioma === i.id
              ? 'bg-cyan-500/20 text-cyan-200 font-semibold'
              : 'text-slate-400 hover:text-slate-200'
          }`}
        >
          {i.corto}
        </button>
      ))}
    </div>
  );
}
