/**
 * Local database with remote synchronization. Stores complete game state.
 */

import ReconnectingWS from './util/ws';

enum StorageKey {
  Player = 'player',
}

// Known message types
enum MessageType {
  Ping = 'ping',
  Pong = 'pong',
  AddDrawingSegment = 'addDrawingSegment',
  RemoveDrawingSegment = 'removeDrawingSegment',
  SubmitWord = 'submitWord',
  GuessWord = 'guessWord',
  Game = 'game',
  YouAre = 'youAre',
  WrongGuess = 'wrongGuess',
  ClearDrawing = 'clearDrawing',
}

export enum Color {
  Black = 'rgb(0, 0, 0)',
  Red = 'rgb(223, 54, 45)',
  Blue = 'rgb(4, 118, 208)',
  Green = 'rgb(89, 152, 26)',
  Yellow = 'rgb(250, 208, 44)',
  Grey = 'rgb(189, 195, 203)',
  White = 'rgb(255, 255, 255)',
}

export interface Point {
  x: number;
  y: number;
}

export interface DrawingSegment {
  id: string;
  stroke: Color;
  lineWidth: number;
  points: Point[];
}

interface Message {
  eventId: string;
  body: Ping | AddDrawingSegment | DeleteDrawingSegment | SubmitWord | GuessWord;
}

interface AddDrawingSegment extends DrawingSegment {
  type: MessageType.AddDrawingSegment;
}

interface DeleteDrawingSegment {
  type: MessageType.RemoveDrawingSegment;
  segmentId: string;
}

interface SubmitWord {
  type: MessageType.SubmitWord;
  word: string;
}

interface GuessWord {
  type: MessageType.GuessWord;
  word: string;
}

interface Incoming {
  fromEventId: string;
  body: Pong | Game | AddDrawingSegment | DeleteDrawingSegment | YouAre | WrongGuess | ClearDrawing;
}

interface Ping {
  type: MessageType.Ping;
}

interface Pong {
  type: MessageType.Pong;
}

interface YouAre {
  type: MessageType.YouAre,
  player: Player,
}

export interface WrongGuess {
  type: MessageType.WrongGuess,
}

interface ClearDrawing {
  type: MessageType.ClearDrawing,
}

interface Game {
  type: MessageType.Game;
  id: string;
  stage: PlayerChoosing | PlayerDrawing;
  players: Player[];
  history: Turn[];
}

interface PlayerChoosing {
  type: 'playerChoosing',
  playerId: number;
}

interface PlayerDrawing {
  type: 'playerDrawing',
  playerId: number;
}

interface Player {
  id: number;
  nickname: number;
}

interface Turn { }

interface Db {
  ws: ReconnectingWS | null;
  /** Latest returned game from the server */
  game: Game | null;
  /** This player */
  player: Player | null;
  /** Drawing is a collection of segments known to the server */
  drawing: Map<string, any>;
  /** Pending keeps events that have not yet been synced */
  pending: Message[];
  /** Every change increments current evolution */
  evolution: number;
  /** Listeners will be notified every time db's state changes */
  listeners: ((evolution: number) => void)[];
  /** Holds timeout Id for ping requests */
  pingTimeoutId?: any;
  /**
   * Holds pending requests. When we receive a message we try to resolve pending promises.
   */
  requests: Map<String, LocalRequest>;
}

interface LocalRequest {
  resolve: (value: any) => void;
  reject: (reason: any) => void;
}

const db: Db = Object.seal({
  ws: null,
  game: null,
  player: readSavedPlayer(),
  drawing: new Map(),
  pending: [],
  evolution: 0,
  listeners: [],
  pingTimeoutId: undefined,
  requests: new Map(),
});

/**
 * Try to send pending updates
 */
function sendPending() {
  if (!db.ws) {
    return;
  }

  // Send each pending event
  while (true) {
    const first = db.pending.shift();
    if (!first) {
      break;
    }

    db.ws.send(JSON.stringify(first));
  }
}

function notifyListeners() {
  db.listeners.forEach(listener => {
    listener(db.evolution);
  });
}

function nextEvolution() {
  db.evolution += 1;

  setTimeout(() => {
    notifyListeners();
  }, 1);
}

/**
 * Generate temporary ID until remote confirms it
 */
function eventId() {
  return `local-${Math.round(Math.random() * 10000000)}`;
}

function schedulePing() {
  clearTimeout(db.pingTimeoutId);

  db.pingTimeoutId = setTimeout(() => {
    if (db.ws) {
      db.ws.send(JSON.stringify({
        body: {
          type: MessageType.Ping,
        }
      }));
    }
  }, 20000);
}

/**
 * Schedule a message to be sent.
 */
function scheduleMessage(message: Message) {
  db.pending.push(message);
  nextEvolution();

  setTimeout(() => {
    sendPending();
  }, 1);
}

/**
 * Schedule a message to be sent. Returns a promise that
 * will be fulfilled after receiving a response message.
 */
