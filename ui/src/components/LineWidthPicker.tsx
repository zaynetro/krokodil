import LineWidthButton from './LineWidthButton';

import styles from './LineWidthPicker.css';

interface Props {
  selected: number,
  onSelect: (lineWidth: number) => void;
}

export const DEFAULT_WIDTH: number = 2;

const options = [{
  width: DEFAULT_WIDTH,
  percentage: 25,
}, {
  width: 6,
  percentage: 50,
}, {
  width: 20,
  percentage: 75,
}, {
  width: 50,
  percentage: 100,
}];

const LineWidthPicker = (props: Props) => (
  <ul class={styles.linePicker}>
    {options.map((option) => (
      <li key={option.width}>
        <LineWidthButton
          selected={option.width === props.selected}
          percentage={option.percentage}
          onSelect={() => props.onSelect(option.width)} />
      </li>
    ))}
  </ul>
);

export default LineWidthPicker;
