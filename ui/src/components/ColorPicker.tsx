import { Color } from '../db';
import ColorButton from './ColorButton';

import styles from './ColorPicker.css';

interface Props {
  selected: Color,
  onSelect: (color: Color) => void;
}

const ColorPicker = (props: Props) => (
  <ul class={styles.colorPicker}>
    {Object.entries(Color).map(([name, rgb]) => (
      <li key={name}>
        <ColorButton
          selected={rgb === props.selected}
          color={rgb}
          onSelect={() => props.onSelect(rgb)} />
      </li>
    ))}
  </ul>
);

export default ColorPicker;
