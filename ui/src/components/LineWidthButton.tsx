import styles from './LineWidthButton.css';

interface Props {
  selected: boolean;
  percentage: number;
  onSelect: () => void;
}

const LineWidthButton = (props: Props) => (
  <div
    class={[styles.lineWidth, props.selected ? styles.selected : ''].join(' ')}
    onClick={props.onSelect}>
    <span style={{
      height: `${props.percentage}%`,
      marginTop: `${(100 - props.percentage) / 2}%`,
    }} />
  </div>
);

export default LineWidthButton;
