import { useEffect, useState } from 'react';

/**
 * Motor de inferencia en uso, compartido por toda la UI.
 *
 * Los mensajes de progreso nombran el motor real ("granite3.3:2b está analizando…") en vez de un
 * proveedor fijo: desde que el motor es una elección del usuario, decir "Gemini" siempre era mentira
 * la mitad de las veces.
 */

type Escucha = (etiqueta: string) => void;

let etiqueta = 'la IA';
const escuchas = new Set<Escucha>();

export function fijarMotorActual(nueva: string) {
  etiqueta = nueva && nueva.trim() ? nueva.trim() : 'la IA';
  escuchas.forEach((f) => f(etiqueta));
}

export function motorActual() {
  return etiqueta;
}

/** Hook para los textos que están en JSX (se re-renderizan al cambiar el motor). */
export function useMotorActual() {
  const [v, setV] = useState(etiqueta);
  useEffect(() => {
    escuchas.add(setV);
    return () => {
      escuchas.delete(setV);
    };
  }, []);
  return v;
}
