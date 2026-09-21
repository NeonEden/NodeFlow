import React, { useCallback, useEffect, useRef, useState } from 'react';
import { X, Mic, Square, Loader2, Sparkles, Check, AlertTriangle, Wand2, Target, Layers, MessageSquarePlus, Link2, Quote, Gauge, Volume2, VolumeX, PenLine, CornerDownRight, MessageCircleQuestion } from 'lucide-react';
import { type EstadoVoz } from '../services/speechmaticsRt';
import { crearClienteStt, type ClienteStt } from '../services/sttRt';
import { apiUrl } from '../services/apiBase';
import { trazaVoz } from '../services/trazaVoz';
import { getVozEstado, getVozJwt, pedirPlanVoz, describirComando, decir, hablarConElSistema, VozEstado, PlanVoz, VozComando } from '../services/vozService';
import { useIdioma } from '../i18n/useIdioma';
import { esAfirmativo, planEsConsulta } from '../utils/voz';

interface VozPanelProps {
  isOpen: boolean;
  onClose: () => void;
  /** Aplica el plan aprobado. Devuelve cuántos nodos creó y a cuántos afectó. */
  onAplicar: (plan: PlanVoz) => Promise<{
    creados: number;
    afectados: number;
    /** Consultas respondidas: no hubo cambios en el lienzo (acción de sólo lectura). */
    consultas?: string[];
    /** Si el plan pidió el motor profundo: se disparó y la respuesta llega después, por su cuenta. */
    delegando?: boolean;
  } | null>;
  /** Qué va a pasar, en números, para mostrarlo ANTES de aplicar. */
  onPrevisualizar: (plan: PlanVoz) => string;
  /** Aplica los comandos de una fase de investigación (el nodo crece mientras investiga). */
  onAplicarComandos: (comandos: VozComando[], que: string) => Promise<void>;
  tituloNodo: (id: string) => string;
  /**
   * Modo conversación: la app arranca el turno con una pregunta y encadena ida y vuelta con el
   * micrófono abierto por turnos. Devuelve la primera frase de la app.
   */
  onInicioConversacion: () => string;
  /**
   * Un turno hablado en modo conversación. Devuelve qué decir a continuación, si la conversación
   * termina, y si el pedido no era del guion (hay que pedirle el plan al motor).
   */
  onTurnoConversacion: (texto: string) => Promise<{ decir: string; fin?: boolean; alMotor?: boolean }>;
  /** La primera pregunta abierta del lienzo: se lee en voz alta y se espera la respuesta hablada. */
  preguntaAbierta: () => { id: string; titulo: string } | null;
  /** Guarda una respuesta dictada: nace el nodo RESPUESTA enlazado y la pregunta se cierra. */
  onResponder: (preguntaId: string, texto: string) => void;
  /**
   * Pedido de afuera —el atajo global, Ctrl+Shift+Space—: «empezar» abre el turno y «cortar» lo cierra.
   * Se reacciona al contador `n`, no al objeto: dos pulsaciones seguidas se ejecutan las dos.
   */
  pedidoExterno?: { accion: 'empezar' | 'cortar'; n: number } | null;
}

/** Id corto de sesión de STT: sólo sirve para correlacionar y contar las trazas del log. */
const nuevoIdSesion = () => Math.random().toString(36).slice(2, 8);

const ICONO: Record<VozComando['accion'], React.ReactNode> = {
  crear: <MessageSquarePlus size={12} />,
  enlazar: <Link2 size={12} />,
  enfocar: <Target size={12} />,
  condensar: <Layers size={12} />,
  criticar: <Quote size={12} />,
  delegar: <Sparkles size={12} />,
  actualizar: <PenLine size={12} />,
  responder: <CornerDownRight size={12} />,
  aceptar: <Check size={12} />,
  descartar: <X size={12} />,
  consultar: <MessageCircleQuestion size={12} />,
};

/** «¿qué quedó abierto?» — pedido de estado que se resuelve con regla local, sin motor (0 tokens). */
// El «sí» hablado vive en `utils/voz` (`esAfirmativo`): una sola definición para el guion y el panel.
// El regex local que estaba acá se fue cuando el panel pasó a usar la función compartida.

const PEDIDO_DE_RETOMAR =
  /(qu[eé]\s+(qued[oó]|ten[eé]s|hay)\s+(abierto|pendiente))|(preguntas?\s+abiertas?)|(^retom)|(le[eé]me la pregunta)/i;

const EJEMPLOS = [
  'Dictá ideas nuevas: «el orquestador de voz se integra con NodeFlow y con el mapa conceptual por nodos»',
  'O comandá: «limpiá el lienzo y dejá sólo lo que se conecta con el orquestador de voz»',
  'O pedí lo que necesita herramientas: «averiguá si el sensor SHT31 sigue fabricándose y decime alternativas»',
];

/**
 * Panel de Voz (Speechmatics). Hablás, la transcripción aparece en vivo y al cortar el motor
 * propone un PLAN de operaciones sobre el lienzo — que se aprueba antes de aplicarse.
 */
