/**
 * Tests de los clientes de voz: **el turno y nada más que el turno**.
 *
 * Por qué existen: al empezar a reusar la sesión entre turnos (20/09/2026) aparecieron dos defectos que sólo
 * se ven con la conexión viva de un turno al siguiente —el acumulador de texto nunca se vaciaba, así que el
 * segundo turno devolvía el texto del primero pegado— y no hay forma de cazarlos con el micrófono real sin
 * una sesión de voz de verdad. Acá el WebSocket, el micrófono y el grafo de audio son falsos, y lo que se
 * prueba es el contrato del cliente: qué manda por el socket y qué texto devuelve cada turno.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AssemblyAiRt } from './assemblyaiRt';
import { SpeechmaticsRt } from './speechmaticsRt';

// ── WebSocket falso ─────────────────────────────────────────────────────────────────────────────────
class FakeWebSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  /** El último socket creado: es el que los tests manejan a mano. */
  static ultimo: FakeWebSocket | null = null;

  readyState = FakeWebSocket.CONNECTING;
  binaryType = 'arraybuffer';
  /** Todo lo que el cliente mandó: strings del protocolo y frames binarios. */
  enviados: Array<string | ArrayBuffer> = [];
  onopen: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onmessage: ((ev: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;

  constructor(public url: string) {
    FakeWebSocket.ultimo = this;
  }

  send(dato: string | ArrayBuffer) {
    this.enviados.push(dato);
  }

  close() {
    this.readyState = FakeWebSocket.CLOSED;
    this.onclose?.();
  }

  /** Handshake del servidor: a partir de acá el socket está abierto. */
  abrir() {
    this.readyState = FakeWebSocket.OPEN;
    this.onopen?.();
  }

  /** Mensaje del servidor (siempre JSON de texto en los dos protocolos). */
  recibir(obj: unknown) {
    this.onmessage?.({ data: JSON.stringify(obj) });
  }

  /** Mensajes de texto que mandó el cliente (sin los frames de audio). */
  textos(): string[] {
    return this.enviados.filter((e): e is string => typeof e === 'string');
  }
}

// ── Grafo de audio y micrófono falsos ───────────────────────────────────────────────────────────────
class FakeNodo {
  conectado = false;
  connect() {
    this.conectado = true;
  }
  disconnect() {
    this.conectado = false;
  }
}

class FakeProcesador extends FakeNodo {
  onaudioprocess: ((e: { inputBuffer: { getChannelData: () => Float32Array } }) => void) | null = null;
  /** Simula un bloque de audio del micrófono. */
  empujarAudio() {
    this.onaudioprocess?.({ inputBuffer: { getChannelData: () => new Float32Array(4096) } });
  }
}

class FakeAudioContext {
  static ultimo: FakeAudioContext | null = null;
  readonly destination = {};
  readonly fuente = new FakeNodo();
  readonly procesador = new FakeProcesador();
  cerrado = false;

  constructor(public opciones?: unknown) {
    FakeAudioContext.ultimo = this;
  }

  createMediaStreamSource() {
    return this.fuente;
  }
  createScriptProcessor() {
    return this.procesador;
  }
  close() {
    this.cerrado = true;
  }
}

beforeEach(() => {
  FakeWebSocket.ultimo = null;
  FakeAudioContext.ultimo = null;
  vi.stubGlobal('WebSocket', FakeWebSocket);
  vi.stubGlobal('AudioContext', FakeAudioContext);
  Object.defineProperty(navigator, 'mediaDevices', {
    configurable: true,
    value: { getUserMedia: async () => ({ getTracks: () => [{ stop: () => {} }] }) },
  });
});

/** Arranca el cliente y completa el handshake: deja el socket abierto y el micrófono listo. */
async function arrancar(cliente: { start: () => Promise<void> }) {
  const arranque = cliente.start();
  FakeWebSocket.ultimo!.abrir();
  await arranque;
}

describe('AssemblyAiRt · un turno por turno', () => {
  const cfg = { url: 'wss://streaming.assemblyai.com/v3/ws', token: 'tok', idioma: 'es', modelo: 'u3-rt-pro' };

  it('el turno siguiente NO arrastra el texto del anterior (la sesión se reusa)', async () => {
    const cliente = new AssemblyAiRt(cfg, {});
    await arrancar(cliente);
    const ws = FakeWebSocket.ultimo!;

    ws.recibir({ type: 'Turn', transcript: 'idea uno', end_of_turn: false });
    const turno1 = cliente.cerrarTurno();
    ws.recibir({ type: 'Turn', transcript: 'idea uno', end_of_turn: true });
    expect(await turno1).toBe('idea uno');

    // Misma conexión, turno nuevo: antes devolvía «idea uno idea dos».
    ws.recibir({ type: 'Turn', transcript: 'idea dos', end_of_turn: false });
    const turno2 = cliente.cerrarTurno();
    ws.recibir({ type: 'Turn', transcript: 'idea dos', end_of_turn: true });
    expect(await turno2).toBe('idea dos');
  });

  it('cerrarTurno pide ForceEndpoint y deja la sesión viva (no la cierra)', async () => {
    const cliente = new AssemblyAiRt(cfg, {});
    await arrancar(cliente);
    const ws = FakeWebSocket.ultimo!;
    ws.recibir({ type: 'Turn', transcript: 'hola', end_of_turn: false });

    const cierre = cliente.cerrarTurno();
    expect(ws.textos()).toContain(JSON.stringify({ type: 'ForceEndpoint' }));
    ws.recibir({ type: 'Turn', transcript: 'hola', end_of_turn: true });
    await cierre;

    expect(ws.readyState).toBe(FakeWebSocket.OPEN); // nadie la cerró
    expect(cliente.viva).toBe(true);
  });

  it('pausar desengancha el micrófono y reanudar lo vuelve a enganchar sobre la misma sesión', async () => {
    const cliente = new AssemblyAiRt(cfg, {});
    await arrancar(cliente);
    const ctx = FakeAudioContext.ultimo!;

    expect(ctx.fuente.conectado).toBe(true);
    cliente.pausar();
    expect(ctx.fuente.conectado).toBe(false);
    cliente.reanudar();
    expect(ctx.fuente.conectado).toBe(true);
  });

  it('stop cosecha el turno y cierra el socket', async () => {
    const cliente = new AssemblyAiRt(cfg, {});
    await arrancar(cliente);
    const ws = FakeWebSocket.ultimo!;
    ws.recibir({ type: 'Turn', transcript: 'chau', end_of_turn: true });

    const parada = cliente.stop();
    ws.recibir({ type: 'Termination' });
    expect(await parada).toBe('chau');
    expect(cliente.viva).toBe(false);
  });
});

describe('SpeechmaticsRt · paridad de sesión', () => {
  const cfg = { url: 'wss://eu.rt.speechmatics.com/v2', jwt: 'jwt', idioma: 'es', modelo: 'enhanced' };

  it('cerrarTurno pide ForceEndOfUtterance, cosecha y deja la sesión abierta', async () => {
    const cliente = new SpeechmaticsRt(cfg, {});
    await arrancar(cliente);
    const ws = FakeWebSocket.ultimo!;

    ws.recibir({ message: 'AddTranscript', metadata: { transcript: 'primera idea' } });
    const turno1 = cliente.cerrarTurno();
    expect(ws.textos()).toContain(JSON.stringify({ message: 'ForceEndOfUtterance' }));
    ws.recibir({ message: 'EndOfUtterance', metadata: { forced: true } });
    expect(await turno1).toBe('primera idea');

    // Segundo turno sobre la MISMA conexión, sin arrastrar el texto anterior.
    ws.recibir({ message: 'AddTranscript', metadata: { transcript: 'segunda idea' } });
    const turno2 = cliente.cerrarTurno();
    ws.recibir({ message: 'EndOfUtterance', metadata: { forced: true } });
    expect(await turno2).toBe('segunda idea');

    expect(ws.readyState).toBe(FakeWebSocket.OPEN);
    expect(cliente.viva).toBe(true);
  });

  it('pausada, no manda ni un frame de audio; reanudada, sí', async () => {
    const cliente = new SpeechmaticsRt(cfg, {});
    await arrancar(cliente);
    const ws = FakeWebSocket.ultimo!;
    const procesador = FakeAudioContext.ultimo!.procesador;

    procesador.empujarAudio();
    const hablando = ws.enviados.length;
    expect(hablando).toBeGreaterThan(0);

    cliente.pausar();
    procesador.empujarAudio();
    procesador.empujarAudio();
    expect(ws.enviados.length).toBe(hablando); // la pausa corta el audio de verdad

    cliente.reanudar();
    procesador.empujarAudio();
    expect(ws.enviados.length).toBe(hablando + 1);
  });

  it('sin habla no llega EndOfUtterance: la red de seguridad devuelve el turno vacío', async () => {
    vi.useFakeTimers();
    try {
      const cliente = new SpeechmaticsRt(cfg, {});
      const arranque = cliente.start();
      FakeWebSocket.ultimo!.abrir();
      await vi.advanceTimersByTimeAsync(0);
      await arranque;

      const turno = cliente.cerrarTurno();
      await vi.advanceTimersByTimeAsync(2000);
      expect(await turno).toBe('');
      expect(cliente.viva).toBe(true);
    } finally {
      vi.useRealTimers();
    }
  });
});
