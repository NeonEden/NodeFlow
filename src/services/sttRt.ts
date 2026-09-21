/**
 * Fábrica de clientes de voz: el backend dice QUÉ motor usar, acá se instancia el cliente correcto.
 *
 * Por qué existe: el panel de voz no debería saber si atrás hay Speechmatics o AssemblyAI. El motor es
 * un dato del catálogo (`/api/voz/proveedores`) y el protocolo decide el cliente. Agregar un motor
 * nuevo = una rama acá + su emisión de token en el backend (`stt::abrir_sesion`).
 *
 * Regla de diseño: los dos clientes exponen el MISMO contrato (start/stop/texto + eventos), así que
 * el panel no cambia cuando cambia el motor.
 */

import { SpeechmaticsRt, type EventosVoz } from './speechmaticsRt';
import { AssemblyAiRt } from './assemblyaiRt';

export type { EstadoVoz, EventosVoz } from './speechmaticsRt';

/** Lo que el panel de voz necesita de cualquier cliente, sin importar el motor. */
export interface ClienteStt {
  start(): Promise<void>;
  stop(): Promise<string>;
  readonly texto: string;
  /**
   * Cierra el turno **sin cerrar la sesión** (AssemblyAI: `ForceEndpoint`), para que el turno siguiente
   * vaya sobre la misma conexión. Opcional a propósito: un motor que no lo implemente sigue funcionando
   * —el panel corta y vuelve a abrir, como antes—. Medido 20/09/2026: abrir una sesión por turno sumaba
   * ~2 s de handshake y se facturaba por tiempo de conexión, no por audio.
   */
  cerrarTurno?(): Promise<string>;
  /** Deja de enviar audio sin cerrar la conexión (el motor no se escucha a sí mismo). */
  pausar?(): void;
  /** Vuelve a enviar audio sobre la MISMA sesión. */
  reanudar?(): void;
  /** ¿La sesión sigue abierta y lista para el próximo turno? */
  readonly viva?: boolean;
}

/** Sesión que devuelve `GET /api/voz/jwt` (agnóstica del motor). */
export interface SesionVoz {
  success: boolean;
  /** `speechmatics` | `assemblyai` */
  proveedor: string;
  /** `speechmatics-v2` | `assemblyai-v3` */
  protocolo: string;
  etiqueta?: string;
  /** Token temporal. `jwt` se mantiene por compatibilidad con el panel viejo. */
  token?: string;
  jwt?: string;
  url: string;
  idioma: string;
  modelo: string;
  expira_en_s: number;
  codec?: string;
  /** Se muestra cuando el idioma pedido no se puede cumplir con este motor. */
  aviso?: string;
}

/** Motores con cliente implementado en el frontend. */
export const PROTOCOLOS_SOPORTADOS = ['speechmatics-v2', 'assemblyai-v3'] as const;

/**
 * Crea el cliente del motor que indica la sesión.
 * Tira un error explícito si el protocolo no tiene cliente: mejor fallar claro que abrir el motor
 * equivocado y quedarse escuchando el vacío.
 */
export function crearClienteStt(sesion: SesionVoz, ev: EventosVoz): ClienteStt {
  const token = sesion.token || sesion.jwt || '';
  switch (sesion.protocolo) {
    case 'assemblyai-v3':
      return new AssemblyAiRt(
        { url: sesion.url, token, idioma: sesion.idioma, modelo: sesion.modelo },
        ev
      );
    case 'speechmatics-v2':
      return new SpeechmaticsRt(
        { url: sesion.url, jwt: token, idioma: sesion.idioma, modelo: sesion.modelo },
        ev
      );
    default:
      throw new Error(
        `El motor «${sesion.proveedor}» usa un protocolo sin cliente: ${sesion.protocolo || 'sin declarar'}.`
      );
  }
}
