import styles from './ShareGame.css';

const ShareGame = () => {
  const gameUrl = location.href;

  return (
    <label>
      <p>Share game with friends:</p>
      <div class={styles.share}>
        <input
          type="text"
          readOnly
          onFocus={(e: FocusEvent) => {
            let target = e.target as HTMLInputElement;
            target.select();
          }}
          value={gameUrl} />

        {!!navigator.share && (
          <button
            type="button"
            onClick={() => {
              navigator.share?.({
                text: 'Play Krokodil with me',
                url: gameUrl,
              });
            }}
          >
            Share
          </button>
        )}
      </div>
    </label>
  );
};

export default ShareGame;
