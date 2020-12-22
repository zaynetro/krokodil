import { useState } from 'preact/hooks';

import styles from './PickWord.css';

interface Props {
  onChoose: (word: string) => void;
}

const PickWord = (props: Props) => {
  const [word, setWord] = useState('');

  return (
    <form onSubmit={(e) => {
      e.preventDefault();
      props.onChoose(word);
    }}>
      <label>
        <p>Pick a word:</p>
        <div class={styles.word}>
          <input
            type="text"
            onChange={(e) => setWord((e.target as HTMLInputElement).value || '')}
            value={word} />

          <button type="submit">
            Submit
                  </button>
        </div>
      </label>
    </form>
  );
};

export default PickWord;
