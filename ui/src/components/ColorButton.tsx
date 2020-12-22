import { Color } from '../db';

import styles from './ColorButton.css';

interface Props {
  selected: boolean;
  color: Color;
  onSelect: () => void;
}

const ColorButton = (props: Props) => (
  <div
    class={[styles.color, props.selected ? styles.colorSelected : ''].join(' ')}
    style={{
      backgroundColor: props.color,
    }}
    onClick={props.onSelect} />
);

export default ColorButton;
