/**
 * Drawing board API
 *
 * References:
 * - https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API/Tutorial/Optimizing_canvas
 * - https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API/Tutorial/Applying_styles_and_colors
 */

import db, { Point, Color } from './db';

interface MousePosition {
  clientX: number;
  clientY: number;
}

class Board {

  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;

  /** Holds a list of currently drawn points. If non-empty then drawing is active. */
  private currentPoints: Point[] = [];
  /** Only one player is allowed to draw at a time */
  private disabled = true;

  color: Color = Color.Black;
  lineWidth = 2;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    // Turn off transparency for performance
    this.ctx = canvas.getContext('2d', { alpha: false })!;

    this.addListeners();
    this.setStyles();
    this.clean();
  }

  /**
   * Set default drawing styles
   */
  private setStyles() {
    this.ctx.fillStyle = 'rgb(255, 255, 255)';
    this.ctx.strokeStyle = this.color;
    this.ctx.lineWidth = this.lineWidth;
    this.ctx.lineCap = 'round';
    this.ctx.lineJoin = 'round';
  }

  /**
   * Clean canvas
   */
  clean() {
    this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);
  }

  /**
   * Redraw
   */
  redraw() {
    this.setStyles();
    this.clean();
    for (let segment of db.drawingSegments()) {
      this.ctx.strokeStyle = segment.stroke;
      this.ctx.lineWidth = segment.lineWidth;
      this.ctx.beginPath();
      segment.points.forEach((point, i) => {
        if (i == 0) {
          this.ctx.moveTo(point.x, point.y);
        } else {
          this.ctx.lineTo(point.x, point.y);
        }
      });
      this.ctx.stroke();
    }
  }

  /**
   * Enable context for drawing. Starts reacting to user events.
   */
  enable() {
    this.disabled = false;
  }

  /**
   * Disable context for drawing. Stops reacting to user events.
   */
  disable() {
    this.disabled = true;
  }

  /**
   * Whether drawing is currently active.
   */
  get drawing() {
    return this.currentPoints.length > 0;
  }

  beginDrawing(e: MousePosition) {
    if (this.disabled) {
      return;
    }

    const point = this.coords(e);
    this.currentPoints.push(point);
    this.ctx.strokeStyle = this.color;
    this.ctx.lineWidth = this.lineWidth;
  }

  stopDrawing(e: MousePosition) {
    if (!this.drawing || this.disabled) {
      return;
    }

    const segment = {
      id: segmentId(),
      stroke: this.color,
      lineWidth: this.lineWidth,
      points: this.currentPoints,
    };
    db.addDrawingSegment(segment);
    this.currentPoints = [];
  }

  draw(e: MousePosition) {
    if (!this.drawing || this.disabled) {
      return;
    }

    const point = this.coords(e);
    const lastPoint = this.currentPoints[this.currentPoints.length - 1];
    this.ctx.beginPath();
    this.ctx.moveTo(lastPoint.x, lastPoint.y);
    this.ctx.lineTo(point.x, point.y);
    this.ctx.stroke();
    this.currentPoints.push(point);
  }

  /**
   * Undo previously drawn path.
   */
  undo() {
    if (this.disabled) {
      return;
    }

    const segments = db.drawingSegments();
    if (segments.length) {
      const last = segments[segments.length - 1];
      db.removeDrawingSegment(last.id);
    }
  }

  /**
   * Set drawing color
   */
  setColor(color: Color) {
    this.color = color;
  }

  setLineWidth(width: number) {
    this.lineWidth = width;
  }

  /**
   * Resize Canvas to fill parent
   */
  resize() {
    const width = this.canvas.parentElement?.clientWidth;
    const height = this.canvas.parentElement?.clientHeight;
    if (width && width > 0) {
      this.canvas.width = width;
    }

    if (height && height > 0) {
      this.canvas.height = height;
    }

    this.redraw();
  }

  /**
   * Find coordinates relative to the board
   */
  private coords(e: MousePosition): Point {
    const box = this.canvas.getBoundingClientRect();
    return {
      x: Math.round(e.clientX) - Math.round(box.left),
      y: Math.round(e.clientY) - Math.round(box.top),
    }
  }

  private addListeners() {
    const canvas = this.canvas;
    canvas.addEventListener('mousedown', (e) => this.beginDrawing(e), false);
    canvas.addEventListener("touchstart", (e) => this.wrapTouch(e, this.beginDrawing), false);

    canvas.addEventListener('mouseup', (e) => this.stopDrawing(e), false);
    canvas.addEventListener('touchend', (e) => this.wrapTouch(e, this.stopDrawing), false);

    // Seems like not having these provides better drawing experience
    // canvas.addEventListener('mouseleave', (e) => this.stopDrawing(e), false);
    // canvas.addEventListener('touchcancel', (e) => this.wrapTouch(e, this.stopDrawing), false);

    canvas.addEventListener('mousemove', (e) => this.draw(e), false);
    canvas.addEventListener('touchmove', (e) => this.wrapTouch(e, this.draw), false);

    window.addEventListener('resize', () => this.resize(), false);
  }

  private wrapTouch(e: TouchEvent, cb: (e: MousePosition) => void) {
    if (e.changedTouches.length !== 1) {
      // Ignore multiple touches.
      return;
    }

    e.preventDefault();
    const touch = e.changedTouches[0];
    cb.call(this, touch);
  }

}

function segmentId() {
  return `seg-${Math.round(Math.random() * 10000000)}`;
}

export default Board;