export const VozPanel: React.FC<VozPanelProps> = ({ isOpen, onClose, onAplicar, onPrevisualizar, onAplicarComandos, tituloNodo, preguntaAbierta, onResponder, onInicioConversacion, onTurnoConversacion, pedidoExterno }) => {
  // Textos del panel en el idioma activo. La voz (entrada y salida) sigue el mismo idioma desde el
  // backend, así que acá sólo se traduce la interfaz.
  const { t } = useIdioma();
  const [servicio, setServicio] = useState<VozEstado | null>(null);
  const [estado, setEstado] = useState<EstadoVoz>('inactivo');
  const [detalleEstado, setDetalleEstado] = useState('');
  const [parcial, setParcial] = useState('');
  const [texto, setTexto] = useState('');
  const [plan, setPlan] = useState<PlanVoz | null>(null);
  const [metricas, setMetricas] = useState<{ asrSeg: number; ms: number; modelo: string; costo: number; cache: string } | null>(null);
  const [error, setError] = useState('');
  const [pensando, setPensando] = useState(false);
  const [aplicando, setAplicando] = useState(false);
  const [resultado, setResultado] = useState('');
  // Cuando la app te leyó una pregunta, lo próximo que digas es su respuesta (no un plan nuevo).
  const [modoRespuesta, setModoRespuesta] = useState<{ id: string; titulo: string } | null>(null);
  // Modo conversación: turnos encadenados. La app pregunta, escucha, actúa, vuelve a preguntar.
  const [continuo, setContinuo] = useState(false);
  // Quién habla: Kokoro local o la voz del sistema (fallback). Es DATO, se declara.
  const [motorVoz, setMotorVoz] = useState<'kokoro' | 'sistema'>('kokoro');
  // La voz local es un paquete descargable: acá vive su estado y su progreso.
  const [bajando, setBajando] = useState(false);
  const [motorLocal, setMotorLocal] = useState<{
    instalada: boolean;
    corriendo: boolean;
    en_curso: boolean;
    tamano_descarga: string;
    progreso?: { fase: string; bajado: number; total: number; error?: string | null } | null;
  } | null>(null);
  const cargarMotorLocal = useCallback(async () => {
    try {
      const r = await (await fetch(apiUrl('/api/voz/motor/estado'))).json();
      setMotorLocal(r.motor);
      setBajando(!!r.motor?.en_curso);
    } catch {
      /* sin backend no hay estado que mostrar */
    }
  }, []);
  useEffect(() => {
    if (isOpen) void cargarMotorLocal();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen]);
  useEffect(() => {
    if (!isOpen || !bajando) return;
    const t = window.setInterval(() => void cargarMotorLocal(), 2500);
    return () => window.clearInterval(t);
  }, [isOpen, bajando, cargarMotorLocal]);
  const instalarMotorLocal = async () => {
    setBajando(true);
    try {
      await fetch(apiUrl('/api/voz/motor/instalar'), { method: 'POST' });
    } catch {
      /* el estado lo dirá */
    }
    await cargarMotorLocal();
  };
  const [pasoConv, setPasoConv] = useState('idea');
  // Plan esperando la aprobación hablada («¿lo aplico?» → «dale»).
  const [planPendiente, setPlanPendiente] = useState(false);
  const continuoRef = useRef(false);
  continuoRef.current = continuo;
  const planPendienteRef = useRef(false);
  planPendienteRef.current = planPendiente;
  const cortarRef = useRef<() => void>(() => {});
  // Último texto escuchado (parcial o final): con esto el modo conversación sabe cuándo te callaste.
  const fragRef = useRef('');
  // Lo último que DIJO la app, y cuántos turnos seguidos llegaron como eco del micrófono. Con
  // parlantes (no auriculares) el motor de transcripción se escucha a sí mismo: sin esto, la
  // conversación se mordía la cola y repetía la misma pregunta.
  const dichoRef = useRef('');
  const ecosSeguidosRef = useRef(0);
  const [delegado, setDelegado] = useState<{ pedido: string; salida: string; ms: number; ok?: boolean } | null>(null);
  const [investigando, setInvestigando] = useState(false);
  const [fases, setFases] = useState<{ fase: string; titulo: string; emoji: string; que: string }[]>([]);
  const aplicadas = useRef(0);
  const [silencio, setSilencio] = useState<boolean>(() => {
    try {
      return localStorage.getItem('nodeflow_voz_silencio') === '1';
    } catch {
      return false;
    }
  });
  const [hablando, setHablando] = useState(false);
  /** Espejo del estado: `hablando` re-renderiza, el ref es el que consultan los bucles async. */
  const hablandoRef = useRef(false);
  /**
   * ¿El guion ya dijo su frase en este turno? Entonces el motor **no** vuelve a hablar: eran dos voces
   * seguidas y sonaba a bot confundido (medido 20/09/2026 en el modo conversación).
   */
  const guionHabloRef = useRef(false);
  /** El audio que está sonando (para poder interrumpirlo) y cómo resolver la espera de `hablar()`. */
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const finAudioRef = useRef<(() => void) | null>(null);
  // Referencia viva del mute: el sondeo no se reinicia cada vez que se toca el botón.
  const silencioRef = useRef(silencio);
  useEffect(() => {
    silencioRef.current = silencio;
  }, [silencio]);
  const rtRef = useRef<ClienteStt | null>(null);
  /**
   * Identidad de la sesión y del turno: es lo que hace **contable** la traza del log
   * (`grep -c "voz(ui).turno.cerrado"`). Sin esto el diagnóstico vuelve a ser deducción: medido el
   * 20/09/2026, 10 pulsaciones del atajo emitieron 13 sesiones de STT y no había ni una línea del
   * webview que dijera por qué.
   */
  const sesionRef = useRef('');
  const sesionInicioRef = useRef(0);
  const turnosServidosRef = useRef(0);
  const turnoRef = useRef(0);
  const t0PedidoRef = useRef(0);
  const t0ListoRef = useRef(0);
  const primerParcialRef = useRef(false);
  /** Último turno cerrado: el cierre real de la sesión a los 90 s de inactividad se mide desde acá. */
  const ultimaActividadRef = useRef(0);
  const inicioRef = useRef(0);

  const consultarEstado = useCallback(async () => {
    try {
      setServicio(await getVozEstado());
    } catch (e: any) {
      setError(e?.message || 'No pude consultar el servicio de voz.');
    }
  }, []);

  useEffect(() => {
    if (isOpen) void consultarEstado();
  }, [isOpen, consultarEstado]);

  // Al cerrar el panel, cortamos cualquier captura en curso: no dejamos el micrófono abierto.
  useEffect(() => {
    if (!isOpen && rtRef.current) {
      cerrarSesion('panel_cerrado');
      setEstado('inactivo');
    }
  }, [isOpen]);

  const alternarSilencio = () => {
    setSilencio((s) => {
      try {
        localStorage.setItem('nodeflow_voz_silencio', s ? '0' : '1');
      } catch {
        /* almacenamiento restringido */
      }
      return !s;
    });
  };

  /**
   * Lo que se dice de una investigación: las primeras frases, no el informe entero. Una respuesta
   * larga leída completa es insoportable; el texto queda en pantalla para leerlo con calma.
   */
  const fraseParaDecir = (texto: string): string => {
    const limpio = texto.replace(/\s+/g, ' ').trim();
    if (limpio.length <= 240) return limpio;
    const recorte = limpio.slice(0, 240);
    const punto = Math.max(recorte.lastIndexOf('. '), recorte.lastIndexOf('? '), recorte.lastIndexOf('! '));
    return punto > 80 ? recorte.slice(0, punto + 1) : `${recorte.trim()}…`;
  };

  /**
   * ¿Lo que llegó es la propia voz de la app rebotando en el micrófono?
   *
   * Compara por palabras (sin acentos ni signos) contra lo último que dijo la app: si el dictado usa
   * las mismas palabras, es el eco. Un «sí», un «dale» o un «no» no llegan a tres palabras y por eso
   * **nunca** se descartan como eco: las respuestas cortas son las que más importan.
   */
  const esEco = (dictado: string, dicho: string): boolean => {
    const palabras = (t: string) =>
      t
        .toLowerCase()
        .normalize('NFD')
        .replace(/[\u0300-\u036f]/g, '')
        .replace(/[^a-z0-9ñ ]+/g, ' ')
        .split(/\s+/)
        .filter((p) => p.length > 2);
    const del = palabras(dictado);
    const suyas = palabras(dicho);
    if (del.length < 4 || suyas.length < 4) return false;
    // 1) Un fragmento LITERAL de lo que dijo la app, en orden: es eco aunque venga mezclado con tu
    //    respuesta (medido 20/09: «quiero explorar la idea de comandos por voz. Sí, por favor. Explora
    //    ramificaciones…» era una sola transcripción con las dos voces). Sólo se mira en turnos largos
    //    —≥8 palabras—: en uno corto sería un falso positivo.
    if (del.length >= 8) {
      for (let i = 0; i + 4 <= del.length; i++) {
        for (let j = 0; j + 4 <= suyas.length; j++) {
          if (del.slice(i, i + 4).join(' ') === suyas.slice(j, j + 4).join(' ')) return true;
        }
      }
    }
    // 2) La red de siempre: si el dictado es casi todo lo que dijo la app. Un «sí», un «dale» o un
    //    «no» no llegan a cuatro palabras y por eso **nunca** se descartan como eco.
    return del.filter((p) => suyas.includes(p)).length / del.length >= 0.7;
  };

  /**
   * ¿Este pedazo de transcripción suena a la propia voz de la app?
   *
   * Más laxo que `esEco` a propósito: el barge-in tiene que decidir con dos o tres palabras recién
   * llegadas, no con el turno entero.
   */
  const pareceMia = (t: string): boolean => {
    const w = (s: string) =>
      s
        .toLowerCase()
        .normalize('NFD')
        .replace(/[\u0300-\u036f]/g, '')
        .replace(/[^a-z0-9ñ ]+/g, ' ')
        .split(/\s+/)
        .filter((p) => p.length > 2);
    const del = w(t);
    const suyas = new Set(w(dichoRef.current));
    if (!del.length || !suyas.size) return false;
    return del.filter((p) => suyas.has(p)).length / del.length >= 0.5;
  };

  /**
   * Barge-in: el usuario tomó la palabra mientras la app hablaba, así que la app se calla ya.
   *
   * Resolver la espera de `hablar()` es la mitad del trabajo: si no, el turno seguiría colgado hasta el
   * timeout del audio y la conversación quedaría sorda varios segundos justo cuando el usuario habló.
   */
  const interrumpirVoz = () => {
    const el = audioRef.current;
    try {
      if (el && !el.paused) el.pause();
      window.speechSynthesis?.cancel();
    } catch {
      /* ya estaba detenida */
    }
    const fin = finAudioRef.current;
    finAudioRef.current = null;
    hablandoRef.current = false;
    setHablando(false);
    fin?.();
  };

  /** Habla sólo si el backend lo autorizó (regla de voz selectiva) y no está en silencio. */
  const hablar = async (texto: string) => {
    if (!texto.trim()) return;
    dichoRef.current = texto;
    hablandoRef.current = true;
    setHablando(true);
    try {
      // Primero la voz local (Kokoro). Si esa PC no la tiene —una instalación limpia nunca la
      // tiene—, habla la voz del sistema: la app no queda muda en ninguna máquina.
      const audio = await decir(texto).catch(() => null);
      if (audio) {
        if (motorVoz !== 'kokoro') setMotorVoz('kokoro');
        const url = URL.createObjectURL(audio);
        const el = new Audio(url);
        // La app queda «hablando» hasta el final del audio… salvo que el usuario la interrumpa: para eso
        // el elemento y la resolución de la espera quedan a mano de `interrumpirVoz()`.
        audioRef.current = el;
        await new Promise<void>((listo) => {
          let cerrado = false;
          const fin = () => {
            if (cerrado) return;
            cerrado = true;
            URL.revokeObjectURL(url);
            audioRef.current = null;
            finAudioRef.current = null;
            listo();
          };
          el.onended = fin;
          el.onerror = fin;
          void el.play().catch(fin);
          // Red de seguridad: si `ended` no llega (audio raro, salida de audio cambiada en el medio),
          // no nos quedamos sordos para siempre.
          const ms = Number.isFinite(el.duration) ? el.duration * 1000 + 1000 : 0;
          window.setTimeout(fin, Math.max(2500, ms));
        });
      } else {
        if (motorVoz !== 'sistema') setMotorVoz('sistema');
        await hablarConElSistema(texto, servicio?.idioma || 'es');
      }
    } catch (e: any) {
      // Que la voz falle no rompe nada: el lienzo ya cambió y el texto está en pantalla.
      setError((previo) => previo || `Voz: ${e?.message || 'no pude reproducir'}`);
    } finally {
      hablandoRef.current = false;
      setHablando(false);
    }
  };

  // Mientras el motor profundo investiga, el panel consulta cada 5 s y muestra lo que llegue.
  useEffect(() => {
    if (!investigando) return;
    let vivo = true;
    const consultar = async () => {
      try {
        const d = await (await fetch(apiUrl('/api/ai/investigar'))).json();
        if (!vivo) return;
        const inv = d?.investigacion;
        const pasos: any[] = Array.isArray(inv?.pasos) ? inv.pasos : [];
        setFases(pasos.map((x) => ({ fase: x.fase, titulo: x.titulo, emoji: x.emoji, que: x.que })));
        // Las fases las aplica `App.tsx`, que vive siempre montado: si el aplicador estuviera acá,
        // la investigación sólo llegaría al lienzo con esta ventana abierta (y se aplicaría dos veces).
        if (inv?.terminado) {
          setInvestigando(false);
          if (inv.salida) {
            setDelegado({ pedido: inv.pedido || '', salida: inv.salida, ms: 0, ok: inv.ok !== false });
            if (silencioRef.current === false) void hablar(fraseParaDecir(inv.salida));
          } else if (inv.ok === false) {
            setError('La investigación no llegó a buen puerto esta vez.');
          }
        }
      } catch {
        /* si el backend no contesta, el panel sigue intentando */
      }
    };
    const t = setInterval(consultar, 4000);
    void consultar();
    return () => {
      vivo = false;
      clearInterval(t);
    };
  }, [investigando, onAplicarComandos]);

  // Fin de turno por inactividad: en conversación, quedarse callado cierra el turno (1,8 s con texto).
  useEffect(() => {
    if (!continuo || estado !== 'escuchando') return;
    if (!fragRef.current.trim()) return;
    const t = window.setTimeout(() => void cortarRef.current(), 1800);
    return () => window.clearTimeout(t);
  }, [continuo, estado, parcial, texto]);

  /**
   * Cierra la sesión de voz de verdad (fin de conversación, eco, inactividad, cambio de modo).
   *
   * El `motivo` viaja a la traza: sin él, «se cerró la sesión» no distingue si lo pidió el usuario, el
   * eco del micrófono o el reloj de inactividad — y el diagnóstico vuelve a ser una deducción.
   */
  const cerrarSesion = (motivo = 'salida') => {
    const rt = rtRef.current;
    rtRef.current = null;
    if (!rt) return;
    trazaVoz('sesion.cerrada', {
      sesion: sesionRef.current,
      motivo,
      ms_vida: Math.round(performance.now() - sesionInicioRef.current),
      turnos: turnosServidosRef.current,
    });
    sesionRef.current = '';
    turnosServidosRef.current = 0;
    void rt.stop();
  };

  /**
   * Espera a que la app deje de hablar, **con tope**: con un tope corto (barge-in) alcanza para que no
   * entre el arranque del TTS; el resto de la voz lo filtran `pareceMia` y `esEco`. Con tope largo se usa
   * cuando la voz tiene que terminar sí o sí antes de abrir el micrófono.
   */
  const esperarVoz = async (topeMs = 9000) => {
    const t0 = performance.now();
    while (hablandoRef.current && performance.now() - t0 < topeMs) {
      await new Promise((r) => window.setTimeout(r, 120));
    }
    // Un respiro para que la última sílaba se apague en la sala antes de abrir el micrófono.
    await new Promise((r) => window.setTimeout(r, 250));
  };

  /**
   * Vuelve a escuchar cuando el turno se cierra por conversación.
   *
   * Medido 20/09/2026: abrir una sesión nueva por turno daba **11 sesiones en 90 s** — handshake en cada
   * una, y el proveedor factura el tiempo de conexión abierto. Si la sesión sigue viva, el turno
   * siguiente va sobre la misma (el motor la cierra sólo si se la termina).
   */
  const seguirEscuchando = async () => {
    // Con barge-in se espera sólo el ARRANQUE de la voz (600 ms), no su final: el micrófono tiene que
    // quedar escuchando para poder interrumpirla. Lo que evita que la conversación se muerda la cola no
    // es la espera sino los filtros: `pareceMia` en los parciales y `esEco` en el turno.
    await esperarVoz(600);
    if (!continuoRef.current) return;
    const rt = rtRef.current;
    if (rt?.viva && rt.reanudar) {
      fragRef.current = '';
      setError('');
      setParcial('');
      setTexto('');
      inicioRef.current = performance.now();
      rt.reanudar();
      return;
    }
    void empezar('conv');
  };

  /** El atajo pidió cortar mientras `empezar()` todavía estaba abriendo la sesión. */
  const cancelarRef = useRef(false);

  const empezar = async (origen: 'ui' | 'atajo' | 'conv' = 'ui') => {
    // Cada pedido abre un turno numerado: es lo que después se cuenta en el log.
    turnoRef.current += 1;
    t0PedidoRef.current = performance.now();
    primerParcialRef.current = false;
    trazaVoz('pedido', { turno: turnoRef.current, origen, sesion_viva: Boolean(rtRef.current?.viva) });
    // Una sesión a la vez: sin este guard, cada pulsación del atajo apilaba otra sesión de streaming
    // (medido 20/09: 7 sesiones en 25 s). Si ya hay una viva, se reanuda en vez de apilar.
    if (rtRef.current) {
      if (rtRef.current.viva && rtRef.current.reanudar) {
        fragRef.current = '';
        setParcial('');
        setTexto('');
        inicioRef.current = performance.now();
        t0ListoRef.current = performance.now();
        ultimaActividadRef.current = performance.now();
        rtRef.current.reanudar();
        // `reanudar` no paga handshake: este es el número que prueba que el corte ya no reabre la sesión.
        trazaVoz('listo', { turno: turnoRef.current, modo: 'reanudar', ms_desde_pedido: Math.round(performance.now() - t0PedidoRef.current), sesion: sesionRef.current });
        return;
      }
      // El cliente quedó creado pero la conexión ya no está viva (se cayó, o el motor la cerró). Si no se
      // descarta, la pulsación muere en el vacío: el guard de arriba sale sin abrir nada y el micrófono
      // nunca arranca (caso borde medido el 20/09/2026).
      cerrarSesion('caida');
    }
    cancelarRef.current = false;
    setError('');
    setResultado('');
    setPlan(null);
    setMetricas(null);
    setTexto('');
    setParcial('');
    try {
      const sesion = await getVozJwt();
      // El motor lo decide el backend: acá sólo se instancia el cliente del protocolo que devuelva.
      // El aviso se fija antes para que no lo pise el primer `onEstado` (habla antes de escuchar).
      if (sesion.aviso) setDetalleEstado(sesion.aviso);
      // Identidad de la sesión ANTES de abrir: así todo lo que pasa durante el handshake queda
      // correlacionado con la misma sesión en el log.
      sesionRef.current = nuevoIdSesion();
      sesionInicioRef.current = performance.now();
      turnosServidosRef.current = 0;
      const rt = crearClienteStt(sesion, {
        onEstado: (e, d) => {
          setEstado(e);
          if (d) setDetalleEstado(d);
        },
        onParcial: (t) => {
          fragRef.current = t;
          setParcial(t);
          // Primera marca de que el audio ENTRA: si el turno empieza a llegar tarde, se ve acá y no en la
          // transcripción final (que es donde el síntoma aparecía como «no me toma las palabras»).
          if (!primerParcialRef.current && t.trim()) {
            primerParcialRef.current = true;
            trazaVoz('parcial.primero', {
              turno: turnoRef.current,
              ms_desde_listo: Math.round(performance.now() - t0ListoRef.current),
              chars: t.trim().length,
            });
          }
          // Barge-in (20/09/2026): si el usuario habla encima, la app se calla en el acto. Tres palabras
          // y que no suenen a la propia voz: con parlantes la app se oiría a sí misma y se cortaría sola.
          const partes = t.trim().split(/\s+/).filter(Boolean);
          if (hablandoRef.current && partes.length >= 3 && !pareceMia(t)) interrumpirVoz();
        },
        onFinal: (t) => {
          fragRef.current = t;
          setTexto(t);
        },
        onError: (m) => {
          trazaVoz('error', { fase: 'motor', mensaje: m });
          setError(m);
        },
      });
      rtRef.current = rt;
      inicioRef.current = performance.now();
      await rt.start();
      t0ListoRef.current = performance.now();
      ultimaActividadRef.current = performance.now();
      // El número que decide si el arranque sigue costando ~1 s (abrir) o decenas de ms (reanudar).
      trazaVoz('listo', {
        turno: turnoRef.current,
        modo: 'abrir',
        ms_desde_pedido: Math.round(performance.now() - t0PedidoRef.current),
        sesion: sesionRef.current,
        motor: sesion.proveedor || '',
      });
      // El atajo ya se soltó mientras abríamos: el turno se cancela acá, sin dejar el micrófono abierto.
      if (cancelarRef.current) {
        cancelarRef.current = false;
        rtRef.current = null;
        sesionRef.current = '';
        void rt.stop();
        trazaVoz('turno.cancelado', { turno: turnoRef.current, motivo: 'soltado_durante_el_arranque' });
        setEstado('inactivo');
        setDetalleEstado('');
      }
    } catch (e: any) {
      trazaVoz('error', { fase: 'abrir', mensaje: e?.message || 'desconocido' });
      setError(e?.message || 'No pude empezar a escuchar.');
      setEstado('error');
    }
  };

  const cortar = async () => {
    const rt = rtRef.current;
    if (!rt) {
      // Todavía no hay cliente: `empezar()` está abriendo la sesión (pide el token por HTTP) y el atajo ya
      // se soltó. Sin esto el corte caía en el vacío y el micrófono quedaba escuchando para siempre
      // (medido 20/09 a las 23:29: 7 sesiones emitidas y ningún corte). Queda marcado para que `empezar()`
      // cierre la sesión apenas termine de abrirla.
      cancelarRef.current = true;
      trazaVoz('turno.cancelado', { turno: turnoRef.current, motivo: 'soltado_durante_el_arranque' });
      setEstado('inactivo');
      setDetalleEstado('');
      return;
    }
    guionHabloRef.current = false; // turno nuevo: el guion todavía no dijo nada
    const asrSeg = Math.round((performance.now() - inicioRef.current) / 100) / 10;
    // Con un motor que sabe cerrar el turno **sin** cerrar la sesión (AssemblyAI: `ForceEndpoint`) la
    // conexión queda viva para el turno siguiente; si no, se corta y se vuelve a abrir como siempre.
    const reusa = Boolean(rt.cerrarTurno);
    const fuente: 'ForceEndpoint' | 'stop' = reusa ? 'ForceEndpoint' : 'stop';
    const dictado = (await (reusa ? rt.cerrarTurno!() : rt.stop())).trim();
    turnosServidosRef.current += 1;
    ultimaActividadRef.current = performance.now();
    trazaVoz('turno.cerrado', {
      turno: turnoRef.current,
      fuente,
      ms: Math.round(performance.now() - t0ListoRef.current),
      chars: dictado.length,
      vacio: !dictado,
    });
    // Con un motor que sabe cerrar el turno sin cerrar la sesión, la conexión queda **pausada** y el turno
    // siguiente la reusa. Cerrarla acá costaba ~1 s de handshake con el micrófono abierto al final, que es
    // justo donde se perdían las primeras palabras del turno siguiente (medido 20/09/2026: 10 pulsaciones
    // del atajo → 13 sesiones de STT emitidas). El cierre real lo hace el reloj de inactividad de 90 s, y
    // el cambio de modo (fin de conversación, panel cerrado) cierra en el acto.
    if (!reusa) cerrarSesion('sin_reuso');
    setParcial('');
    setTexto(dictado);
    if (!dictado) {
      setError('No se escuchó nada. Probá de nuevo hablando más cerca del micrófono.');
      setEstado('inactivo');
      // En conversación el turno no se corta por un silencio: se vuelve a escuchar (después de que
      // termine de sonar el aviso, no encima).
      if (continuoRef.current) {
        await hablar('No te escuché. ¿Me lo repetís?');
        void seguirEscuchando();
      }
      return;
    }
    // ── Eco del micrófono ─────────────────────────────────────────────────────────────────────
    // Con parlantes, el motor de transcripción escucha lo que la propia app acaba de decir. No es una
    // respuesta tuya: no se toca el lienzo y se vuelve a escuchar. A la tercera vez seguida la
    // conversación se cierra y dice por qué (antes seguía repitiendo la misma pregunta sin fin).
    if (esEco(dictado, dichoRef.current)) {
      ecosSeguidosRef.current += 1;
      const veces = ecosSeguidosRef.current;
      setResultado('Me escuché a mí misma (eco del micrófono): no lo tomo como respuesta.');
      if (veces >= 3) {
        setContinuo(false);
        continuoRef.current = false;
        setEstado('inactivo');
        cerrarSesion('eco');
        setError(
          'Cerré la conversación: el micrófono estaba escuchando la voz de la app. Usá auriculares, o apagá la voz de salida, y volvé a empezar.'
        );
        return;
      }
      void seguirEscuchando();
      return;
    }
    ecosSeguidosRef.current = 0;
    // ── Modo conversación ─────────────────────────────────────────────────────────────────────
    // a) Aprobación hablada: el plan esperaba un «¿lo aplico?» y el contrato sigue siendo el mismo
    //    (el humano aprueba), sólo que aprobás hablando.
    if (continuoRef.current && planPendienteRef.current) {
      if (esAfirmativo(dictado)) {
        setPlanPendiente(false);
        await aplicar();
        if (!silencio) await hablar('Listo, aplicado. ¿Qué más querés hacer?');
      } else {
        setPlanPendiente(false);
        setPlan(null);
        setResultado('Lo dejé sin aplicar.');
        if (!silencio) await hablar('Lo dejo sin aplicar. ¿Qué más querés hacer?');
      }
      void seguirEscuchando();
      return;
    }
    // b) El guion del modo conversación: pasos guiados que no gastan motor (0 tokens).
    if (continuoRef.current) {
      const turno = await onTurnoConversacion(dictado);
      if (!turno?.alMotor) {
        if (turno?.decir && !silencio) {
          // El guion habla: queda marcado para que el motor no diga una segunda frase en este turno.
          guionHabloRef.current = true;
          await hablar(turno.decir);
        }
        if (turno?.fin) {
          setContinuo(false);
          continuoRef.current = false;
          setEstado('inactivo');
          cerrarSesion('fin_dialogo');
          return;
        }
        void seguirEscuchando();
        return;
      }
      // No era parte del guion: sigue el camino normal (le pide el plan al motor) y al final
      // retoma la conversación.
    }

    // ── Los cierres del ciclo, sin motor ──────────────────────────────────────────────────────
    // 1) Si la app te acaba de leer una pregunta, lo que dijiste ES la respuesta: se guarda y se cierra.
    if (modoRespuesta) {
      const pregunta = modoRespuesta;
      setModoRespuesta(null);
      setResultado(`Respuesta guardada · la pregunta quedó cerrada.`);
      onResponder(pregunta.id, dictado);
      if (!silencio) void hablar('Anotado. La pregunta quedó cerrada.');
      setEstado('inactivo');
      return;
    }
    // 2) «¿Qué quedó abierto?»: regla local. Se lee la primera pregunta y se queda esperando la
    //    respuesta: es el ciclo del pensamiento con las manos libres y sin gastar un token.
    if (PEDIDO_DE_RETOMAR.test(dictado)) {
      const pendiente = preguntaAbierta();
      if (!pendiente) {
        setResultado('No hay preguntas abiertas en el lienzo.');
        void hablar('No hay preguntas abiertas en el lienzo.');
        setEstado('inactivo');
        return;
      }
      setModoRespuesta(pendiente);
      setResultado(`Pregunta abierta: ${pendiente.titulo} · apretá el micrófono y respondé.`);
      void hablar(`Pregunta abierta: ${pendiente.titulo}. Te escucho.`);
      setEstado('inactivo');
      return;
    }

    setPensando(true);
    try {
      const { plan: p, modelo, uso } = await pedirPlanVoz(dictado);
      setPlan(p);
      setMetricas({
        asrSeg,
        ms: Math.round(uso?.ms ?? 0),
        modelo: uso?.modelo || modelo || 'motor',
        costo: uso?.costo_usd ?? 0,
        cache: uso?.cache ?? 'miss',
      });
      // En conversación, el plan no se aplica solo: se pide en voz alta y espera un «sí».
      // Una consulta no se aprueba: no hay nada que aplicar. Se dice la respuesta y sigue el turno.
      if (continuoRef.current && p.comandos?.length && !planEsConsulta(p.comandos)) {
        setPlanPendiente(true);
        await hablar(`Voy a ${onPrevisualizar(p)}. ¿Lo aplico?`);
        void empezar('conv');
        return;
      }
      if (continuoRef.current) {
        await hablar(p.respuesta || 'Listo.');
        void empezar('conv');
        return;
      }
      // El backend decidió si esto merece voz; acá sólo se obedece.
      // El backend decidió si esto merece voz; acá sólo se obedece. Si el guion ya dijo su frase en este
      // turno, el motor NO vuelve a hablar: eran dos voces seguidas y sonaba a bot confundido.
      if (p.hablar && !silencio && !guionHabloRef.current) void hablar(p.respuesta || '');
    } catch (e: any) {
      setError(e?.message || 'El motor no pudo interpretar el dictado.');
    } finally {
      setPensando(false);
    }
  };

  cortarRef.current = cortar;

  /** El HUD (panel cerrado) necesita aplicar el plan sin abrir el modal. */
  const aplicarRef = useRef<(() => Promise<void>) | null>(null);

  /** El atajo global necesita llamar a `empezar` desde afuera del render (mismo motivo que `cortarRef`). */
  const empezarRef = useRef<((origen?: 'ui' | 'atajo' | 'conv') => Promise<void>) | null>(null);
  empezarRef.current = empezar;

  /**
   * Pedido del atajo global (Ctrl+Shift+Space): al presionar empieza el turno, al soltar se corta.
   * Se observa el contador `n` y no el objeto: dos pulsaciones seguidas tienen que ejecutarse las dos.
   */
  useEffect(() => {
    if (!pedidoExterno) return;
    // El pedido del atajo se atiende también con el panel CERRADO: es justamente así como se usa (dictar
    // sin abrir el modal, con el lienzo a la vista).
    if (pedidoExterno.accion === 'empezar') void empezarRef.current?.('atajo');
    else void cortarRef.current?.();
    // Sólo el contador: el pedido se ejecuta una vez por pulsación.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pedidoExterno?.n]);

  /**
   * Cierre REAL de la sesión por inactividad.
   *
   * Por qué existe y por qué 90 s: con el turno cerrado por `ForceEndpoint` la conexión queda pausada
   * pero **abierta**, y el proveedor factura el tiempo de conexión, no el audio (medido 20/09/2026). El
   * reloj sólo corre con la sesión pausada y la app en silencio: mientras escucha, mientras piensa o
   * mientras habla, no se toca nada.
   */
  useEffect(() => {
    const t = window.setInterval(() => {
      const rt = rtRef.current;
      if (!rt?.viva) return;
      if (estado !== 'inactivo' || hablandoRef.current) return;
      const quieto = performance.now() - ultimaActividadRef.current;
      if (quieto < 90_000) return;
      trazaVoz('sesion.inactiva', { sesion: sesionRef.current, ms: Math.round(quieto) });
      cerrarSesion('inactividad');
    }, 5000);
    return () => window.clearInterval(t);
  }, [estado]);

  /** Arranca la conversación: la app pregunta primero y después escucha. */
  const conversar = async () => {
    setError('');
    setPlan(null);
    if (continuo) {
      setContinuo(false);
      continuoRef.current = false;
      cerrarSesion('fin_conversacion');
      setResultado('Conversación terminada.');
      return;
    }
    setContinuo(true);
    continuoRef.current = true;
    const saludo = onInicioConversacion();
    setPasoConv('idea');
    setResultado(`Conversación · ${saludo}`);
    await hablar(saludo);
    void empezar('conv');
  };

  const aplicar = async () => {
    if (!plan) return;
    setAplicando(true);
    try {
      const r = await onAplicar(plan);
      if (r) {
        setResultado(
          r.consultas?.length
            ? `Consulta respondida · sin cambios en el lienzo.`
            : `Listo: ${r.creados} nodo(s) creado(s), ${r.afectados} afectado(s).`
        );
        if (r.delegando) setInvestigando(true);
      }
      setPlan(null);
    } finally {
      setAplicando(false);
    }
  };

  // Después de la definición (no antes: `aplicar` es const y usarla antes sería un error de inicialización).
  aplicarRef.current = aplicar;

  // ── HUD flotante: dictar sin abrir el modal ────────────────────────────────────────────────
  // Con el atajo global el panel NO se abre: el lienzo tiene que quedar a la vista, porque el grafo es el
  // resultado y el modal lo tapaba. Mientras hay voz en curso se muestra este indicador abajo a la derecha:
  // el parcial mientras hablás y, si quedó un plan, cuántos cambios hay y el botón para aplicarlos.
  if (!isOpen) {
    const n = plan?.comandos?.length ?? 0;
    const activo = estado !== 'inactivo' || pensando || n > 0;
    if (!activo) return null;
    return (
      <div className="fixed bottom-4 right-4 z-40 w-[22rem] rounded-xl border border-cyan-500/40 bg-slate-900/90 px-3 py-2 text-[11px] text-slate-200 shadow-xl backdrop-blur">
        <div className="flex items-center gap-2">
          <Mic
            size={13}
            className={estado === 'escuchando' ? 'text-cyan-300 animate-pulse' : 'text-slate-400'}
          />
          <span className="font-medium">
            {estado === 'escuchando'
              ? t('voz.hud.escuchando')
              : pensando
                ? t('voz.hud.pensando')
                : t('voz.hud.voz')}
          </span>
          {(estado === 'escuchando' || estado === 'conectando') && (
            <button
              onClick={() => void cortarRef.current?.()}
              className="ml-auto flex items-center gap-1 rounded border border-slate-600 px-1.5 py-0.5 text-[10px] text-slate-300 hover:bg-slate-800"
            >
              <Square size={9} /> {t('voz.hud.cortar')}
            </button>
          )}
        </div>
        <div className="mt-1 min-h-[15px] text-slate-400">{parcial || texto || ''}</div>
        {n > 0 && (
          <div className="mt-2 flex items-center gap-2 border-t border-slate-700/60 pt-2">
            <span className="text-cyan-300">
              {t('voz.hud.cambios').replace('{n}', String(n))}
            </span>
            <button
              onClick={() => void aplicarRef.current?.()}
              className="ml-auto flex items-center gap-1 rounded bg-cyan-600/90 px-2 py-0.5 text-[10px] font-medium text-white hover:bg-cyan-500"
            >
              <Check size={10} /> {t('voz.hud.aplicar')}
            </button>
          </div>
        )}
        {error && <div className="mt-1 text-amber-300">{error}</div>}
      </div>
    );
  }

  const escuchando = estado === 'escuchando' || estado === 'conectando' || estado === 'cerrando';
  const colorEstado = estado === 'escuchando' ? 'bg-emerald-400' : estado === 'error' ? 'bg-rose-400' : estado === 'conectando' || estado === 'cerrando' ? 'bg-amber-400' : 'bg-slate-500';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm" id="voz-panel">
      <div className="relative w-full max-w-2xl max-h-[88vh] overflow-hidden flex flex-col bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl">
        {/* Encabezado */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400">
              <Mic size={17} />
            </div>
            <div>
              <div className="text-sm font-semibold text-slate-200 flex items-center gap-2">
                Voz
                {/* El motor es DATO: el panel no sabe con quién habla. Si el backend no declara
                    modelo, se dice, no se inventa uno (antes caía en 'enhanced', que es un
                    modelo de Speechmatics y mentía cuando el motor activo era otro). */}
                <span className="text-[10px] font-mono px-1.5 py-0.5 rounded border border-slate-700 text-slate-400">
                  {servicio?.proveedor_etiqueta || servicio?.proveedor || 'motor de voz'}
                  {servicio?.modelo ? ` · ${servicio.modelo}` : ' · modelo no declarado'}
                </span>
                <span className={`w-2 h-2 rounded-full ${colorEstado} ${estado === 'escuchando' ? 'animate-pulse' : ''}`} />
              </div>
              <div className="text-[11px] text-slate-400">
                {estado === 'escuchando'
                  ? 'Escuchando… hablá normal'
                  : estado === 'conectando'
                    ? `Conectando con ${servicio?.proveedor_etiqueta || servicio?.proveedor || 'el motor de voz'}…`
                    : estado === 'cerrando'
                      ? 'Cerrando el dictado…'
                      : estado === 'error'
                        ? `Error: ${detalleEstado || 'ver abajo'}`
                        : 'Hablá y el lienzo se opera solo (vos aprobás)'}
              </div>
            </div>
          </div>
          <button type="button" onClick={onClose} className="p-2 text-slate-400 hover:text-slate-200 rounded-lg hover:bg-slate-800 cursor-pointer">
            <X size={16} />
          </button>
        </div>

        {/* Cuerpo */}
        <div className="p-5 overflow-y-auto space-y-4 text-sm flex-1">
            <button
              type="button"
              id="btn-voz-conversar"
              onClick={conversar}
              title={
                continuo
                  ? 'Terminar la conversación'
                  : 'Modo conversación: la app te pregunta primero y van por turnos, sin tocar nada'
              }
              className={`flex items-center gap-2 px-3 py-2.5 rounded-xl text-sm font-semibold border transition-colors cursor-pointer ${
                continuo
                  ? 'bg-amber-600/90 hover:bg-amber-500 text-white border-amber-400/60'
                  : 'bg-slate-800/80 hover:bg-slate-700 text-amber-200 border-slate-700'
              }`}
            >
              <MessageSquarePlus size={15} />
              {continuo ? 'Conversación activa' : 'Conversar'}
            </button>
            {continuo && (
              <span className="text-[10px] font-mono px-1.5 py-0.5 rounded border border-amber-700/50 text-amber-300 whitespace-nowrap">
                {escuchando ? 'te escucho' : hablando ? 'hablando' : pensando ? 'pensando' : 'turno'}
              </span>
            )}
          {servicio && !servicio.configurada && (
            <div className="flex gap-2.5 items-start bg-slate-800 border border-slate-700 rounded-xl p-3.5 text-xs">
              <AlertTriangle size={15} className="shrink-0 mt-0.5 text-amber-400" />
              <div>
                <div className="font-semibold mb-0.5 text-amber-200">{t('voz.faltaClave')}</div>
                <div className="text-slate-300">{servicio.pista}</div>
                {servicio.aviso ? (
                  <div className="text-amber-300 flex items-start gap-1">
                    <AlertTriangle className="w-3 h-3 mt-0.5 shrink-0" />
                    <span>{servicio.aviso}</span>
                  </div>
                ) : null}
              </div>
            </div>
          )}

          {/* Botón de escucha */}
          <div className="flex items-center gap-3">
            <button
              type="button"
              id="btn-voz-escuchar"
              onClick={escuchando ? cortar : empezar}
              disabled={pensando || aplicando || (servicio ? !servicio.configurada : false)}
              className={`flex items-center gap-2 px-4 py-2.5 rounded-xl text-sm font-semibold border transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed ${
                escuchando
                  ? 'bg-rose-600/90 hover:bg-rose-500 text-white border-rose-400/60'
                  : 'bg-cyan-600/90 hover:bg-cyan-500 text-white border-cyan-400/60'
              }`}
            >
              {escuchando ? <Square size={15} /> : pensando ? <Loader2 size={15} className="animate-spin" /> : <Mic size={15} />}
              {escuchando ? 'Cortar y armar el plan' : pensando ? 'Interpretando…' : 'Escuchar'}
            </button>
            {servicio && !servicio.configurada && (
              <button type="button" onClick={consultarEstado} className="text-xs text-slate-400 hover:text-slate-200 underline cursor-pointer">
                Ya la puse, reintentar
              </button>
            )}
            <button
              type="button"
              id="btn-voz-silencio"
              onClick={alternarSilencio}
              title={silencio ? 'Activar la voz de salida' : 'Silenciar la voz de salida'}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-[11px] border border-slate-700 bg-slate-800 text-slate-200 hover:text-white transition-colors cursor-pointer"
            >
              {silencio ? <VolumeX size={13} className="text-slate-400" /> : <Volume2 size={13} className={hablando ? 'text-cyan-300 animate-pulse' : 'text-cyan-400'} />}
              {silencio ? 'Voz apagada' : hablando ? 'Hablando…' : 'Voz activa'}
            </button>
            {servicio?.tts && (
              <span className={`text-[11px] ${servicio.tts.disponible ? 'text-slate-400' : 'text-amber-300'}`}>
                {servicio.tts.disponible
                  ? `${servicio.tts.motor} ✓`
                  : motorVoz === 'sistema'
                  ? 'voz del sistema (Kokoro no está en esta PC)'
                  : 'Kokoro no disponible · habla la voz del sistema'}
              </span>
            )}
            <span className="text-[11px] text-slate-500">{servicio?.codec} · latencia objetivo &lt; 1 s</span>
          </div>

          {/* Voz local (Kokoro) como paquete descargable: el instalador es chico a propósito. */}
          {motorLocal && (
            <div className="rounded-xl border border-slate-800 bg-slate-950/40 px-3.5 py-3 space-y-2">
              <div className="flex items-center gap-2">
                <Volume2 size={13} className="text-cyan-400" />
                <span className="text-[11px] font-semibold text-slate-200">Voz local (Kokoro)</span>
                <span
                  className={`ml-auto text-[9px] font-mono px-1.5 py-0.5 rounded border ${
                    motorLocal.corriendo
                      ? 'border-emerald-700/50 text-emerald-300'
                      : motorLocal.instalada
                      ? 'border-amber-700/50 text-amber-300'
                      : 'border-slate-700 text-slate-400'
                  }`}
                >
                  {motorLocal.corriendo ? 'sonando' : motorLocal.instalada ? 'instalada' : 'no instalada'}
                </span>
              </div>
              {motorLocal.en_curso && motorLocal.progreso ? (
                <div>
                  <div className="h-1.5 rounded-full bg-slate-800 overflow-hidden">
                    <div
                      className="h-full bg-cyan-500 transition-[width]"
                      style={{
                        width: `${
                          motorLocal.progreso.total
                            ? Math.min(
                                100,
                                Math.round((motorLocal.progreso.bajado / motorLocal.progreso.total) * 100)
                              )
                            : 8
                        }%`,
                      }}
                    />
                  </div>
                  <p className="text-[10px] text-slate-500 mt-1">
                    {motorLocal.progreso.fase} ·{' '}
                    {(motorLocal.progreso.bajado / 1024 / 1024).toFixed(0)} MB de{' '}
                    {motorLocal.progreso.total
                      ? `${(motorLocal.progreso.total / 1024 / 1024).toFixed(0)} MB`
                      : '—'}
                  </p>
                </div>
              ) : (
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    id="btn-voz-motor-instalar"
                    onClick={instalarMotorLocal}
                    className="px-2.5 py-1.5 rounded-lg text-[11px] bg-cyan-600/20 border border-cyan-500/40 text-cyan-100 hover:bg-cyan-600/30 cursor-pointer"
                  >
                    {motorLocal.instalada
                      ? 'Arrancar y verificar'
                      : `Descargar e instalar (${motorLocal.tamano_descarga})`}
                  </button>
                  <span className="text-[10px] text-slate-500 leading-tight">
                    {motorLocal.instalada
                      ? 'Kokoro corre en tu placa: sin cuotas y sin que el texto salga de la máquina.'
                      : 'Opcional. Mientras tanto habla la voz del sistema, que ya está en Windows.'}
                  </span>
                </div>
              )}
              {motorLocal.progreso?.error && (
                <p className="text-[10px] text-amber-300">
                  La descarga falló: {motorLocal.progreso.error}. Podés reintentar.
                </p>
              )}
            </div>
          )}

          {/* Transcripción viva */}
          <div className="bg-slate-900/70 border border-slate-700 rounded-xl p-3.5 min-h-[110px] max-h-[200px] overflow-y-auto">
            {!texto && !parcial && (
              <div className="space-y-1.5">
                {EJEMPLOS.map((t) => (
                  <div key={t} className="text-[11px] text-slate-500 italic">· {t}</div>
                ))}
              </div>
            )}
            {texto && <p className="text-xs text-slate-200 leading-relaxed">{texto}</p>}
            {parcial && <p className="text-xs text-slate-400 italic leading-relaxed">{parcial}…</p>}
          </div>

          {error && (
            <div className="flex gap-2 items-start text-xs bg-slate-800 border border-slate-700 rounded-xl p-3">
              <AlertTriangle size={13} className="shrink-0 mt-0.5 text-rose-400" />
              <span className="text-slate-200">{error}</span>
            </div>
          )}

          {/* Plan propuesto */}
          {plan && (
            <div className="bg-slate-900/70 border border-slate-700 rounded-xl p-4 space-y-3" id="voz-plan">
              <div className="flex items-center gap-2 text-[10px] uppercase tracking-widest text-violet-300 font-bold">
                <Sparkles size={12} /> Plan propuesto
                <span className="px-1.5 py-0.5 rounded border border-slate-600 text-violet-200 font-mono normal-case tracking-normal">
                  {plan.intencion === 'capturar' ? 'agregar al lienzo' : 'operar sobre el lienzo'}
                </span>
              </div>
              <p className="text-sm text-slate-200 leading-relaxed">{plan.respuesta}</p>
              {plan.motivo && <p className="text-[11px] text-slate-400 italic">{plan.motivo}</p>}

              <div className="space-y-1.5">
                {plan.comandos.length === 0 && (
                  <div className="text-xs text-slate-400">{t('voz.nadaAplicable')}</div>
                )}
                {plan.comandos.map((c, i) => (
                  <div key={i} className="flex items-center gap-2 text-xs text-slate-200 bg-slate-800 rounded-lg px-2.5 py-1.5 border border-slate-700">
                    <span className="text-violet-300">{ICONO[c.accion]}</span>
                    <span>{describirComando(c, tituloNodo)}</span>
                    {c.criterio && <span className="text-slate-500 truncate">· {c.criterio}</span>}
                  </div>
                ))}
              </div>

              {!!plan.descartados && (
                <div className="text-[11px] text-slate-300">
                  Descarté {plan.descartados} operación(es) que no cerraban contra el lienzo.
                  {plan.motivo_descarte?.length ? ` (${plan.motivo_descarte.slice(0, 2).join('; ')})` : ''}
                </div>
              )}

              {plan.comandos.length > 0 && (
                <div
                  id="voz-impacto"
                  className="flex items-start gap-2 text-[11px] bg-slate-800 border border-slate-700 rounded-lg px-2.5 py-2"
                >
                  <AlertTriangle size={12} className="shrink-0 mt-0.5 text-amber-400" />
                  <span className="text-slate-200">
                    <span className="font-semibold text-amber-200">{t('voz.vaAPasar')}</span> {onPrevisualizar(plan)}
                  </span>
                </div>
              )}

              <div className="flex items-center gap-2 pt-1">
                <button
                  type="button"
                  id="btn-voz-aplicar"
                  onClick={aplicar}
                  disabled={aplicando || plan.comandos.length === 0}
                  className="flex items-center gap-1.5 px-3 py-2 bg-violet-600 hover:bg-violet-500 text-white rounded-xl text-xs font-semibold border border-violet-400/60 transition-colors cursor-pointer disabled:opacity-50"
                >
                  {aplicando ? <Loader2 size={13} className="animate-spin" /> : <Check size={13} />}
                  Aplicar al lienzo
                </button>
                <button
                  type="button"
                  onClick={() => setPlan(null)}
                  className="px-3 py-2 text-xs text-slate-300 hover:text-white bg-slate-800 hover:bg-slate-700 rounded-xl border border-slate-600 transition-colors cursor-pointer"
                >
                  Descartar
                </button>
                <span className="text-[11px] text-slate-500">{t('voz.deshacer')}</span>
              </div>
            </div>
          )}

          {resultado && (
            <div className="flex items-center gap-2 text-xs bg-slate-800 border border-slate-700 rounded-xl p-3">
              <Wand2 size={13} className="text-emerald-400" /> <span className="text-slate-200">{resultado}</span>
            </div>
          )}

          {investigando && (
            <div className="rounded-xl border border-slate-700 bg-slate-800 p-3 space-y-2" id="voz-investigando">
              <div className="flex items-center gap-2 text-xs">
                <Loader2 size={13} className="animate-spin text-violet-400" />
                <span className="text-slate-100 font-medium">{t('voz.investigando')}</span>
                <span className="text-slate-400">· el nodo crece en el lienzo mientras tanto</span>
              </div>
              {fases.length === 0 ? (
                <p className="text-[11px] text-slate-300">🌱 Arrancando: nace el nodo y sale a buscar fuentes…</p>
              ) : (
                <div className="space-y-1">
                  {['🌱', '⚔️', '🧪', '🚀'].map((e, idx) => {
                    const f = fases.find((x) => x.emoji === e);
                    const ultima = fases[fases.length - 1];
                    const activa = !!f && !!ultima && f.fase === ultima.fase;
                    return (
                      <div key={e} className={`flex items-start gap-2 text-[11px] ${f ? 'text-slate-200' : 'text-slate-500'}`}>
                        <span>{e}</span>
                        <span className={activa ? 'text-slate-100' : ''}>
                          {f ? f.que : 'pendiente'}
                          {activa && <Loader2 size={10} className="inline ml-1 animate-spin text-violet-300" />}
                        </span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          )}

          {delegado && (
            <div className="rounded-xl border border-slate-700 bg-slate-800 p-3 space-y-2" id="voz-delegado">
              <div className="flex items-center gap-2 text-[11px] text-slate-300">
                <Sparkles size={13} className="text-violet-400" />
                <span className="font-medium text-slate-100">{t('voz.motorProfundo')}</span>
                <span className="text-slate-400">
                  · {delegado.ms > 0 ? `${Math.round(delegado.ms / 1000)} s · ` : ''}te lo respondió Hermes con sus herramientas
                </span>
              </div>
              <p className="text-xs text-slate-200 whitespace-pre-wrap">{delegado.salida}</p>
              <p className="text-[10px] text-slate-400">Lo pediste: «{delegado.pedido}»</p>
            </div>
          )}
        </div>

        {/* Pie: lo medido en esta corrida */}
        <div className="flex items-center justify-between gap-3 px-5 py-3 border-t border-slate-800 text-[11px] text-slate-400">
          <span className="flex items-center gap-1.5">
            <Gauge size={12} className="text-cyan-400" />
            {metricas
              ? `dictado ${metricas.asrSeg} s · plan ${metricas.ms} ms · ${metricas.modelo} · ${metricas.cache === 'hit' ? 'caché HIT' : `US$${metricas.costo.toFixed(6)}`}`
              : 'El costo y la latencia de cada dictado se miden acá'}
          </span>
          <span>{t('voz.pie')}</span>
          <span className="ml-2 text-cyan-500/80">{t('voz.atajo')}</span>
        </div>
      </div>
    </div>
  );
};
