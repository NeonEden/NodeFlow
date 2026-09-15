import { useCallback, useEffect, useState } from 'react';

import { idiomaActual, setIdioma as guardarIdioma, suscribir, type Idioma } from './idioma';
import { traducir, type Clave } from './textos';

/**
 * Hook de idioma: `const { t, idioma, cambiar } = useIdioma()`.
 *
 * Cualquier componente que lo use se vuelve a renderizar solo cuando el idioma cambia, así que el
 * switch es instantáneo y no hace falta recargar la ventana.
 */
export function useIdioma() {
  const [idioma, setIdiomaEstado] = useState<Idioma>(idiomaActual());

  useEffect(() => {
    return suscribir(() => setIdiomaEstado(idiomaActual()));
  }, []);

  const t = useCallback(
    (clave: Clave, vars?: Record<string, string | number>) => traducir(idioma, clave, vars),
    [idioma],
  );

  const cambiar = useCallback(async (nuevo: Idioma) => {
    await guardarIdioma(nuevo);
    setIdiomaEstado(nuevo);
  }, []);

  return { idioma, t, cambiar };
}
