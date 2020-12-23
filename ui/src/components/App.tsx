import { Component } from 'preact';

import ColorPicker from './ColorPicker';
import LineWidthPicker, { DEFAULT_WIDTH } from './LineWidthPicker';
import Spinner from './Spinner';
import ShareGame from './ShareGame';
import PickWord from './PickWord';
import GuessWord from './GuessWord';

import db, { Color } from '../db';
import Board from '../board';

import styles from './App.css';

interface State {
  connecting: boolean;
  error: Error | null;
  evolution: number;
}

interface Props { }

class App extends Component<Props, State> {

  board: Board | null = null;
  state: State = {
    connecting: true,
    error: null,
    evolution: 0,
  };

  setBoardRef = (canvas: HTMLCanvasElement | null) => {
    this.board = new Board(canvas!);
    this.updateBoard();
  }

  undo = () => {
    this.board!.undo();
  }

  componentDidCatch(error: Error) {
    console.log('Failed to render App', error);
  }

  componentDidMount() {
    const gameId = location.pathname.split('/')[2];
    db.connect(gameId, {
      onError: (e: Error) => {
        this.setState({
          connecting: false,
          error: e,
        });
      },
    });

    db.listen((evolution) => {
      if (db.game() !== null && this.state.connecting) {
        // Game loaded
        this.setState({
          connecting: false,
          error: null,
        });

        // Resize board after it is mounted.
        setTimeout(() => {
          this.board?.resize();
        }, 200);
      }

      // DB has changed trigger a render
      this.setState({ evolution });
      this.updateBoard();
    });
  }

  componentWillUnmount() {
    db.disconnect();
  }

  updateBoard() {
    if (this.isPlayerDrawing()) {
      this.board?.enable();
    } else {
      this.board?.disable();
    }

    const game = db.game();
    if (game?.stage.type === 'playerDrawing') {
      this.board?.setOriginalSize(game.stage.drawing.canvas);
    }

    this.board?.redraw();
  }

  /**
   * Returns true if this player is currently drawing
   */
  isPlayerDrawing = () => {
    const game = db.game();
    return game !== null && game?.stage.type === 'playerDrawing' && game?.stage.playerId === db.player()?.id;
  }

  render() {
    const player = db.player();
    return (
      <div class={styles.app}>
        <header>
          <h1>
            <a href="/">Krokodil</a>
            {!!this.state.error && (
              <>
                :
              <span class={styles.error}>{this.state.error}</span>
              </>
            )}
          </h1>

          {!!player && (
            <h2>
              You are: {player.nickname}
            </h2>
          )}
        </header>

        {this.state.connecting
          ? this.renderSpinner()
          : this.renderBody()}
      </div>
    );
  }

  renderBody() {
    return (
      <main>
        {this.renderOverlay()}

        <div class={styles.toolbox}>
          <ColorPicker
            selected={this.board?.color || Color.Black}
            onSelect={(color) => {
              this.board!.setColor(color);
              // Trigger UI update
              this.setState({
                evolution: this.state.evolution + 1,
              });
            }} />

          <LineWidthPicker
            selected={this.board?.lineWidth || DEFAULT_WIDTH}
            onSelect={(width) => {
              this.board!.setLineWidth(width);
              // Trigger UI update
              this.setState({
                evolution: this.state.evolution + 1,
              });
            }} />

          <div class={styles.undo}>
            <button type="button" onClick={this.undo}>Undo</button>
          </div>
        </div>

        <div class={styles.board}>
          <canvas ref={this.setBoardRef}></canvas>
        </div>

        {!this.isPlayerDrawing() && (
          <GuessWord
            onGuess={(word) => db.guessWord(word)}
            onAskTip={() => db.askWordTip()} />
        )}
      </main>
    )
  }

  renderOverlay() {
    const game = db.game();
    if (game === null || game.stage.type !== 'playerChoosing') {
      return null;
    }

    const choosingPlayer = game.players.find((p) => p.id === game.stage.playerId);
    const choosingNickname = choosingPlayer ? `Player ${choosingPlayer.nickname}` : 'Other player';

    return (
      <div class={styles.overlay}>
        <div class={styles.overlayContent}>
          {db.game()?.players.length === 1 && (
            <ShareGame />
          )}

          {game.stage.playerId === db.player()?.id ? (
            <PickWord onChoose={(word) => {
              if (word.length) {
                db.submitWord(word, this.board!.size);
              }
            }} />
          ) : (
              <label>{choosingNickname} is choosing a word.</label>
            )}

          {!!game.history.length && (
            <div class={styles.history}>
              <h3>History:</h3>

              <ol>
                {game.history
                  .map((turn, i) => (
                    <li key={i}>{turn.word} (Guessed by {turn.playerGuessed?.nickname || 'unknown'})</li>
                  ))}
              </ol>
            </div>
          )}

          <div class={styles.players}>
            <h3>Other players:</h3>

            <ul>
              {game.players
                .filter((p) => p.id !== db.player()?.id)
                .map((player) => (
                  <li key={player.id}>{player.nickname}</li>
                ))}
            </ul>
          </div>
        </div>
      </div>
    );
  }

  renderSpinner() {
    return (
      <div class={styles.loading}>
        <Spinner />
      </div>
    );
  }
}

export default App;
