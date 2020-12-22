/**
 * Reconnecting WebSocket
 */

const MAX_TRIES = 5;

class ReconnectingWS {
  private ws: WebSocket;
  private tries = 0;

  // Overridable fields
  onopen = () => { };
  onmessage = (_message: MessageEvent<any>) => { };
  onerror = (_e: Event) => { };
  onreconnect = (_tries: number) => { };
  onclose = () => { };

  send(data: string) {
    this.ws.send(data);
  }

  close() {
    this.ws.close();
  }

  constructor(private url: string) {
    this.ws = this.connect();
  }

  private connect() {
    const ws = new WebSocket(this.url);
    ws.onopen = () => {
      this.tries = 0;
      this.onopen();
    };

    ws.onmessage = (message) => {
      this.onmessage(message);
    };

    ws.onerror = (e) => {
      this.onerror(e);
    };

    ws.onclose = () => {
      this.onreconnect(this.tries);

      if (this.tries >= MAX_TRIES) {
        this.onclose();
        return;
      };

      this.tries += 1;
      const timeout = Math.pow(2, this.tries) * 1000;

      setTimeout(() => {
        this.ws = this.connect();
      }, timeout);
    };

    return ws;
  }


}

export default ReconnectingWS;
