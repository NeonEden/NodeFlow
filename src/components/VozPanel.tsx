import React, { useCallback, useEffect, useRef, useState } from 'react';
import { X, Mic, Square, Loader2, Sparkles, Check, AlertTriangle, Wand2, Target, Layers, MessageSquarePlus, Link2, Quote, Gauge, Volume2, VolumeX, PenLine, CornerDownRight, MessageCircleQuestion } from 'lucide-react';
import { type EstadoVoz } from '../services/speechmaticsRt';
import { crearClienteStt, type ClienteStt } from '../services/sttRt';
import { apiUrl } from '../services/apiBase';
import { getVozEstado, getVozJwt, pedirPlanVoz, describirComando, decir, hablarConElSistema, VozEstado, PlanVoz, VozComando } from '../services/vozService';
import { useIdioma } from '../i18n/useIdioma';
import { planEsConsulta } from '../utils/voz';

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
}

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
/** «Sí» hablado: sirve para aprobar lo que la app propone en modo conversación. */
const ES_AFIRMATIVO =
  /^\s*(s[ií]|dale|ok|okey|aplic\w*|vale|claro|obvio|por supuesto|perfecto|hac[eé]lo|vamos|de una|yes)\b/i;

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
export const VozPanel: React.FC<VozPanelProps> = ({ isOpen, onClose, onAplicar, onPrevisualizar, onAplicarComandos, tituloNodo, preguntaAbierta, onResponder, onInicioConversacion, onTurnoConversacion }) => {
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
  // Referencia viva del mute: el sondeo no se reinicia cada vez que se toca el botón.
  const silencioRef = useRef(silencio);
  useEffect(() => {
    silencioRef.current = silencio;
  }, [silencio]);
  const rtRef = useRef<ClienteStt | null>(null);
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
      void rtRef.current.stop();
      rtRef.current = null;
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
    if (del.length < 3) return false;
    const suyas = new Set(palabras(dicho));
    if (!suyas.size) return false;
    return del.filter((p) => suyas.has(p)).length / del.length >= 0.7;
  };

  /** Habla sólo si el backend lo autorizó (regla de voz selectiva) y no está en silencio. */
  const hablar = async (texto: string) => {
    if (!texto.trim()) return;
    dichoRef.current = texto;
    setHablando(true);
    try {
      // Primero la voz local (Kokoro). Si esa PC no la tiene —una instalación limpia nunca la
      // tiene—, habla la voz del sistema: la app no queda muda en ninguna máquina.
      const audio = await decir(texto).catch(() => null);
      if (audio) {
        if (motorVoz !== 'kokoro') setMotorVoz('kokoro');
        const url = URL.createObjectURL(audio);
        const el = new Audio(url);
        // Se espera el FIN del audio, no el comienzo: `play()` resuelve apenas arranca a sonar, y el
        // micrófono se abría encima de la propia voz (medido 19/09 con parlantes: el motor de
        // transcripción transcribía a la app y la conversación se mordía la cola).
        await new Promise<void>((listo) => {
          let cerrado = false;
          const fin = () => {
            if (cerrado) return;
            cerrado = true;
            URL.revokeObjectURL(url);
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

  /** Cierra la sesión de voz de verdad (fin de conversación, eco, cambio de modo). */
  const cerrarSesion = () => {
    const rt = rtRef.current;
    rtRef.current = null;
    if (rt) void rt.stop();
  };

  /**
   * Vuelve a escuchar cuando el turno se cierra por conversación: deja pasar un instante para que el
   * audio termine de apagarse en la sala antes de abrir el micrófono.
   *
   * Medido 20/09/2026: abrir una sesión nueva por turno daba **11 sesiones en 90 s** — handshake en cada
   * una, y el proveedor factura el tiempo de conexión abierto. Si la sesión sigue viva, el turno
   * siguiente va sobre la misma (el motor la cierra sólo si se la termina).
   */
  const seguirEscuchando = async () => {
    await new Promise((r) => window.setTimeout(r, 350));
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
    void empezar();
  };

  const empezar = async () => {
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
      const rt = crearClienteStt(sesion, {
        onEstado: (e, d) => {
          setEstado(e);
          if (d) setDetalleEstado(d);
        },
        onParcial: (t) => {
          fragRef.current = t;
          setParcial(t);
        },
        onFinal: (t) => {
          fragRef.current = t;
          setTexto(t);
        },
        onError: (m) => setError(m),
      });
      rtRef.current = rt;
      inicioRef.current = performance.now();
      await rt.start();
    } catch (e: any) {
      setError(e?.message || 'No pude empezar a escuchar.');
      setEstado('error');
    }
  };

  const cortar = async () => {
    const rt = rtRef.current;
    if (!rt) return;
    const asrSeg = Math.round((performance.now() - inicioRef.current) / 100) / 10;
    // Con un motor que sabe cerrar el turno **sin** cerrar la sesión (AssemblyAI: `ForceEndpoint`) la
    // conexión queda viva para el turno siguiente; si no, se corta y se vuelve a abrir como siempre.
    const reusa = Boolean(rt.cerrarTurno);
    const dictado = (await (reusa ? rt.cerrarTurno!() : rt.stop())).trim();
    // Fuera de una conversación no se deja una conexión abierta ocupando el micrófono ni facturando.
    if (!reusa || !continuoRef.current) cerrarSesion();
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
        cerrarSesion();
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
      if (ES_AFIRMATIVO.test(dictado)) {
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
        if (turno?.decir && !silencio) await hablar(turno.decir);
        if (turno?.fin) {
          setContinuo(false);
          continuoRef.current = false;
          setEstado('inactivo');
          cerrarSesion();
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
        void empezar();
        return;
      }
      if (continuoRef.current) {
        await hablar(p.respuesta || 'Listo.');
        void empezar();
        return;
      }
      // El backend decidió si esto merece voz; acá sólo se obedece.
      if (p.hablar && !silencio) void hablar(p.respuesta || '');
    } catch (e: any) {
      setError(e?.message || 'El motor no pudo interpretar el dictado.');
    } finally {
      setPensando(false);
    }
  };

  cortarRef.current = cortar;

  /** Arranca la conversación: la app pregunta primero y después escucha. */
  const conversar = async () => {
    setError('');
    setPlan(null);
    if (continuo) {
      setContinuo(false);
      continuoRef.current = false;
      cerrarSesion();
      setResultado('Conversación terminada.');
      return;
    }
    setContinuo(true);
    continuoRef.current = true;
    const saludo = onInicioConversacion();
    setPasoConv('idea');
    setResultado(`Conversación · ${saludo}`);
    await hablar(saludo);
    void empezar();
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

  if (!isOpen) return null;

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
        </div>
      </div>
    </div>
  );
};
