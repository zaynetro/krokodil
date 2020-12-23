import { useState } from 'preact/hooks';
import { WrongGuess, WordTip } from '../db';

import styles from './GuessWord.css';

interface Props {
  onGuess: (word: string) => Promise<WrongGuess>;
  onAskTip: () => Promise<WordTip>;
}

const GuessWord = (props: Props) => {
  const [word, setWord] = useState('');
  const [tip, setTip] = useState('');
  const [error, setError] = useState('');

  return (
    <form
      class={styles.guess}
      onSubmit={async (e) => {
        e.preventDefault();
        setError('');

        if (word.length) {
          try {
            await props.onGuess(word);
            setError('Wrong guess');
          } catch (e) {
          }
        }
      }}>
      <label>
        <span>Guess word:</span>
        <input
          type="text"
          class={error.length ? styles.error : ''}
          onChange={(e) => {
            const word = (e.target as HTMLInputElement).value || '';
            setWord(word);
            setError('');
          }}
          placeholder={tip}
          value={word} />

        <button type="submit">
          Guess
          </button>

        <button
          type="button"
          onClick={async () => {
            try {
              const tip = await props.onAskTip();
              setTip(tip.tip);
              setWord('');
            } catch (e) { }
          }}
        >
          Help
        </button>
      </label>
    </form>
  );
};

export default GuessWord;