function requestMessage<T>(message: Message): Promise<T> {
  const promise: Promise<T> = new Promise((resolve, reject) => {
    db.requests.set(message.eventId, {
      resolve,
      reject,
    })
  });

  scheduleMessage(message);
  return promise;
}

interface ConnectOptions {
  onError: (err: Error) => void;
}

export default {
  /**
   * Init DB and establish remote connection
   */
  connect: function connect(gameId: string, { onError }: ConnectOptions) {
    const host = location.host;
    const protocol = location.protocol === 'https:' ? 'wss' : 'ws';
    let url = `${protocol}://${host}/sync?game_id=${gameId}`;
    if (db.player) {
      url += `&player_id=${db.player.id}`;
    }
    const ws = new ReconnectingWS(url);
    db.ws = ws;

    ws.onopen = () => {
      sendPending();
      schedulePing();
    };

    ws.onmessage = (message) => {
      schedulePing();

      try {
        const event: Incoming = JSON.parse(message.data);
        console.log(event.body);

        // Fulfil pending promises
        if (event.fromEventId) {
          const request = db.requests.get(event.fromEventId);
          db.requests.delete(event.fromEventId)

          if (request) {
            console.log('Fulfilling request', event.fromEventId);
            request.resolve(event.body);
            return;
          }
        }

        switch (event.body.type) {
          case MessageType.Game:
            db.game = Object.freeze(event.body);
            nextEvolution();
            break;

          case MessageType.AddDrawingSegment:
            db.drawing.set(event.body.id, event.body);
            nextEvolution();
            break;

          case MessageType.RemoveDrawingSegment:
            db.drawing.delete(event.body.segmentId);
            nextEvolution();
            break;

          case MessageType.YouAre:
            db.player = event.body.player;
            savePlayer(db.player);
            nextEvolution();
            break;

          case MessageType.ClearDrawing:
            db.drawing.clear();
            nextEvolution();

          case MessageType.Pong:
            break;

          default:
            console.warn('Received unknown event:', event.body);
        }
      } catch (e) {
        console.error(e);
      }
    };

    ws.onerror = (e) => {
      onError(new Error(e.toString()));
    };

    ws.onreconnect = (tries) => {
      console.log('WebSocket is reconnecting. Tries=', tries);
      // onError(new Error('Disconnected'));
    };

    ws.onclose = () => {
      // In case we weren't able to reconnect multiple times we need to fail.
      onError(new Error('Disconnected'));
    };
  },

  /**
   * Disconnect closes remote connection and unregisters all listeners.
   */
  disconnect: () => {
    if (db.ws) {
      db.ws.close();
      db.ws = null;
    }

    db.listeners = [];
  },

  listen: (cb: (evolution: number) => void) => {
    db.listeners.push(cb);
  },

  /**
   * Add new drawing segment.
   */
  addDrawingSegment: (segment: DrawingSegment) => {
    db.drawing.set(segment.id, segment);

    scheduleMessage(Object.freeze({
      eventId: eventId(),
      body: {
        ...segment,
        type: MessageType.AddDrawingSegment,
      }
    }));
  },

  /**
   * Remove drawing segment by its id.
   **/
  removeDrawingSegment: (id: string) => {
    db.drawing.delete(id);

    scheduleMessage(Object.freeze({
      eventId: eventId(),
      body: {
        type: MessageType.RemoveDrawingSegment,
        segmentId: id,
      },
    }));
  },

  /**
   * Return a list of drawing segments. We combine segments that were not yet synced
   * with the ones that are known to the server.
   */
  drawingSegments: () => {
    const events = db.pending;
    const local: DrawingSegment[] = [];
    const removed: Set<String> = new Set();

    for (let e of events) {
      switch (e.body.type) {
        case MessageType.AddDrawingSegment:
          local.push(e.body);
          break;

        case MessageType.RemoveDrawingSegment:
          removed.add(e.body.segmentId);
          break;
      }
    }

    const saved: DrawingSegment[] = [];
    for (let segment of db.drawing.values()) {
      if (removed.has(segment.id)) {
        continue;
      }

      saved.push(segment);
    }

    return saved.concat(local);
  },

  /**
   * Return current game
   */
  game: () => db.game,

  /**
   * Return this player
   */
  player: () => db.player,

  /**
   * Submit a word to draw.
   */
  submitWord: (word: string) => {
    scheduleMessage(Object.freeze({
      eventId: eventId(),
      body: {
        type: MessageType.SubmitWord,
        word,
      },
    }));
  },

  /**
   * Guess a word.
   */
  guessWord: (word: string): Promise<WrongGuess> => {
    return requestMessage(Object.freeze({
      eventId: eventId(),
      body: {
        type: MessageType.GuessWord,
        word,
      },
    }));
  },

};

/**
 * Attempt to read a player from local storage
 */
function readSavedPlayer(): Player | null {
  try {
    const value = localStorage.getItem(StorageKey.Player);
    return JSON.parse(value!);
  } catch (e) {
    return null;
  }
}

/**
 * Save player in local storage
 */
function savePlayer(player: Player) {
  localStorage.setItem(StorageKey.Player, JSON.stringify(player));
}
